//! File transfer over encrypted sessions.
//!
//! Outgoing files are fed by the frontend one chunk at a time (`begin`,
//! `chunk`, `end`): Android's IPC only carries JSON, so this keeps memory
//! bounded on every platform and gives the UI its progress for free. Each
//! chunk is sealed and relayed to every target as a binary frame.
//!
//! Incoming files stream into a hidden `.part` file next to their destination
//! and are renamed only once the size and SHA-256 match what the sender announced.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, Result};
use crate::protocol::{
    b64d, b64e, decrypt_wire, encrypt_plain, open_sealed, read_any_frame, write_frame,
    write_sealed, Frame, Plain, FEATURE_FILES, FILE_CHUNK_BYTES, MAX_FILE_BYTES,
};
use crate::session::{self, reject, within, Session, IDLE_TIMEOUT};
use crate::state::{now_ms, AppState, Direction, FileMeta, HistoryItem, PeerRecord, SendResult};

const START_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_NAME_CHARS: usize = 120;

/// Outgoing transfers by id, each behind its own async lock so a slow peer
/// never blocks another transfer.
pub type Outgoing = std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<Transfer>>>>;

pub struct Transfer {
    item: HistoryItem,
    size: u64,
    sent: u64,
    hasher: Sha256,
    targets: Vec<Target>,
}

struct Target {
    peer: PeerRecord,
    /// The open session, or why this device dropped out.
    session: std::result::Result<Session, String>,
}

impl Target {
    fn new(peer: PeerRecord, session: Result<Session>) -> Self {
        let session = session.map_err(|e| {
            log::info!("file transfer to {} failed: {e}", peer.name);
            e.to_string()
        });
        Target { peer, session }
    }
}

fn too_large_message() -> String {
    format!(
        "The file is too large ({} MB max)",
        MAX_FILE_BYTES / (1024 * 1024)
    )
}

/// Opens a session to every target and announces the file. Fails only when
/// no device accepted it; partial failures are reported by [`end`].
pub async fn begin(
    app: &AppHandle,
    target: String,
    name: String,
    size: u64,
    mime: String,
) -> Result<String> {
    if size == 0 {
        return Err(AppError::msg("The file is empty"));
    }
    if size > MAX_FILE_BYTES {
        return Err(AppError::msg(too_large_message()));
    }
    let name = clean_file_name(&name);
    let peers = session::resolve_targets(app, &target)?;

    let id = uuid::Uuid::new_v4().to_string();
    let start = Plain::FileStart {
        id: id.clone(),
        ts: now_ms(),
        sender: app.state::<AppState>().identity.device_id.clone(),
        name: name.clone(),
        size,
        mime: mime.clone(),
    };

    let mut opening = tokio::task::JoinSet::new();
    for peer in peers.iter().cloned() {
        let app = app.clone();
        let start = start.clone();
        let id = id.clone();
        opening.spawn(async move {
            let session = within(START_TIMEOUT, async {
                let mut session = session::open_session(&app, &peer).await?;
                if !session.features.iter().any(|f| f == FEATURE_FILES) {
                    return Err(AppError::msg(
                        "This device runs an older NearClip that cannot receive files",
                    ));
                }
                let frame = encrypt_plain(&session.key, &start)?;
                write_frame(&mut session.stream, &frame).await?;
                session::expect_ack(&mut session, &id).await?;
                Ok(session)
            })
            .await;
            Target::new(peer, session)
        });
    }
    let targets = opening.join_all().await;

    if targets.iter().all(|t| t.session.is_err()) {
        let reason = targets.iter().find_map(|t| t.session.as_ref().err());
        return Err(AppError::msg(reason.cloned().unwrap_or_default()));
    }

    let item = HistoryItem {
        id: id.clone(),
        direction: Direction::Sent,
        peer_name: session::target_label(&target, &peers),
        peer_id: target,
        text: String::new(),
        ts_ms: now_ms(),
        ok: true,
        file: Some(FileMeta {
            name,
            size,
            mime,
            path: None,
        }),
    };
    let transfer = Transfer {
        item,
        size,
        sent: 0,
        hasher: Sha256::new(),
        targets,
    };
    app.state::<AppState>()
        .outgoing
        .lock()
        .unwrap()
        .insert(id.clone(), Arc::new(tokio::sync::Mutex::new(transfer)));
    Ok(id)
}

