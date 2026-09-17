//! Encrypted sessions between paired devices. One connection per send.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::net::TcpStream;
use tokio::task::JoinSet;

use crate::clipboard::ClipboardCmd;
use crate::crypto::{derive_session_key, random_bytes};
use crate::error::{AppError, Result};
use crate::protocol::{
    b64d_array, b64e, decrypt_wire, encrypt_plain, error_frame, read_frame, write_frame, Plain,
    Wire, FEATURE_FILES, MAX_TEXT_BYTES, PROTO_VERSION,
};
use crate::state::{forget_peer, now_ms, AppState, Direction, HistoryItem, PeerRecord, SendResult};
use crate::{transfer, transport};

const SEND_TIMEOUT: Duration = Duration::from_secs(10);
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ClipboardSentEvent {
    item: HistoryItem,
    results: Vec<SendResult>,
}

/// An open, keyed session to one peer.
pub struct Session {
    pub stream: TcpStream,
    pub key: [u8; 32],
    pub addr: SocketAddr,
    /// What the other device announced it supports (client side only).
    pub features: Vec<String>,
}

/// Runs `fut`, failing with `AppError::Timeout` after `limit`.
pub async fn within<T>(limit: Duration, fut: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(limit, fut).await?
}

/// Tells the peer why the session stops, and returns the matching local error.
pub async fn reject(session: &mut Session, code: &str, msg: &str) -> AppError {
    let _ = write_frame(&mut session.stream, &error_frame(code, msg)).await;
    AppError::protocol(msg)
}

/// The paired devices `target` designates: one device id, or "all".
pub fn resolve_targets(app: &AppHandle, target: &str) -> Result<Vec<PeerRecord>> {
    let state = app.state::<AppState>();
    let peers = state.peers.lock().unwrap();
    let targets: Vec<PeerRecord> = if target == "all" {
        peers.values().cloned().collect()
    } else {
        peers.get(target).cloned().into_iter().collect()
    };
    if targets.is_empty() {
        return Err(AppError::msg(if target == "all" {
            "No paired devices yet"
        } else {
            "This device is not paired"
        }));
    }
    Ok(targets)
}

/// History label for a send: the device name, or "All paired devices".
pub fn target_label(target: &str, targets: &[PeerRecord]) -> String {
    if target == "all" {
        "All paired devices".to_string()
    } else {
        targets[0].name.clone()
    }
}

/// Stores a sent item and tells the frontend how each device fared.
pub fn record_sent(app: &AppHandle, mut item: HistoryItem, results: Vec<SendResult>) {
    item.ok = results.iter().all(|r| r.ok);
    app.state::<AppState>().record_history(app, item.clone());
    let _ = app.emit("clipboard-sent", ClipboardSentEvent { item, results });
}

/// Sends `text` to one paired device (`target` = device id) or to every paired
/// device (`target` = "all"). Records one history item and emits `clipboard-sent`.
pub async fn send_text(app: AppHandle, target: String, text: String) -> Vec<SendResult> {
    let targets = match resolve_targets(&app, &target) {
        Ok(t) => t,
        Err(e) => {
            return vec![SendResult {
                device_id: target,
                ok: false,
                error: Some(e.to_string()),
            }]
        }
    };

    let item_id = uuid::Uuid::new_v4().to_string();
    let mut set = JoinSet::new();
    for peer in targets.iter().cloned() {
        let app = app.clone();
        let text = text.clone();
        let item_id = item_id.clone();
        set.spawn(async move {
            let r = send_text_to_peer(&app, &peer, &item_id, &text).await;
            SendResult {
                device_id: peer.device_id.clone(),
                ok: r.is_ok(),
                error: r.err().map(|e| e.to_string()),
            }
        });
    }
    let results: Vec<SendResult> = set.join_all().await;

    let item = HistoryItem {
        id: item_id,
        direction: Direction::Sent,
        peer_name: target_label(&target, &targets),
        peer_id: target,
        text,
        ts_ms: now_ms(),
        ok: true,
        file: None,
    };
    record_sent(&app, item, results.clone());
    results
}

