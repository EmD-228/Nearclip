//! Pairing: X25519 key agreement bound to Ed25519 identities, verified by a
//! 6-digit short authentication string (SAS) that both users compare.
//!
//! ```text
//! A (initiator)                              B (responder)
//! PairRequest  { id_pk_A, commit(eph_A, nonce_A) }  -->
//!                                       <--  PairResponse { id_pk_B, eph_B, nonce_B }
//! PairReveal   { eph_A, nonce_A, sig_A(T) }          -->   (B checks commit + sig)
//!                                       <--  PairSig { sig_B(T) }   (A checks sig)
//! both derive pairing_key + SAS, show SAS, then exchange Enc(PairAck) with pairing_key
//! ```

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::crypto::{commit, derive_pairing, pairing_transcript, random_bytes};
use crate::error::{AppError, Result};
use crate::identity::{device_id_from_pk, verify};
use crate::protocol::{
    b64d_array, b64e, decrypt_wire, encrypt_plain, error_frame, read_frame, write_frame, Plain,
    Wire, DEFAULT_PORT, PROTO_VERSION,
};
use crate::state::{now_ms, save_peers, AppState, PairedVia, PeerRecord};
use crate::{discovery, transport, tray};

const PAIRING_TIMEOUT: Duration = Duration::from_secs(120);
const QR_TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
const QR_PREFIX: &str = "nearclip://pair?";