/// Relays one chunk (base64 from the frontend) to every device still taking
/// part. Returns how many bytes have been sent so far.
pub async fn chunk(app: &AppHandle, id: &str, data: String) -> Result<u64> {
    let transfer = app
        .state::<AppState>()
        .outgoing
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(inactive)?;
    let mut t = transfer.lock().await;
    let bytes = b64d(&data)?;
    if bytes.is_empty() || bytes.len() > FILE_CHUNK_BYTES {
        return Err(AppError::msg("Invalid chunk size"));
    }
    if t.sent + bytes.len() as u64 > t.size {
        return Err(AppError::msg("More data than the announced file size"));
    }
    t.hasher.update(&bytes);
    t.sent += bytes.len() as u64;

    for target in t.targets.iter_mut() {
        let Ok(session) = target.session.as_mut() else {
            continue;
        };
        let written = within(
            IDLE_TIMEOUT,
            write_sealed(&mut session.stream, &session.key, &bytes),
        )
        .await;
        if let Err(e) = written {
            log::info!("file transfer to {} failed: {e}", target.peer.name);
            target.session = Err(e.to_string());
        }
    }
    if t.targets.iter().all(|t| t.session.is_err()) {
        drop(t);
        abort(app, id);
        return Err(AppError::msg("Every device stopped receiving the file"));
    }
    Ok(t.sent)
}

