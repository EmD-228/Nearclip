//! Clipboard access through tauri-plugin-clipboard-manager (works on desktop and
//! mobile) plus the desktop-only polling watcher that powers auto-sync.
//!
//! All writes go through this single thread so the echo-suppression state stays
//! consistent. On mobile there is no watcher: Android and iOS only allow reading
//! the clipboard while the app is in the foreground and warn the user on every read.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::crypto::sha256;
use crate::protocol::MAX_TEXT_BYTES;
use crate::session;
use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const SUPPRESSED_CAP: usize = 16;

pub enum ClipboardCmd {
    /// Write text to the clipboard; the text is remembered so the watcher never echoes it back.
    Write(String),
    /// Take the current clipboard content as the new baseline (used when auto-sync turns on).
    ResetBaseline,
}

pub fn spawn(app: AppHandle, rx: Receiver<ClipboardCmd>) {
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || {
            if cfg!(desktop) {
                run_with_watcher(app, rx)
            } else {
                run_write_only(app, rx)
            }
        })
        .expect("failed to spawn clipboard thread");
}

fn write(app: &AppHandle, text: String) {
    if let Err(e) = app.clipboard().write_text(text) {
        log::warn!("clipboard write failed: {e}");
    }
}

fn run_write_only(app: AppHandle, rx: Receiver<ClipboardCmd>) {
    while let Ok(cmd) = rx.recv() {
        if let ClipboardCmd::Write(text) = cmd {
            write(&app, text);
        }
    }
}

fn run_with_watcher(app: AppHandle, rx: Receiver<ClipboardCmd>) {
    let mut last_hash: Option<[u8; 32]> = None;
    let mut suppressed: VecDeque<[u8; 32]> = VecDeque::with_capacity(SUPPRESSED_CAP);

    loop {
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(ClipboardCmd::Write(text)) => {
                let h = sha256(text.as_bytes());
                remember(&mut suppressed, h);
                last_hash = Some(h);
                write(&app, text);
            }
            Ok(ClipboardCmd::ResetBaseline) => {
                last_hash = app
                    .clipboard()
                    .read_text()
                    .ok()
                    .map(|t| sha256(t.as_bytes()));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        // Poll. Errors are expected when another app holds the clipboard or the
        // content is not text; just try again next tick.
        let Ok(text) = app.clipboard().read_text() else {
            continue;
        };
        if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
            continue;
        }
        let h = sha256(text.as_bytes());
        if last_hash == Some(h) {
            continue;
        }
        let is_baseline = last_hash.is_none();
        last_hash = Some(h);
        if is_baseline || suppressed.contains(&h) {
            continue;
        }

        let state = app.state::<AppState>();
        let auto_sync = state.settings.lock().map(|s| s.auto_sync).unwrap_or(false);
        if !auto_sync || state.peers.lock().unwrap().is_empty() {
            continue;
        }

        log::debug!("auto-sync: clipboard changed, sending to all paired devices");
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            session::send_text(app2, "all".to_string(), text).await;
        });
    }
}

fn remember(suppressed: &mut VecDeque<[u8; 32]>, h: [u8; 32]) {
    if suppressed.contains(&h) {
        return;
    }
    if suppressed.len() >= SUPPRESSED_CAP {
        suppressed.pop_front();
    }
    suppressed.push_back(h);
}