/// One-time secret embedded in the QR code a desktop displays. Presenting it
/// proves the initiator saw the screen, so neither side needs to compare a code.
pub struct PairToken {
    value: String,
    created: Instant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairQr {
    svg: String,
    /// How long the code stays valid, so the UI can show its expiry.
    ttl_ms: u64,
}

/// What the initiator learned from a scanned QR code: the displayed device's
/// identity key and the one-time token that proves the screen was seen.
struct QrAuth {
    pk: [u8; 32],
    token: String,
}

/// What a scanned QR code tells us about the device that displayed it.
struct QrTarget {
    addrs: Vec<SocketAddr>,
    id: String,
    auth: QrAuth,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Initiator,
    Responder,
}

pub struct PairingHandle {
    pub peer_name: String,
    pub code: Option<String>,
    confirm_tx: mpsc::Sender<bool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PairingRequestEvent {
    device_id: String,
    name: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PairingCodeEvent {
    device_id: String,
    name: String,
    code: String,
    role: Role,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PairingResultEvent {
    device_id: String,
    ok: bool,
    error: Option<String>,
}

/// Starts pairing with a discovered device. Progress is reported through events.
pub fn start(app: AppHandle, device_id: String) -> Result<()> {
    let state = app.state::<AppState>();
    let addr =
        transport::resolve_peer_addr(&state, &device_id).ok_or(AppError::DeviceUnavailable)?;
    let peer_name = state
        .discovered
        .lock()
        .unwrap()
        .get(&device_id)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "Unknown device".to_string());
    spawn_initiator(
        app,
        device_id.clone(),
        peer_name,
        Some(device_id),
        vec![addr],
        None,
        PairedVia::Discovery,
    )
}

/// Starts pairing with a device mDNS cannot see, given "ip" or "ip:port".
/// Until the peer answers, the typed address stands in for its device id in
/// the pairing events; `pairing-code` then carries the real id and name.
pub fn start_by_address(app: AppHandle, addr_str: &str) -> Result<()> {
    let addr_str = addr_str.trim();
    let addr = parse_peer_addr(addr_str)?;
    spawn_initiator(
        app,
        addr_str.to_string(),
        addr_str.to_string(),
        None,
        vec![addr],
        None,
        PairedVia::Address,
    )
}

/// Desktop side of QR pairing: a payload with our addresses, identity and a
/// fresh one-time token, plus its rendering as an SVG.
pub fn create_qr(app: &AppHandle) -> Result<PairQr> {
    let state = app.state::<AppState>();
    let addrs = transport::local_ipv4_addrs();
    if addrs.is_empty() {
        return Err(AppError::msg(
            "No network address found. Connect to Wi-Fi first.",
        ));
    }
    let port = state.listen_port.load(Ordering::Relaxed);
    let token = hex::encode(random_bytes::<16>());
    let payload = format!(
        "{QR_PREFIX}a={}&id={}&pk={}&t={}",
        addrs
            .iter()
            .map(|ip| format!("{ip}:{port}"))
            .collect::<Vec<_>>()
            .join(","),
        state.identity.device_id,
        hex::encode(state.identity.public_key()),
        token,
    );
    let svg = qrcode::QrCode::new(payload.as_bytes())
        .map_err(|e| AppError::msg(format!("QR code: {e}")))?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    *state.pair_token.lock().unwrap() = Some(PairToken {
        value: token,
        created: Instant::now(),
    });
    Ok(PairQr {
        svg,
        ttl_ms: QR_TOKEN_TTL.as_millis() as u64,
    })
}

/// Phone side of QR pairing: connect to the displayed device, check its
/// identity against the QR and let both sides confirm automatically.
pub fn start_by_qr(app: AppHandle, payload: &str) -> Result<()> {
    let target = parse_qr(payload)?;
    spawn_initiator(
        app,
        target.id.clone(),
        "Computer".to_string(),
        Some(target.id),
        target.addrs,
        Some(target.auth),
        PairedVia::Qr,
    )
}

/// The recorded pairing method: QR only when the token was actually verified;
/// a QR attempt that fell back to the code comparison counts as by-address.
fn paired_via(intent: PairedVia, verified_by_qr: bool) -> PairedVia {
    match (verified_by_qr, intent) {
        (true, _) => PairedVia::Qr,
        (false, PairedVia::Qr) => PairedVia::Address,
        (false, other) => other,
    }
}

fn parse_qr(payload: &str) -> Result<QrTarget> {
    let query = payload
        .strip_prefix(QR_PREFIX)
        .ok_or_else(|| AppError::msg("This is not a NearClip pairing code"))?;
    let mut addrs = Vec::new();
    let (mut id, mut pk, mut token) = (None, None, None);
    for (k, v) in query.split('&').filter_map(|kv| kv.split_once('=')) {
        match k {
            "a" => addrs = v.split(',').filter_map(|a| a.parse().ok()).collect(),
            "id" => id = Some(v.to_string()),
            "pk" => {
                pk = hex::decode(v)
                    .ok()
                    .and_then(|b| <[u8; 32]>::try_from(b).ok())
            }
            "t" => token = Some(v.to_string()),
            _ => {}
        }
    }
    match (id, pk, token) {
        (Some(id), Some(pk), Some(token)) if !addrs.is_empty() && device_id_from_pk(&pk) == id => {
            Ok(QrTarget {
                addrs,
                id,
                auth: QrAuth { pk, token },
            })
        }
        _ => Err(AppError::msg("This pairing code is incomplete or damaged")),
    }
}

/// Consumes the displayed token if `presented` matches and it is still fresh.
fn take_token(state: &AppState, presented: &str) -> bool {
    let mut slot = state.pair_token.lock().unwrap();
    let valid = slot
        .as_ref()
        .is_some_and(|t| t.value == presented && t.created.elapsed() < QR_TOKEN_TTL);
    if valid {
        *slot = None;
    }
    valid
}

/// "ip:port" or bare "ip" (default port).
fn parse_peer_addr(s: &str) -> Result<SocketAddr> {
    s.parse::<SocketAddr>()
        .or_else(|_| {
            s.parse::<std::net::IpAddr>()
                .map(|ip| SocketAddr::new(ip, DEFAULT_PORT))
        })
        .map_err(|_| AppError::msg("Enter an address like 192.168.1.20 or 192.168.1.20:47821"))
}

/// Registers the pairing handle under `key` and runs the initiator flow.
/// `expected_id` pins the peer identity when it is already known from mDNS.
fn spawn_initiator(
    app: AppHandle,
    mut key: String,
    peer_name: String,
    expected_id: Option<String>,
    addrs: Vec<SocketAddr>,
    qr: Option<QrAuth>,
    via: PairedVia,
) -> Result<()> {
    let state = app.state::<AppState>();
    let (confirm_tx, confirm_rx) = mpsc::channel(1);
    {
        let mut pairings = state.pairings.lock().unwrap();
        if pairings.contains_key(&key) {
            return Err(AppError::msg(
                "A pairing with this device is already in progress",
            ));
        }
        pairings.insert(
            key.clone(),
            PairingHandle {
                peer_name,
                code: None,
                confirm_tx,
            },
        );
    }

    tauri::async_runtime::spawn(async move {
        let res = tokio::time::timeout(
            PAIRING_TIMEOUT,
            run_initiator(
                &app,
                &mut key,
                expected_id.as_deref(),
                &addrs,
                qr.as_ref(),
                via,
                confirm_rx,
            ),
        )
        .await
        .unwrap_or(Err(AppError::Timeout));
        finish(&app, &key, res);
    });
    Ok(())
}

/// Handles an incoming `PairRequest` on an accepted connection.
pub async fn respond(
    app: AppHandle,
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    first: Wire,
) -> Result<()> {
    let Wire::PairRequest {
        v,
        device_id: peer_id,
        name: peer_name,
        id_pk,
        commit: commit_a,
        port: peer_port,
        token,
        via: peer_via,
    } = first
    else {
        return Err(AppError::protocol("expected PairRequest"));
    };
    if v != PROTO_VERSION {
        let _ = write_frame(
            &mut stream,
            &error_frame("version", "unsupported protocol version"),
        )
        .await;
        return Err(AppError::protocol("version mismatch"));
    }

    // Validate the request before bothering the user with a dialog.
    let peer_pk: Option<[u8; 32]> = b64d_array(&id_pk).ok();
    let commit_a: Option<[u8; 32]> = b64d_array(&commit_a).ok();
    let (peer_pk, commit_a) = match (peer_pk, commit_a) {
        (Some(pk), Some(c)) if device_id_from_pk(&pk) == peer_id => (pk, c),
        _ => {
            let _ = write_frame(
                &mut stream,
                &error_frame("identity", "device id does not match key"),
            )
            .await;
            return Err(AppError::Crypto);
        }
    };

    let state = app.state::<AppState>();
    // A valid QR token means the initiator saw our screen: no code to compare.
    let token_ok = token.is_some_and(|t| take_token(&state, &t));
    let (confirm_tx, confirm_rx) = mpsc::channel(1);
    let busy = state.pairings.lock().unwrap().contains_key(&peer_id);
    if busy {
        let _ = write_frame(
            &mut stream,
            &error_frame("busy", "pairing already in progress"),
        )
        .await;
        return Err(AppError::msg("pairing already in progress"));
    }
    {
        let mut pairings = state.pairings.lock().unwrap();
        pairings.insert(
            peer_id.clone(),
            PairingHandle {
                peer_name: peer_name.clone(),
                code: None,
                confirm_tx,
            },
        );
    }

    let _ = app.emit(
        "pairing-request",
        PairingRequestEvent {
            device_id: peer_id.clone(),
            name: peer_name.clone(),
        },
    );
    tray::show_main(&app);

    let res = tokio::time::timeout(
        PAIRING_TIMEOUT,
        run_responder(
            &app,
            stream,
            transport::listen_addr(peer_addr, peer_port),
            &peer_id,
            &peer_name,
            peer_pk,
            commit_a,
            token_ok,
            peer_via,
            confirm_rx,
        ),
    )
    .await
    .unwrap_or(Err(AppError::Timeout));
    finish(&app, &peer_id, res);
    Ok(())
}

/// Called from the UI once the user compared the codes (or cancelled).
pub fn confirm(app: &AppHandle, device_id: &str, accepted: bool) -> Result<()> {
    let state = app.state::<AppState>();
    let tx = state
        .pairings
        .lock()
        .unwrap()
        .get(device_id)
        .map(|h| h.confirm_tx.clone())
        .ok_or_else(|| AppError::msg("No pairing in progress with this device"))?;
    tx.try_send(accepted)
        .map_err(|_| AppError::msg("Pairing already confirmed"))
}

/// `key` is the pairing handle key: the peer's device id when known, otherwise
/// the typed address. It is switched to the real device id once the peer answers.
async fn run_initiator(
    app: &AppHandle,
    key: &mut String,
    expected_id: Option<&str>,
    addrs: &[SocketAddr],
    qr: Option<&QrAuth>,
    via: PairedVia,
    confirm_rx: mpsc::Receiver<bool>,
) -> Result<()> {
    let state = app.state::<AppState>();
    let my_id = state.identity.device_id.clone();
    let my_pk = state.identity.public_key();
    let my_name = state.settings.lock().unwrap().device_name.clone();

    let (mut stream, addr) = transport::connect_any(addrs).await?;

    let eph_secret = StaticSecret::from(random_bytes::<32>());
    let eph_a = PublicKey::from(&eph_secret).to_bytes();
    let nonce_a: [u8; 32] = random_bytes();

    write_frame(
        &mut stream,
        &Wire::PairRequest {
            v: PROTO_VERSION,
            device_id: my_id,
            name: my_name,
            id_pk: b64e(&my_pk),
            commit: b64e(&commit(&eph_a, &nonce_a)),
            port: state.listen_port.load(Ordering::Relaxed),
            token: qr.map(|q| q.token.clone()),
            via: Some(via),
        },
    )
    .await?;

    let (peer_id, peer_name, peer_pk, eph_b, nonce_b, token_ok) =
        match read_frame(&mut stream).await? {
            Wire::PairResponse {
                device_id,
                name,
                id_pk,
                eph,
                nonce,
                token_ok,
            } => (
                device_id,
                name,
                b64d_array::<32>(&id_pk)?,
                b64d_array::<32>(&eph)?,
                b64d_array::<32>(&nonce)?,
                token_ok,
            ),
            Wire::Error { code, msg } => return Err(AppError::msg(format!("{msg} ({code})"))),
            other => {
                return Err(AppError::protocol(format!(
                    "expected PairResponse, got {other:?}"
                )))
            }
        };
    if expected_id.is_some_and(|id| id != peer_id)
        || device_id_from_pk(&peer_pk) != peer_id
        || qr.is_some_and(|q| q.pk != peer_pk)
    {
        return Err(AppError::Crypto);
    }

    let transcript = pairing_transcript(&my_pk, &eph_a, &nonce_a, &peer_pk, &eph_b, &nonce_b);
    let sig_a = state.identity.sign(&transcript);
    write_frame(
        &mut stream,
        &Wire::PairReveal {
            eph: b64e(&eph_a),
            nonce: b64e(&nonce_a),
            sig: b64e(&sig_a),
        },
    )
    .await?;

    let sig_b: [u8; 64] = match read_frame(&mut stream).await? {
        Wire::PairSig { sig } => b64d_array(&sig)?,
        Wire::Error { code, msg } => return Err(AppError::msg(format!("{msg} ({code})"))),
        other => {
            return Err(AppError::protocol(format!(
                "expected PairSig, got {other:?}"
            )))
        }
    };
    verify(&peer_pk, &transcript, &sig_b)?;

    let shared = eph_secret
        .diffie_hellman(&PublicKey::from(eph_b))
        .to_bytes();
    let material = derive_pairing(&shared, &transcript);

    // Switch to the real id right before the UI learns it, so there is no
    // window where events carry an id the frontend does not know yet.
    if *key != peer_id {
        rekey(app, key, &peer_id)?;
        *key = peer_id.clone();
    }
    // Only trust the peer's token_ok when we actually presented a QR token.
    let verified_by_qr = qr.is_some() && token_ok;
    settle(
        app,
        key,
        &peer_name,
        &material.sas,
        Role::Initiator,
        verified_by_qr,
    )?;
    confirm_phase(stream, confirm_rx, &material.pairing_key).await?;

    save_peer(
        app,
        PeerRecord {
            device_id: peer_id,
            name: peer_name,
            id_pk: peer_pk,
            pairing_key: material.pairing_key,
            paired_at: now_ms(),
            last_seen_addr: Some(addr),
            via: paired_via(via, verified_by_qr),
        },
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_responder(
    app: &AppHandle,
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    peer_id: &str,
    peer_name: &str,
    peer_pk: [u8; 32],
    commit_a: [u8; 32],
    token_ok: bool,
    peer_via: Option<PairedVia>,
    confirm_rx: mpsc::Receiver<bool>,
) -> Result<()> {
    let state = app.state::<AppState>();
    let my_id = state.identity.device_id.clone();
    let my_pk = state.identity.public_key();
    let my_name = state.settings.lock().unwrap().device_name.clone();

    let eph_secret = StaticSecret::from(random_bytes::<32>());
    let eph_b = PublicKey::from(&eph_secret).to_bytes();
    let nonce_b: [u8; 32] = random_bytes();

    write_frame(
        &mut stream,
        &Wire::PairResponse {
            device_id: my_id,
            name: my_name,
            id_pk: b64e(&my_pk),
            eph: b64e(&eph_b),
            nonce: b64e(&nonce_b),
            token_ok,
        },
    )
    .await?;

    let (eph_a, nonce_a, sig_a) = match read_frame(&mut stream).await? {
        Wire::PairReveal { eph, nonce, sig } => (
            b64d_array::<32>(&eph)?,
            b64d_array::<32>(&nonce)?,
            b64d_array::<64>(&sig)?,
        ),
        Wire::Error { code, msg } => return Err(AppError::msg(format!("{msg} ({code})"))),
        other => {
            return Err(AppError::protocol(format!(
                "expected PairReveal, got {other:?}"
            )))
        }
    };
    if commit(&eph_a, &nonce_a) != commit_a {
        let _ = write_frame(&mut stream, &error_frame("commit", "commitment mismatch")).await;
        return Err(AppError::Crypto);
    }

    let transcript = pairing_transcript(&peer_pk, &eph_a, &nonce_a, &my_pk, &eph_b, &nonce_b);
    if verify(&peer_pk, &transcript, &sig_a).is_err() {
        let _ = write_frame(&mut stream, &error_frame("signature", "bad signature")).await;
        return Err(AppError::Crypto);
    }

    let sig_b = state.identity.sign(&transcript);
    write_frame(&mut stream, &Wire::PairSig { sig: b64e(&sig_b) }).await?;

    let shared = eph_secret
        .diffie_hellman(&PublicKey::from(eph_a))
        .to_bytes();
    let material = derive_pairing(&shared, &transcript);

    settle(
        app,
        peer_id,
        peer_name,
        &material.sas,
        Role::Responder,
        token_ok,
    )?;
    confirm_phase(stream, confirm_rx, &material.pairing_key).await?;

    save_peer(
        app,
        PeerRecord {
            device_id: peer_id.to_string(),
            name: peer_name.to_string(),
            id_pk: peer_pk,
            pairing_key: material.pairing_key,
            paired_at: now_ms(),
            last_seen_addr: Some(peer_addr),
            via: paired_via(peer_via.unwrap_or_default(), token_ok),
        },
    );
    Ok(())
}

/// Both users must confirm. Each side sends its encrypted `PairAck` as soon as
/// its user decides, and waits for the other side's.
async fn confirm_phase(
    stream: TcpStream,
    mut confirm_rx: mpsc::Receiver<bool>,
    key: &[u8; 32],
) -> Result<()> {
    let (mut rd, mut wr) = stream.into_split();
    let mut local: Option<bool> = None;
    let mut remote: Option<bool> = None;

    loop {
        if let (Some(l), Some(r)) = (local, remote) {
            return if l && r {
                Ok(())
            } else {
                Err(AppError::PairingRejected)
            };
        }
        tokio::select! {
            v = confirm_rx.recv(), if local.is_none() => {
                let accepted = v.unwrap_or(false);
                write_frame(&mut wr, &encrypt_plain(key, &Plain::PairAck { accepted })?).await?;
                local = Some(accepted);
                if !accepted {
                    return Err(AppError::PairingRejected);
                }
            }
            f = read_frame(&mut rd), if remote.is_none() => {
                match decrypt_wire(key, &f?)? {
                    Plain::PairAck { accepted } => {
                        remote = Some(accepted);
                        if !accepted {
                            return Err(AppError::PairingRejected);
                        }
                    }
                    other => return Err(AppError::protocol(format!("expected PairAck, got {other:?}"))),
                }
            }
        }
    }
}

/// Once the key material is derived: with a verified QR token both identities
/// are already proven, so this side confirms on its own; otherwise the user
/// gets the code to compare.
fn settle(
    app: &AppHandle,
    device_id: &str,
    peer_name: &str,
    sas: &str,
    role: Role,
    verified_by_qr: bool,
) -> Result<()> {
    if verified_by_qr {
        confirm(app, device_id, true)
    } else {
        publish_code(app, device_id, peer_name, sas, role);
        Ok(())
    }
}

fn publish_code(app: &AppHandle, device_id: &str, peer_name: &str, code: &str, role: Role) {
    let state = app.state::<AppState>();
    if let Some(h) = state.pairings.lock().unwrap().get_mut(device_id) {
        h.code = Some(code.to_string());
        h.peer_name = peer_name.to_string();
    }
    let _ = app.emit(
        "pairing-code",
        PairingCodeEvent {
            device_id: device_id.to_string(),
            name: peer_name.to_string(),
            code: code.to_string(),
            role,
        },
    );
}

fn save_peer(app: &AppHandle, peer: PeerRecord) {
    let state = app.state::<AppState>();
    let mut peers = state.peers.lock().unwrap();
    log::info!("paired with {} ({})", peer.name, peer.device_id);
    peers.insert(peer.device_id.clone(), peer);
    if let Err(e) = save_peers(app, &peers) {
        log::error!("saving peers failed: {e}");
    }
}

/// Moves an in-flight pairing handle from a provisional key (typed address)
/// to the peer's real device id, so `confirm` finds it under the id the UI has.
/// Refuses to clobber another pairing already running under that id.
fn rekey(app: &AppHandle, from: &str, to: &str) -> Result<()> {
    let state = app.state::<AppState>();
    let mut pairings = state.pairings.lock().unwrap();
    if pairings.contains_key(to) {
        return Err(AppError::msg(
            "A pairing with this device is already in progress",
        ));
    }
    if let Some(handle) = pairings.remove(from) {
        pairings.insert(to.to_string(), handle);
    }
    Ok(())
}

fn finish(app: &AppHandle, device_id: &str, res: Result<()>) {
    let state = app.state::<AppState>();
    state.pairings.lock().unwrap().remove(device_id);
    let error = res.as_ref().err().map(|e| e.to_string());
    if let Some(e) = &error {
        log::info!("pairing with {device_id} failed: {e}");
    }
    let _ = app.emit(
        "pairing-result",
        PairingResultEvent {
            device_id: device_id.to_string(),
            ok: res.is_ok(),
            error,
        },
    );
    discovery::emit_devices(app);
}

#[cfg(test)]
mod tests {
    //! End-to-end handshake over an in-memory duplex, without Tauri.
    use super::*;
    use crate::crypto;
    use crate::identity::Identity;
    use tokio::io::{AsyncRead, AsyncWrite};

    struct Side {
        identity: Identity,
    }

    async fn initiator<S: AsyncRead + AsyncWrite + Unpin>(
        me: &Side,
        s: &mut S,
    ) -> Result<crypto::PairingMaterial> {
        let my_pk = me.identity.public_key();
        let eph_secret = StaticSecret::from(random_bytes::<32>());
        let eph_a = PublicKey::from(&eph_secret).to_bytes();
        let nonce_a: [u8; 32] = random_bytes();
        write_frame(
            s,
            &Wire::PairRequest {
                v: PROTO_VERSION,
                device_id: me.identity.device_id.clone(),
                name: "A".into(),
                id_pk: b64e(&my_pk),
                commit: b64e(&commit(&eph_a, &nonce_a)),
                port: 0,
                token: None,
                via: None,
            },
        )
        .await?;
        let Wire::PairResponse {
            device_id,
            id_pk,
            eph,
            nonce,
            ..
        } = read_frame(s).await?
        else {
            panic!("expected PairResponse")
        };
        let peer_pk: [u8; 32] = b64d_array(&id_pk)?;
        assert_eq!(device_id_from_pk(&peer_pk), device_id);
        let eph_b: [u8; 32] = b64d_array(&eph)?;
        let nonce_b: [u8; 32] = b64d_array(&nonce)?;
        let t = pairing_transcript(&my_pk, &eph_a, &nonce_a, &peer_pk, &eph_b, &nonce_b);
        write_frame(
            s,
            &Wire::PairReveal {
                eph: b64e(&eph_a),
                nonce: b64e(&nonce_a),
                sig: b64e(&me.identity.sign(&t)),
            },
        )
        .await?;
        let Wire::PairSig { sig } = read_frame(s).await? else {
            panic!("expected PairSig")
        };
        verify(&peer_pk, &t, &b64d_array::<64>(&sig)?)?;
        let shared = eph_secret
            .diffie_hellman(&PublicKey::from(eph_b))
            .to_bytes();
        Ok(derive_pairing(&shared, &t))
    }

    async fn responder<S: AsyncRead + AsyncWrite + Unpin>(
        me: &Side,
        s: &mut S,
    ) -> Result<crypto::PairingMaterial> {
        let my_pk = me.identity.public_key();
        let Wire::PairRequest {
            id_pk, commit: c, ..
        } = read_frame(s).await?
        else {
            panic!("expected PairRequest")
        };
        let peer_pk: [u8; 32] = b64d_array(&id_pk)?;
        let commit_a: [u8; 32] = b64d_array(&c)?;
        let eph_secret = StaticSecret::from(random_bytes::<32>());
        let eph_b = PublicKey::from(&eph_secret).to_bytes();
        let nonce_b: [u8; 32] = random_bytes();
        write_frame(
            s,
            &Wire::PairResponse {
                device_id: me.identity.device_id.clone(),
                name: "B".into(),
                id_pk: b64e(&my_pk),
                eph: b64e(&eph_b),
                nonce: b64e(&nonce_b),
                token_ok: false,
            },
        )
        .await?;
        let Wire::PairReveal { eph, nonce, sig } = read_frame(s).await? else {
            panic!("expected PairReveal")
        };
        let eph_a: [u8; 32] = b64d_array(&eph)?;
        let nonce_a: [u8; 32] = b64d_array(&nonce)?;
        assert_eq!(commit(&eph_a, &nonce_a), commit_a, "commitment must match");
        let t = pairing_transcript(&peer_pk, &eph_a, &nonce_a, &my_pk, &eph_b, &nonce_b);
        verify(&peer_pk, &t, &b64d_array::<64>(&sig)?)?;
        write_frame(
            s,
            &Wire::PairSig {
                sig: b64e(&me.identity.sign(&t)),
            },
        )
        .await?;
        let shared = eph_secret
            .diffie_hellman(&PublicKey::from(eph_a))
            .to_bytes();
        Ok(derive_pairing(&shared, &t))
    }

    #[test]
    fn parse_qr_roundtrip_and_rejects_tampering() {
        let me = Identity::from_seed(random_bytes());
        let pk = hex::encode(me.public_key());
        let payload = format!(
            "{QR_PREFIX}a=192.168.1.5:47821,10.0.0.2:47821&id={}&pk={pk}&t=abc123",
            me.device_id
        );
        let t = parse_qr(&payload).unwrap();
        assert_eq!(t.addrs.len(), 2);
        assert_eq!(t.id, me.device_id);
        assert_eq!(t.auth.pk, me.public_key());
        assert_eq!(t.auth.token, "abc123");

        // Wrong scheme, or an id that does not match the key, is refused.
        assert!(parse_qr("https://example.com").is_err());
        let other = Identity::from_seed(random_bytes());
        let forged = payload.replace(&me.device_id, &other.device_id);
        assert!(parse_qr(&forged).is_err());
    }

    #[test]
    fn parse_peer_addr_accepts_ip_with_or_without_port() {
        assert_eq!(
            parse_peer_addr("192.168.1.20").unwrap().to_string(),
            format!("192.168.1.20:{DEFAULT_PORT}")
        );
        assert_eq!(
            parse_peer_addr("192.168.1.20:5000").unwrap().to_string(),
            "192.168.1.20:5000"
        );
        assert!(parse_peer_addr("not an address").is_err());
        assert!(parse_peer_addr("192.168.1.20:99999").is_err());
    }

    #[tokio::test]
    async fn full_handshake_yields_same_key_and_sas() {
        let a = Side {
            identity: Identity::from_seed(random_bytes()),
        };
        let b = Side {
            identity: Identity::from_seed(random_bytes()),
        };
        let (mut sa, mut sb) = tokio::io::duplex(8192);
        let (ra, rb) = tokio::join!(initiator(&a, &mut sa), responder(&b, &mut sb));
        let (ma, mb) = (ra.unwrap(), rb.unwrap());
        assert_eq!(ma.pairing_key, mb.pairing_key);
        assert_eq!(ma.sas, mb.sas);

        // The derived key works for the PairAck exchange.
        let wire = encrypt_plain(&ma.pairing_key, &Plain::PairAck { accepted: true }).unwrap();
        assert_eq!(
            decrypt_wire(&mb.pairing_key, &wire).unwrap(),
            Plain::PairAck { accepted: true }
        );
    }
}