/// Finishes the transfer: sends the checksum, waits for each device to confirm
/// it saved the file, and records one history item.
pub async fn end(app: &AppHandle, id: &str) -> Result<Vec<SendResult>> {
    let transfer = app
        .state::<AppState>()
        .outgoing
        .lock()
        .unwrap()
        .remove(id)
        .ok_or_else(inactive)?;
    let mut t = transfer.lock().await;
    if t.sent != t.size {
        return Err(AppError::msg("The file was not sent completely"));
    }
    let digest = std::mem::take(&mut t.hasher).finalize();
    let end = Plain::FileEnd {
        id: id.to_string(),
        sha256: b64e(&digest),
    };

    let mut results = Vec::with_capacity(t.targets.len());
    for target in std::mem::take(&mut t.targets) {
        let outcome = match target.session {
            Ok(mut session) => within(IDLE_TIMEOUT, async {
                let frame = encrypt_plain(&session.key, &end)?;
                write_frame(&mut session.stream, &frame).await?;
                session::expect_ack(&mut session, id).await?;
                Ok(session.addr)
            })
            .await
            .map(|addr| crate::transport::remember_peer_addr(app, &target.peer.device_id, addr))
            .map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        results.push(SendResult {
            device_id: target.peer.device_id,
            ok: outcome.is_ok(),
            error: outcome.err(),
        });
    }
    session::record_sent(app, t.item.clone(), results.clone());
    Ok(results)
}

/// Drops a transfer; the open connections close and receivers discard the partial file.
pub fn abort(app: &AppHandle, id: &str) {
    app.state::<AppState>().outgoing.lock().unwrap().remove(id);
}

fn inactive() -> AppError {
    AppError::msg("This transfer is no longer active")
}

/// Receives the file a `FileStart` announced, through to its `FileEnd`, and
/// returns where it was saved once the sender has its `Ack`.
pub async fn receive(
    app: &AppHandle,
    session: &mut Session,
    id: String,
    name: String,
    size: u64,
    mime: String,
) -> Result<FileMeta> {
    if size == 0 || size > MAX_FILE_BYTES {
        return Err(reject(session, "too_large", &too_large_message()).await);
    }
    receive_into(&received_dir(app)?, session, id, name, size, mime).await
}

async fn receive_into(
    dir: &Path,
    session: &mut Session,
    id: String,
    name: String,
    size: u64,
    mime: String,
) -> Result<FileMeta> {
    let name = clean_file_name(&name);
    let part = PartFile(dir.join(format!(".{id}.part")));
    let mut file = tokio::fs::File::create(&part.0).await?;
    let ready = encrypt_plain(&session.key, &Plain::Ack { id: id.clone() })?;
    write_frame(&mut session.stream, &ready).await?;

    let mut received: u64 = 0;
    let mut hasher = Sha256::new();
    loop {
        let frame = within(IDLE_TIMEOUT, read_any_frame(&mut session.stream)).await?;
        let expected = match frame {
            Frame::Sealed(body) => {
                let bytes = open_sealed(&session.key, &body)?;
                received += bytes.len() as u64;
                if received > size {
                    return Err(reject(session, "too_large", "file larger than announced").await);
                }
                hasher.update(&bytes);
                file.write_all(&bytes).await?;
                continue;
            }
            Frame::Wire(wire) => match decrypt_wire(&session.key, &wire)? {
                Plain::FileEnd { id: end_id, sha256 } if end_id == id => sha256,
                other => {
                    return Err(AppError::protocol(format!(
                        "unexpected payload during file transfer: {other:?}"
                    )))
                }
            },
        };

        if received != size || b64d(&expected)? != hasher.finalize().as_slice() {
            return Err(reject(session, "corrupt", "file does not match its checksum").await);
        }
        file.flush().await?;
        drop(file);
        let path = unique_path(dir, &name);
        // Once renamed, the guard's cleanup finds nothing to remove.
        tokio::fs::rename(&part.0, &path).await?;

        let ack = encrypt_plain(&session.key, &Plain::Ack { id })?;
        write_frame(&mut session.stream, &ack).await?;
        log::info!("received {} ({size} bytes)", path.display());
        return Ok(FileMeta {
            name,
            size,
            mime,
            path: Some(path.to_string_lossy().into_owned()),
        });
    }
}

/// Removes an unfinished `.part` file when a transfer fails or is cut off.
struct PartFile(PathBuf);

impl Drop for PartFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Where received files go: Downloads/NearClip. On Android, `home_dir` is the
/// shared storage of the current user or work profile, whose Download folder
/// is writable without a permission from Android 11; if that fails, files go
/// to the app's own downloads folder.
fn received_dir(app: &AppHandle) -> Result<PathBuf> {
    if cfg!(target_os = "android") {
        if let Ok(home) = app.path().home_dir() {
            let public = home.join("Download").join("NearClip");
            if std::fs::create_dir_all(&public).is_ok() {
                return Ok(public);
            }
        }
    }
    let dir = app.path().download_dir()?.join("NearClip");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// A file name that is safe on every platform: no directories, no reserved
/// characters, no leading dots, and not too long (the extension is kept).
pub fn clean_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim().to_string();
    if cleaned.is_empty() {
        return "file".to_string();
    }
    if cleaned.chars().count() <= MAX_NAME_CHARS {
        return cleaned;
    }
    let (stem, ext) = split_extension(&cleaned);
    let keep = MAX_NAME_CHARS.saturating_sub(ext.chars().count());
    format!("{}{ext}", stem.chars().take(keep).collect::<String>())
}

/// "photo.jpg" -> ("photo", ".jpg"); names without an extension keep it empty.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 10 => name.split_at(i),
        _ => (name, ""),
    }
}

