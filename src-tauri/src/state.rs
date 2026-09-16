//! Shared application state and persistence via tauri-plugin-store.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::AtomicU16;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mdns_sd::ServiceDaemon;
use serde::{Deserialize, Serialize};
#[cfg(desktop)]
use tauri::menu::CheckMenuItem;
use tauri::{AppHandle, Manager};
#[cfg(desktop)]
use tauri::Wry;
use tauri_plugin_store::StoreExt;

use crate::clipboard::ClipboardCmd;
use crate::error::Result;
use crate::identity::Identity;
use crate::pairing::{PairToken, PairingHandle};

pub const HISTORY_CAP: usize = 200;
const HISTORY_PERSISTED: usize = 100;
const STALE_AFTER_SECS: u64 = 90;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub device_name: String,
    pub auto_sync: bool,
    pub write_received_to_clipboard: bool,
    pub notify_on_receive: bool,
    pub close_to_tray: bool,
    pub autostart: bool,
    /// mDNS discovery. Off by default: unreliable on many Wi-Fi networks.
    #[serde(default)]
    pub discovery: bool,
}

impl Settings {
    pub fn defaults() -> Self {
        // Phones report "localhost" as hostname; pick something recognizable instead.
        let host = if cfg!(target_os = "android") {
            "Android phone".to_string()
        } else if cfg!(target_os = "ios") {
            "iPhone".to_string()
        } else {
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .map(|h| h.trim_end_matches(".local").to_string())
                .filter(|h| !h.is_empty())
                .unwrap_or_else(|| "My device".to_string())
        };
        Settings {
            device_name: host,
            auto_sync: false,
            write_received_to_clipboard: true,
            notify_on_receive: true,
            close_to_tray: true,
            autostart: false,
            discovery: false,
        }
    }
}

/// How a pairing was established; shown instead of an online/offline state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PairedVia {
    Discovery,
    #[default]
    Address,
    Qr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerRecord {
    pub device_id: String,
    pub name: String,
    pub id_pk: [u8; 32],
    pub pairing_key: [u8; 32],
    pub paired_at: i64,
    pub last_seen_addr: Option<SocketAddr>,
    #[serde(default)]
    pub via: PairedVia,
}

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub addrs: Vec<IpAddr>,
    pub port: u16,
    pub last_seen: Instant,
}

impl DiscoveredDevice {
    /// Prefer IPv4, then any non-link-local IPv6, then anything.
    pub fn best_addr(&self) -> Option<SocketAddr> {
        let ip = self
            .addrs
            .iter()
            .find(|a| a.is_ipv4())
            .or_else(|| {
                self.addrs.iter().find(|a| match a {
                    IpAddr::V6(v6) => !v6.is_unicast_link_local(),
                    _ => false,
                })
            })
            .or_else(|| self.addrs.first())?;
        Some(SocketAddr::new(*ip, self.port))
    }