/// `forget_peer` writes to disk; keep that off the async network threads.
fn forget_peer_in_background(app: &AppHandle, device_id: &str) {
    let app = app.clone();
    let device_id = device_id.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        forget_peer(&app, &device_id);
    });
}

/// Connects to a paired peer and derives the per-connection key. If the peer
/// no longer recognizes us, the local pairing is dropped as a side effect.
pub async fn open_session(app: &AppHandle, peer: &PeerRecord) -> Result<Session> {
    let state = app.state::<AppState>();
    let addr = transport::resolve_peer_addr(&state, &peer.device_id)
        .or(peer.last_seen_addr)
        .ok_or(AppError::DeviceUnavailable)?;

    let mut stream = transport::connect(addr).await?;
    let salt_c: [u8; 32] = random_bytes();
    write_frame(
        &mut stream,
        &Wire::SessionInit {
            v: PROTO_VERSION,
            device_id: state.identity.device_id.clone(),
            salt: b64e(&salt_c),
            port: state.listen_port.load(Ordering::Relaxed),
        },
    )
    .await?;

    let (salt_s, features) = match read_frame(&mut stream).await? {
        Wire::SessionAck {
            device_id,
            salt,
            features,
        } => {
            if device_id != peer.device_id {
                return Err(AppError::protocol("responder identity mismatch"));
            }
            (b64d_array::<32>(&salt)?, features)
        }
        Wire::Error { code, .. } if code == "unknown_peer" => {
            // The other side forgot this pairing (unpaired while we were away):
            // drop it here too so the lists agree again.
            forget_peer_in_background(app, &peer.device_id);
            return Err(AppError::msg(
                "The other device removed this pairing. Pair again.",
            ));
        }
        Wire::Error { code, msg } => return Err(AppError::Remote { code, msg }),
        other => {
            return Err(AppError::protocol(format!(
                "expected SessionAck, got {other:?}"
            )))
        }
    };
    let key = derive_session_key(&peer.pairing_key, &salt_c, &salt_s);
    Ok(Session {
        stream,
        key,
        addr,
        features,
    })
}

/// Reads the peer's `Ack` for `id`. Error frames come back as `AppError::Remote`.
pub async fn expect_ack(session: &mut Session, id: &str) -> Result<()> {
    match decrypt_wire(&session.key, &read_frame(&mut session.stream).await?)? {
        Plain::Ack { id: acked } if acked == id => Ok(()),
        other => Err(AppError::protocol(format!("expected Ack, got {other:?}"))),
    }
}

pub async fn send_text_to_peer(
    app: &AppHandle,
    peer: &PeerRecord,
    item_id: &str,
    text: &str,
) -> Result<()> {
    let my_id = app.state::<AppState>().identity.device_id.clone();
    let addr = within(SEND_TIMEOUT, async {
        let mut session = open_session(app, peer).await?;
        let frame = encrypt_plain(
            &session.key,
            &Plain::Clipboard {
                id: item_id.to_string(),
                ts: now_ms(),
                sender: my_id,
                text: text.to_string(),
            },
        )?;
        write_frame(&mut session.stream, &frame).await?;
        expect_ack(&mut session, item_id).await?;
        Ok(session.addr)
    })
    .await?;

    transport::remember_peer_addr(app, &peer.device_id, addr);
    Ok(())
}

/// Best effort: tells a peer we removed the pairing so it forgets us too.
pub async fn notify_unpair(app: AppHandle, peer: PeerRecord) {
    let sent = within(SEND_TIMEOUT, async {
        let mut session = open_session(&app, &peer).await?;
        let frame = encrypt_plain(&session.key, &Plain::Unpair)?;
        write_frame(&mut session.stream, &frame).await
    })
    .await;
    if let Err(e) = sent {
        log::debug!("unpair notice to {} not delivered: {e}", peer.name);
    }
}

