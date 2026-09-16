//! Tauri commands: thin wrappers over state, pairing and sessions.

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::clipboard::ClipboardCmd;
use crate::error::{AppError, Result};
use crate::protocol::MAX_TEXT_BYTES;
use crate::state::{
    forget_peer, save_history, save_settings, AppState, DeviceView, HistoryItem, SendResult,
    Settings,
};
use crate::{discovery, pairing, session, transport, tray};

const MAX_NAME_LEN: usize = 48;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityView {
    device_id: String,
    name: String,
    fingerprint: String,
    public_key: String,
    /// std::env::consts::OS: "macos", "windows", "linux", "android", "ios".
    platform: &'static str,
    /// Tray, autostart, close-to-tray and clipboard auto-sync exist only on desktop.
    desktop: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenInfo {
    port: u16,
    addrs: Vec<String>,
}

#[tauri::command]
pub fn get_identity(state: State<'_, AppState>) -> IdentityView {
    IdentityView {
        device_id: state.identity.device_id.clone(),
        name: state.settings.lock().unwrap().device_name.clone(),
        fingerprint: state.identity.fingerprint(),
        public_key: state.identity.public_key_b64(),
        platform: std::env::consts::OS,
        desktop: cfg!(desktop),
    }
}

#[tauri::command]
pub fn set_device_name(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<()> {
    let name = clean_name(&name)?;
    let settings = {
        let mut s = state.settings.lock().unwrap();
        s.device_name = name;
        s.clone()
    };
    save_settings(&app, &settings)?;
    discovery::re_announce(&app);
    let _ = app.emit("settings-changed", settings);
    Ok(())
}

#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> Vec<DeviceView> {
    state.device_views()
}

#[tauri::command]
pub fn start_pairing(app: AppHandle, device_id: String) -> Result<()> {
    pairing::start(app, device_id)
}

#[tauri::command]
pub fn confirm_pairing(app: AppHandle, device_id: String, accepted: bool) -> Result<()> {
    pairing::confirm(&app, &device_id, accepted)
}

#[tauri::command]
pub fn cancel_pairing(app: AppHandle, device_id: String) -> Result<()> {
    pairing::confirm(&app, &device_id, false)
}

#[tauri::command]
pub fn unpair(app: AppHandle, device_id: String) -> Result<()> {
    if let Some(peer) = forget_peer(&app, &device_id) {
        // Let the other device drop the pairing too; fine if it is unreachable.
        tauri::async_runtime::spawn(session::notify_unpair(app.clone(), peer));
    }
    Ok(())
}

#[tauri::command]
pub async fn send_text(app: AppHandle, target: String, text: String) -> Result<Vec<SendResult>> {
    if text.trim().is_empty() {
        return Err(AppError::msg("Nothing to send"));
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(AppError::msg("Text is too large (1 MB max)"));
    }
    Ok(session::send_text(app, target, text).await)
}

#[tauri::command]
pub fn copy_to_clipboard(state: State<'_, AppState>, text: String) -> Result<()> {
    state
        .clipboard_tx
        .send(ClipboardCmd::Write(text))
        .map_err(|_| AppError::msg("Clipboard is unavailable"))
}

/// Current clipboard text, or an empty string when the clipboard is empty or
/// holds something that is not text.
#[tauri::command]
pub fn read_clipboard(app: AppHandle) -> String {
    app.clipboard().read_text().unwrap_or_default()
}

#[tauri::command]
pub fn pair_by_address(app: AppHandle, addr: String) -> Result<()> {
    pairing::start_by_address(app, &addr)
}

#[tauri::command]
pub fn create_pair_qr(app: AppHandle) -> Result<pairing::PairQr> {
    pairing::create_qr(&app)
}

#[tauri::command]
pub fn pair_by_qr(app: AppHandle, payload: String) -> Result<()> {
    pairing::start_by_qr(app, &payload)
}

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Vec<HistoryItem> {
    state.history.lock().unwrap().iter().cloned().collect()
}

#[tauri::command]
pub fn clear_history(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let mut h = state.history.lock().unwrap();
    h.clear();
    save_history(&app, &h)?;
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings> {
    let mut incoming = settings;
    incoming.device_name = clean_name(&incoming.device_name)?;

    let (previous, current) = {
        let mut s = state.settings.lock().unwrap();
        let previous = s.clone();
        *s = incoming;
        (previous, s.clone())
    };

    if current.auto_sync && !previous.auto_sync {
        let _ = state.clipboard_tx.send(ClipboardCmd::ResetBaseline);
    }
    if current.discovery != previous.discovery {
        discovery::set_enabled(&app, current.discovery);
    } else if current.device_name != previous.device_name {
        discovery::re_announce(&app);
    }
    apply_settings_side_effects(&app, &current);
    save_settings(&app, &current)?;
    let _ = app.emit("settings-changed", current.clone());
    Ok(current)
}

#[tauri::command]
pub fn get_listen_info(state: State<'_, AppState>) -> ListenInfo {
    ListenInfo {
        port: state.listen_port.load(Ordering::Relaxed),
        addrs: transport::local_ipv4_addrs()
            .iter()
            .map(|ip| ip.to_string())
            .collect(),
    }
}

/// Applies settings that live outside our own state: OS autostart and tray check state.
pub fn apply_settings_side_effects(app: &AppHandle, settings: &Settings) {
    tray::sync_autosync_check(app, settings.auto_sync);

    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let enabled = al.is_enabled().unwrap_or(false);
        let result = match (settings.autostart, enabled) {
            (true, false) => al.enable(),
            (false, true) => al.disable(),
            _ => Ok(()),
        };
        if let Err(e) = result {
            log::warn!("autostart update failed: {e}");
        }
    }
    let _ = app.state::<AppState>();
}

fn clean_name(name: &str) -> Result<String> {
    let name: String = name.trim().chars().take(MAX_NAME_LEN).collect();
    if name.is_empty() {
        return Err(AppError::msg("Device name cannot be empty"));
    }
    Ok(name)
}