    pub fn is_stale(&self) -> bool {
        self.last_seen.elapsed().as_secs() > STALE_AFTER_SECS
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub device_id: String,
    pub name: String,
    pub paired: bool,
    pub addr: Option<String>,
    /// Set for paired devices only.
    pub via: Option<PairedVia>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Sent,
    Received,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub direction: Direction,
    pub peer_id: String,
    pub peer_name: String,
    pub text: String,
    pub ts_ms: i64,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub device_id: String,
    pub ok: bool,
    pub error: Option<String>,
}

pub struct AppState {
    pub identity: Identity,
    pub settings: Mutex<Settings>,
    pub peers: Mutex<HashMap<String, PeerRecord>>,
    pub discovered: Mutex<HashMap<String, DiscoveredDevice>>,
    pub history: Mutex<VecDeque<HistoryItem>>,
    pub pairings: Mutex<HashMap<String, PairingHandle>>,
    /// Token of the QR code currently displayed, if any (one-time, short-lived).
    pub pair_token: Mutex<Option<PairToken>>,
    pub listen_port: AtomicU16,
    pub clipboard_tx: std::sync::mpsc::Sender<ClipboardCmd>,
    pub discovery: Mutex<Option<ServiceDaemon>>,
    #[cfg(desktop)]
    pub tray_autosync: Mutex<Option<CheckMenuItem<Wry>>>,
}

impl AppState {
    pub fn device_views(&self) -> Vec<DeviceView> {
        let peers = self.peers.lock().unwrap();
        let discovered = self.discovered.lock().unwrap();
        let mut out: Vec<DeviceView> = Vec::new();

        for (id, peer) in peers.iter() {
            let disc = discovered.get(id).filter(|d| !d.is_stale());
            out.push(DeviceView {
                device_id: id.clone(),
                name: disc
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| peer.name.clone()),
                paired: true,
                addr: disc
                    .and_then(|d| d.best_addr())
                    .or(peer.last_seen_addr)
                    .map(|a| a.to_string()),
                via: Some(peer.via),
            });
        }
        for (id, d) in discovered.iter() {
            if peers.contains_key(id) || d.is_stale() {
                continue;
            }
            out.push(DeviceView {
                device_id: id.clone(),
                name: d.name.clone(),
                paired: false,
                addr: d.best_addr().map(|a| a.to_string()),
                via: None,
            });
        }
        out.sort_by(|a, b| {
            b.paired
                .cmp(&a.paired)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        out
    }

    pub fn push_history(&self, item: HistoryItem) {
        let mut h = self.history.lock().unwrap();
        h.push_front(item);
        while h.len() > HISTORY_CAP {
            h.pop_back();
        }
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------- persistence ----------

const SETTINGS_FILE: &str = "settings.json";
const PEERS_FILE: &str = "peers.json";
const HISTORY_FILE: &str = "history.json";

pub fn load_settings(app: &AppHandle) -> Settings {
    app.store(SETTINGS_FILE)
        .ok()
        .and_then(|s| s.get("settings"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_else(Settings::defaults)
}

pub fn save_settings(app: &AppHandle, settings: &Settings) -> Result<()> {
    let store = app.store(SETTINGS_FILE)?;
    store.set("settings", serde_json::to_value(settings)?);
    store.save()?;
    Ok(())
}

pub fn load_peers(app: &AppHandle) -> HashMap<String, PeerRecord> {
    let list: Vec<PeerRecord> = app
        .store(PEERS_FILE)
        .ok()
        .and_then(|s| s.get("peers"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    list.into_iter().map(|p| (p.device_id.clone(), p)).collect()
}

/// Removes a pairing, persists the change and refreshes the device list.
pub fn forget_peer(app: &AppHandle, device_id: &str) -> Option<PeerRecord> {
    let state = app.state::<AppState>();
    let removed = {
        let mut peers = state.peers.lock().unwrap();
        let removed = peers.remove(device_id);
        if removed.is_some() {
            if let Err(e) = save_peers(app, &peers) {
                log::error!("saving peers failed: {e}");
            }
        }
        removed
    };
    if removed.is_some() {
        crate::discovery::emit_devices(app);
    }
    removed
}

pub fn save_peers(app: &AppHandle, peers: &HashMap<String, PeerRecord>) -> Result<()> {
    let store = app.store(PEERS_FILE)?;
    let list: Vec<&PeerRecord> = peers.values().collect();
    store.set("peers", serde_json::to_value(list)?);
    store.save()?;
    Ok(())
}

pub fn load_history(app: &AppHandle) -> VecDeque<HistoryItem> {
    let list: Vec<HistoryItem> = app
        .store(HISTORY_FILE)
        .ok()
        .and_then(|s| s.get("history"))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    list.into_iter().collect()
}

pub fn save_history(app: &AppHandle, history: &VecDeque<HistoryItem>) -> Result<()> {
    let store = app.store(HISTORY_FILE)?;
    let list: Vec<&HistoryItem> = history.iter().take(HISTORY_PERSISTED).collect();
    store.set("history", serde_json::to_value(list)?);
    store.save()?;
    Ok(())
}
