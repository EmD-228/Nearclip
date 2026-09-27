//! Whether each paired device can be reached right now.
//!
//! Pairing survives restarts and network changes, so a paired device stays in
//! the list long after it left the network, and the user only finds out when a
//! send fails. Reachability is collected from everything that already proves
//! it — a session opened in either direction, a device mDNS can currently see —
//! and only devices nothing has been heard from get an actual probe, every
//! [`CHECK_EVERY`].

use std::net::SocketAddr;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::task::JoinSet;

use crate::session::within;
use crate::state::{now_ms, AppState, PeerRecord};
use crate::{discovery, session, transport};

/// Cheap enough to run forever on a phone, short enough that the list is right
/// by the time the user looks at it.
const CHECK_EVERY: Duration = Duration::from_secs(20);

/// A device that has not completed a handshake by then counts as unreachable;
/// the next round tries again.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Rounds a device must fail in a row before it is shown as disconnected. Two
/// rounds ride out a Wi-Fi hiccup without an extra connection attempt.
const MISSES_BEFORE_OFFLINE: u8 = 2;

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if check_all(&app).await {
                discovery::emit_devices(&app);
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Notes a device that just answered at `addr`, from a session opened in either
/// direction: its address is remembered and it counts as reached.
pub fn note_reached(app: &AppHandle, device_id: &str, addr: SocketAddr) {
    transport::remember_peer_addr(app, device_id, addr);
    if record_contact(app, device_id) {
        discovery::emit_devices(app);
    }
}

/// Marks a device as reached. Returns true when that changes what the device
/// list shows, which only the callers that can emit an event care about.
fn record_contact(app: &AppHandle, device_id: &str) -> bool {
    let state = app.state::<AppState>();
    let mut presence = state.presence.lock().unwrap();
    let entry = presence.entry(device_id.to_string()).or_default();
    let changed = entry.online != Some(true);
    entry.online = Some(true);
    entry.last_seen_ms = Some(now_ms());
    entry.misses = 0;
    changed
}

/// Probes every paired device nothing has been heard from since the last round.
/// Returns true when any device changed state, which the caller turns into a
/// `devices-changed` event.
pub async fn check_all(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let peers: Vec<PeerRecord> = state.peers.lock().unwrap().values().cloned().collect();
    let mut changed = false;

    let mut probes = JoinSet::new();
    for peer in peers {
        // A device announcing itself on mDNS, or one we exchanged with since
        // the last round, has already answered the question.
        if discovery::seen_recently(&state, &peer.device_id)
            || answered_recently(&state, &peer.device_id)
        {
            changed |= record_contact(app, &peer.device_id);
            continue;
        }
        let app = app.clone();
        // A device that answers is recorded by `open_session`; only the ones
        // that stay silent come back here.
        probes.spawn(async move { (!probe(&app, &peer).await).then_some(peer.device_id) });
    }

    for device_id in probes.join_all().await.into_iter().flatten() {
        changed |= record_miss(app, &device_id);
    }

    // A device unpaired while the probes ran leaves no state behind.
    let paired = state.peers.lock().unwrap();
    state
        .presence
        .lock()
        .unwrap()
        .retain(|id, _| paired.contains_key(id));
    changed
}

/// One handshake, which also refreshes the device's address as a side effect:
/// `open_session` races every address the device may have moved to.
async fn probe(app: &AppHandle, peer: &PeerRecord) -> bool {
    match within(PROBE_TIMEOUT, session::open_session(app, peer)).await {
        Ok(_session) => true,
        Err(e) => {
            log::debug!("{} did not answer: {e}", peer.device_id);
            false
        }
    }
}

fn answered_recently(state: &AppState, device_id: &str) -> bool {
    let cutoff = now_ms() - CHECK_EVERY.as_millis() as i64;
    state
        .presence
        .lock()
        .unwrap()
        .get(device_id)
        .and_then(|p| p.last_seen_ms)
        .is_some_and(|seen| seen > cutoff)
}

fn record_miss(app: &AppHandle, device_id: &str) -> bool {
    let state = app.state::<AppState>();
    let mut presence = state.presence.lock().unwrap();
    let entry = presence.entry(device_id.to_string()).or_default();
    entry.misses = entry.misses.saturating_add(1);
    // Only the round that reaches the limit changes anything: a contact resets
    // the counter, and `saturating_add` keeps a silent device past it.
    if entry.misses != MISSES_BEFORE_OFFLINE {
        return false;
    }
    entry.online = Some(false);
    true
}
