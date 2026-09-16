//! Encrypted clipboard sessions between paired devices. One connection per send.

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
    Wire, MAX_TEXT_BYTES, PROTO_VERSION,
};
use crate::state::{
    forget_peer, now_ms, save_history, AppState, Direction, HistoryItem, PeerRecord, SendResult,
};
use crate::transport;

const SEND_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ClipboardSentEvent {
    item: HistoryItem,
    results: Vec<SendResult>,
}

/// Sends `text` to one paired device (`target` = device id) or to every paired
/// device (`target` = "all"). Records one history item and emits `clipboard-sent`.
pub async fn send_text(app: AppHandle, target: String, text: String) -> Vec<SendResult> {
    let state = app.state::<AppState>();
    let targets: Vec<PeerRecord> = {
        let peers = state.peers.lock().unwrap();
        if target == "all" {
            peers.values().cloned().collect()
        } else {
            peers.get(&target).cloned().into_iter().collect()
        }
    };
    if targets.is_empty() {
        let error = if target_is_all(&target) {
            "No paired devices yet"
        } else {
            "This device is not paired"
        };
        return vec![SendResult {
            device_id: target,
            ok: false,
            error: Some(error.to_string()),
        }];
    }

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
    let mut results = Vec::with_capacity(targets.len());
    while let Some(r) = set.join_next().await {
        if let Ok(r) = r {
            results.push(r);
        }
    }

    let peer_name = if target_is_all(&target) {
        "All paired devices".to_string()
    } else {
        targets[0].name.clone()
    };
    let item = HistoryItem {
        id: item_id,
        direction: Direction::Sent,
        peer_id: target,
        peer_name,
        text,
        ts_ms: now_ms(),
        ok: results.iter().all(|r| r.ok),
    };
    state.push_history(item.clone());
    if let Err(e) = save_history(&app, &state.history.lock().unwrap()) {
        log::warn!("saving history failed: {e}");
    }
    let _ = app.emit(
        "clipboard-sent",
        ClipboardSentEvent {
            item,
            results: results.clone(),
        },
    );
    results
}

fn target_is_all(target: &str) -> bool {
    target == "all"
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
async fn open_session(
    app: &AppHandle,
    peer: &PeerRecord,
) -> Result<(TcpStream, [u8; 32], SocketAddr)> {
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

    let salt_s: [u8; 32] = match read_frame(&mut stream).await? {
        Wire::SessionAck { device_id, salt } => {
            if device_id != peer.device_id {
                return Err(AppError::protocol("responder identity mismatch"));
            }
            b64d_array(&salt)?
        }
        Wire::Error { code, .. } if code == "unknown_peer" => {
            // The other side forgot this pairing (unpaired while we were away):
            // drop it here too so the lists agree again.
            forget_peer_in_background(app, &peer.device_id);
            return Err(AppError::msg(
                "The other device removed this pairing. Pair again.",
            ));
        }
        Wire::Error { code, msg } => return Err(AppError::msg(format!("{msg} ({code})"))),
        other => {
            return Err(AppError::protocol(format!(
                "expected SessionAck, got {other:?}"
            )))
        }
    };
    let key = derive_session_key(&peer.pairing_key, &salt_c, &salt_s);
    Ok((stream, key, addr))
}

pub async fn send_text_to_peer(
    app: &AppHandle,
    peer: &PeerRecord,
    item_id: &str,
    text: &str,
) -> Result<()> {
    let my_id = app.state::<AppState>().identity.device_id.clone();
    let addr = tokio::time::timeout(SEND_TIMEOUT, async {
        let (mut stream, key, addr) = open_session(app, peer).await?;
        write_frame(
            &mut stream,
            &encrypt_plain(
                &key,
                &Plain::Clipboard {
                    id: item_id.to_string(),
                    ts: now_ms(),
                    sender: my_id,
                    text: text.to_string(),
                },
            )?,
        )
        .await?;
        match decrypt_wire(&key, &read_frame(&mut stream).await?)? {
            Plain::Ack { id } if id == item_id => Ok(addr),
            other => Err(AppError::protocol(format!("expected Ack, got {other:?}"))),
        }
    })
    .await??;

    transport::remember_peer_addr(app, &peer.device_id, addr);
    Ok(())
}

/// Best effort: tells a peer we removed the pairing so it forgets us too.
pub async fn notify_unpair(app: AppHandle, peer: PeerRecord) {
    let sent = tokio::time::timeout(SEND_TIMEOUT, async {
        let (mut stream, key, _) = open_session(&app, &peer).await?;
        write_frame(&mut stream, &encrypt_plain(&key, &Plain::Unpair)?).await
    })
    .await
    .map_err(AppError::from)
    .and_then(|r| r);
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
        },
    )
    .await?;
    let key = derive_session_key(&peer.pairing_key, &salt_c, &salt_s);

    loop {
        let wire = match tokio::time::timeout(IDLE_TIMEOUT, read_frame(&mut stream)).await {
            Ok(Ok(w)) => w,
            Ok(Err(AppError::Io(e))) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(())
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Ok(()),
        };
        let plain = match decrypt_wire(&key, &wire) {
            Ok(p) => p,
            Err(e) => {
                let _ = write_frame(
                    &mut stream,
                    &error_frame("auth_failed", "decryption failed"),
                )
                .await;
                return Err(e);
            }
        };
        match plain {
            Plain::Clipboard { id, text, .. } => {
                if text.len() > MAX_TEXT_BYTES {
                    let _ =
                        write_frame(&mut stream, &error_frame("too_large", "text too large")).await;
                    return Err(AppError::protocol("text too large"));
                }
                let peer_name = state
                    .discovered
                    .lock()
                    .unwrap()
                    .get(&peer.device_id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| peer.name.clone());
                let item = HistoryItem {
                    id: id.clone(),
                    direction: Direction::Received,
                    peer_id: peer.device_id.clone(),
                    peer_name: peer_name.clone(),
                    text: text.clone(),
                    ts_ms: now_ms(),
                    ok: true,
                };
                deliver_received(&app, &state, item, text, &peer_name);
                transport::remember_peer_addr(
                    &app,
                    &peer.device_id,
                    transport::listen_addr(peer_addr, port),
                );
                write_frame(&mut stream, &encrypt_plain(&key, &Plain::Ack { id })?).await?;
            }
            Plain::Unpair => {
                log::info!("{} removed the pairing", peer.name);
                forget_peer_in_background(&app, &peer.device_id);
                return Ok(());
            }
            Plain::Ping => {}
            other => return Err(AppError::protocol(format!("unexpected payload {other:?}"))),
        }
    }
}

fn deliver_received(
    app: &AppHandle,
    state: &AppState,
    item: HistoryItem,
    text: String,
    peer_name: &str,
) {
    let (write_cb, notify) = {
        let s = state.settings.lock().unwrap();
        (s.write_received_to_clipboard, s.notify_on_receive)
    };
    state.push_history(item.clone());
    if let Err(e) = save_history(app, &state.history.lock().unwrap()) {
        log::warn!("saving history failed: {e}");
    }
    if write_cb {
        let _ = state.clipboard_tx.send(ClipboardCmd::Write(text.clone()));
    }
    let _ = app.emit("clipboard-received", &item);
    if notify {
        let preview: String = text.chars().take(120).collect();
        let _ = app
            .notification()
            .builder()
            .title(format!("Received from {peer_name}"))
            .body(preview)
            .show();
    }
}
