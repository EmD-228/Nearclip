//! TCP listener and outbound connections. Every connection starts with one
//! plaintext frame that selects the flow: pairing or an encrypted session.

use std::net::SocketAddr;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::net::{TcpListener, TcpStream};

use crate::error::{AppError, Result};
use crate::protocol::{error_frame, read_frame, write_frame, Wire, DEFAULT_PORT};
use crate::state::AppState;
use crate::{pairing, session};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(10);

/// Binds the fixed default port, falling back to an ephemeral one when busy
/// (typically a second instance on the same machine).
pub async fn bind() -> Result<TcpListener> {
    match TcpListener::bind(("0.0.0.0", DEFAULT_PORT)).await {
        Ok(l) => Ok(l),
        Err(e) => {
            log::warn!("port {DEFAULT_PORT} unavailable ({e}), using an ephemeral port");
            Ok(TcpListener::bind(("0.0.0.0", 0)).await?)
        }
    }
}

pub fn start_listener(app: AppHandle, listener: TcpListener) {
    tauri::async_runtime::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = handle_conn(app, stream, addr).await {
                            log::debug!("connection from {addr} ended: {e}");
                        }
                    });
                }
                Err(e) => {
                    log::error!("accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    });
}

async fn handle_conn(app: AppHandle, mut stream: TcpStream, peer_addr: SocketAddr) -> Result<()> {
    let _ = stream.set_nodelay(true);
    let first = tokio::time::timeout(FIRST_FRAME_TIMEOUT, read_frame(&mut stream)).await??;
    match first {
        Wire::PairRequest { .. } => pairing::respond(app, stream, peer_addr, first).await,
        Wire::SessionInit { .. } => session::serve(app, stream, peer_addr, first).await,
        other => {
            let _ = write_frame(
                &mut stream,
                &error_frame("bad_request", "unexpected first frame"),
            )
            .await;
            Err(AppError::protocol(format!(
                "unexpected first frame {other:?}"
            )))
        }
    }
}

pub async fn connect(addr: SocketAddr) -> Result<TcpStream> {
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| AppError::DeviceUnavailable)?
        .map_err(|_| AppError::DeviceUnavailable)?;
    let _ = stream.set_nodelay(true);
    Ok(stream)
}

/// Current address for a device: live mDNS data first, last known address otherwise.
pub fn resolve_peer_addr(state: &AppState, device_id: &str) -> Option<SocketAddr> {
    if let Some(d) = state.discovered.lock().unwrap().get(device_id) {
        if !d.is_stale() {
            if let Some(addr) = d.best_addr() {
                return Some(addr);
            }
        }
    }
    state
        .peers
        .lock()
        .unwrap()
        .get(device_id)
        .and_then(|p| p.last_seen_addr)
}

/// Where a peer can be reached back: the IP it connected from, on the
/// listening port it advertised (its connection's source port is ephemeral).
pub fn listen_addr(conn: SocketAddr, advertised_port: u16) -> SocketAddr {
    let port = if advertised_port > 0 {
        advertised_port
    } else {
        conn.port()
    };
    SocketAddr::new(conn.ip(), port)
}

pub fn remember_peer_addr(app: &AppHandle, device_id: &str, addr: SocketAddr) {
    let state = app.state::<AppState>();
    let mut peers = state.peers.lock().unwrap();
    if let Some(p) = peers.get_mut(device_id) {
        if p.last_seen_addr != Some(addr) {
            p.last_seen_addr = Some(addr);
            let _ = crate::state::save_peers(app, &peers);
        }
    }
}