/// Serves an accepted connection whose first frame was `SessionInit`.
pub async fn serve(
    app: AppHandle,
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    first: Wire,
) -> Result<()> {
    let Wire::SessionInit {
        v,
        device_id,
        salt,
        port,
    } = first
    else {
        return Err(AppError::protocol("expected SessionInit"));
    };
    if v != PROTO_VERSION {
        let _ = write_frame(
            &mut stream,
            &error_frame("version", "unsupported protocol version"),
        )
        .await;
        return Err(AppError::protocol("version mismatch"));
    }

    let state = app.state::<AppState>();
    let my_id = state.identity.device_id.clone();
    let peer = state.peers.lock().unwrap().get(&device_id).cloned();
    let Some(peer) = peer else {
        let _ = write_frame(&mut stream, &error_frame("unknown_peer", "not paired")).await;
        return Err(AppError::UnknownPeer);
    };

    let salt_c: [u8; 32] = b64d_array(&salt)?;
    let salt_s: [u8; 32] = random_bytes();
    write_frame(
        &mut stream,
        &Wire::SessionAck {
            device_id: my_id,
            salt: b64e(&salt_s),
            features: vec![FEATURE_FILES.to_string()],
        },
    )
    .await?;
    let peer_name = display_name(&state, &peer);
    let mut session = Session {
        stream,
        key: derive_session_key(&peer.pairing_key, &salt_c, &salt_s),
        addr: transport::listen_addr(peer_addr, port),
        features: Vec::new(),
    };

    loop {
        let wire = match tokio::time::timeout(IDLE_TIMEOUT, read_frame(&mut session.stream)).await {
            Ok(Ok(w)) => w,
            Ok(Err(AppError::Io(e))) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(())
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Ok(()),
        };
        let plain = match decrypt_wire(&session.key, &wire) {
            Ok(p) => p,
            // Decrypted fine but not a payload this version knows.
            Err(AppError::Json(_)) => {
                return Err(reject(&mut session, "unsupported", "unsupported payload").await)
            }
            Err(_) => return Err(reject(&mut session, "auth_failed", "decryption failed").await),
        };
        let item = match plain {
            Plain::Clipboard { id, text, .. } => {
                if text.len() > MAX_TEXT_BYTES {
                    return Err(reject(&mut session, "too_large", "text too large").await);
                }
                let ack = encrypt_plain(&session.key, &Plain::Ack { id: id.clone() })?;
                write_frame(&mut session.stream, &ack).await?;
                HistoryItem::received(id, &peer, &peer_name, text, None)
            }
            Plain::FileStart {
                id,
                name,
                size,
                mime,
                ..
            } => {
                let file =
                    transfer::receive(&app, &mut session, id.clone(), name, size, mime).await?;
                HistoryItem::received(id, &peer, &peer_name, String::new(), Some(file))
            }
            Plain::Unpair => {
                log::info!("{} removed the pairing", peer.name);
                forget_peer_in_background(&app, &peer.device_id);
                return Ok(());
            }
            Plain::Ping => continue,
            other => return Err(AppError::protocol(format!("unexpected payload {other:?}"))),
        };
        deliver_received(&app, &state, item);
        transport::remember_peer_addr(&app, &peer.device_id, session.addr);
    }
}

/// The peer's current name when discovery sees it, else the name stored at pairing.
fn display_name(state: &AppState, peer: &PeerRecord) -> String {
    state
        .discovered
        .lock()
        .unwrap()
        .get(&peer.device_id)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| peer.name.clone())
}

fn deliver_received(app: &AppHandle, state: &AppState, item: HistoryItem) {
    let (write_cb, notify) = {
        let s = state.settings.lock().unwrap();
        (s.write_received_to_clipboard, s.notify_on_receive)
    };
    state.record_history(app, item.clone());
    // Received files stay files; only text goes to the clipboard.
    if write_cb && item.file.is_none() {
        let _ = state
            .clipboard_tx
            .send(ClipboardCmd::Write(item.text.clone()));
    }
    let _ = app.emit("clipboard-received", &item);
    if notify {
        let body: String = match &item.file {
            Some(file) => file.name.clone(),
            None => item.text.chars().take(120).collect(),
        };
        let notification = app
            .notification()
            .builder()
            .title(format!("Received from {}", item.peer_name))
            .body(body);
        // Android status bar icon (a drawable name) and its tint; the plugin
        // falls back to the generic info icon otherwise. Desktop uses the app icon.
        #[cfg(mobile)]
        let notification = notification.icon("ic_notification").icon_color("#2563eb");
        let _ = notification.show();
    }
}