/// `dir/name`, or `dir/name (2)`, `(3)`... when that file already exists.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = split_extension(name);
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("unbounded range always finds a free name")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{random_bytes, sha256};

    /// Two sessions sharing one key over a loopback TCP connection.
    async fn session_pair() -> (Session, Session) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (client, server) =
            tokio::join!(tokio::net::TcpStream::connect(addr), listener.accept());
        let key: [u8; 32] = random_bytes();
        let (server, peer_addr) = server.unwrap();
        let session = |stream, addr| Session {
            stream,
            key,
            addr,
            features: Vec::new(),
        };
        (session(client.unwrap(), addr), session(server, peer_addr))
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nearclip-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Sender side after `FileStart`: wait for ready, send the bytes and the checksum.
    async fn send_file(sender: &mut Session, id: &str, data: &[u8], checksum: &[u8]) {
        session::expect_ack(sender, id).await.unwrap();
        for piece in data.chunks(FILE_CHUNK_BYTES) {
            write_sealed(&mut sender.stream, &sender.key, piece)
                .await
                .unwrap();
        }
        let end = Plain::FileEnd {
            id: id.to_string(),
            sha256: b64e(checksum),
        };
        let frame = encrypt_plain(&sender.key, &end).unwrap();
        write_frame(&mut sender.stream, &frame).await.unwrap();
    }

    #[tokio::test]
    async fn file_arrives_intact_across_several_chunks() {
        let (mut sender, mut receiver) = session_pair().await;
        let dir = temp_dir();
        let data: Vec<u8> = (0..FILE_CHUNK_BYTES * 2 + 1234)
            .map(|i| (i % 251) as u8)
            .collect();

        let receiving = receive_into(
            &dir,
            &mut receiver,
            "t1".into(),
            "../photo.jpg".into(),
            data.len() as u64,
            "image/jpeg".into(),
        );
        let sending = async {
            send_file(&mut sender, "t1", &data, &sha256(&data)).await;
            session::expect_ack(&mut sender, "t1").await.unwrap();
        };
        let (file, ()) = tokio::join!(receiving, sending);

        let file = file.unwrap();
        assert_eq!(file.name, "photo.jpg");
        assert_eq!(std::fs::read(file.path.unwrap()).unwrap(), data);
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "no .part file left behind"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn corrupt_file_is_rejected_and_discarded() {
        let (mut sender, mut receiver) = session_pair().await;
        let dir = temp_dir();
        let data = vec![7u8; 4096];

        let receiving = receive_into(
            &dir,
            &mut receiver,
            "t2".into(),
            "a.bin".into(),
            data.len() as u64,
            String::new(),
        );
        let sending = async {
            send_file(&mut sender, "t2", &data, &[0u8; 32]).await;
            session::expect_ack(&mut sender, "t2").await
        };
        let (received, acked) = tokio::join!(receiving, sending);

        assert!(received.is_err());
        assert!(matches!(acked, Err(AppError::Remote { code, .. }) if code == "corrupt"));
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "partial file removed"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_names_are_made_safe() {
        assert_eq!(clean_file_name("photo.jpg"), "photo.jpg");
        assert_eq!(clean_file_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_file_name("C:\\Users\\me\\report.pdf"), "report.pdf");
        assert_eq!(clean_file_name(".hidden"), "hidden");
        assert_eq!(clean_file_name("a:b*c?.txt"), "a_b_c_.txt");
        assert_eq!(clean_file_name("  "), "file");
        assert_eq!(clean_file_name("dir/"), "file");
        let long = format!("{}.png", "x".repeat(300));
        let cleaned = clean_file_name(&long);
        assert_eq!(cleaned.chars().count(), MAX_NAME_CHARS);
        assert!(cleaned.ends_with(".png"));
    }

    #[test]
    fn duplicate_names_get_a_counter() {
        let dir = temp_dir();
        assert_eq!(unique_path(&dir, "a.txt"), dir.join("a.txt"));
        std::fs::write(dir.join("a.txt"), b"1").unwrap();
        assert_eq!(unique_path(&dir, "a.txt"), dir.join("a (2).txt"));
        std::fs::write(dir.join("a (2).txt"), b"2").unwrap();
        assert_eq!(unique_path(&dir, "a.txt"), dir.join("a (3).txt"));
        assert_eq!(unique_path(&dir, "README"), dir.join("README"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
