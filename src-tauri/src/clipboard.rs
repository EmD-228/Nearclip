//! Clipboard access through tauri-plugin-clipboard-manager (works on desktop and
//! mobile) plus the auto-sync watcher.
//!
//! All writes go through this single thread so the echo-suppression state stays
//! consistent. Desktop polls the clipboard; Android and iOS only allow reading
//! it while the app is in the foreground, so mobile checks it on demand
//! (`SyncNow`, sent when the app gains focus).

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_store::StoreExt;

use crate::crypto::sha256;
use crate::protocol::MAX_TEXT_BYTES;
use crate::session;
use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const SUPPRESSED_CAP: usize = 16;
const STORE_FILE: &str = "clipboard.json";
const KEY_LAST_HASH: &str = "lastHash";

pub enum ClipboardCmd {
    /// Write text to the clipboard; the text is remembered so the watcher never echoes it back.
    Write(String),
    /// Take the current clipboard content as the new baseline (used when auto-sync turns on).
    ResetBaseline,
    /// Read the clipboard now and send it if it changed (mobile: on app focus).
    SyncNow,
}

pub fn spawn(app: AppHandle, rx: Receiver<ClipboardCmd>) {
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || {
            let mut watcher = Watcher::new(app);
            if cfg!(desktop) {
                watcher.run_polling(rx)
            } else {
                watcher.run_on_demand(rx)
            }
        })
        .expect("failed to spawn clipboard thread");
}

struct Watcher {
    app: AppHandle,
    /// Hash of the last clipboard text seen (not necessarily sent: auto-sync
    /// may have been off). On mobile it survives restarts, so reopening the app
    /// does not resend what was copied before; on desktop it starts empty and
    /// whatever is in the clipboard at startup is never sent.
    last_hash: Option<[u8; 32]>,
    /// Hashes of text we wrote ourselves, never to be sent back.
    suppressed: VecDeque<[u8; 32]>,
}

impl Watcher {
    fn new(app: AppHandle) -> Self {
        let last_hash = if cfg!(mobile) {
            load_last_hash(&app)
        } else {
            None
        };
        Watcher {
            app,
            last_hash,
            suppressed: VecDeque::with_capacity(SUPPRESSED_CAP),
        }
    }

    fn run_polling(&mut self, rx: Receiver<ClipboardCmd>) {
        loop {
            match rx.recv_timeout(POLL_INTERVAL) {
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.check();
        }
    }

    fn run_on_demand(&mut self, rx: Receiver<ClipboardCmd>) {
        while let Ok(cmd) = rx.recv() {
            self.handle(cmd);
        }
    }

    fn handle(&mut self, cmd: ClipboardCmd) {
        match cmd {
            ClipboardCmd::Write(text) => {
                let h = sha256(text.as_bytes());
                self.remember_suppressed(h);
                self.set_last_hash(h);
                if let Err(e) = self.app.clipboard().write_text(text) {
                    log::warn!("clipboard write failed: {e}");
                }
            }
            ClipboardCmd::ResetBaseline => {
                if let Ok(text) = self.app.clipboard().read_text() {
                    self.set_last_hash(sha256(text.as_bytes()));
                }
            }
            ClipboardCmd::SyncNow => self.check(),
        }
    }

    /// Reads the clipboard and sends it to all paired devices when it holds
    /// new text and auto-sync is on. Read errors are expected (another app
    /// holds the clipboard, non-text content) and simply mean "nothing new".
    fn check(&mut self) {
        let Ok(text) = self.app.clipboard().read_text() else {
            return;
        };
        if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
            return;
        }
        let h = sha256(text.as_bytes());
        if self.last_hash == Some(h) {
            return;
        }
        let is_startup_baseline = self.last_hash.is_none() && cfg!(desktop);
        self.set_last_hash(h);
        if is_startup_baseline || self.suppressed.contains(&h) {
            return;
        }

        let state = self.app.state::<AppState>();
        let auto_sync = state.settings.lock().map(|s| s.auto_sync).unwrap_or(false);
        if !auto_sync || state.peers.lock().unwrap().is_empty() {
            return;
        }

        log::debug!("auto-sync: new clipboard text, sending to all paired devices");
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            session::send_text(app, "all".to_string(), text).await;
        });
    }

    fn set_last_hash(&mut self, h: [u8; 32]) {
        self.last_hash = Some(h);
        if cfg!(mobile) {
            save_last_hash(&self.app, h);
        }
    }

    fn remember_suppressed(&mut self, h: [u8; 32]) {
        if self.suppressed.contains(&h) {
            return;
        }
        if self.suppressed.len() >= SUPPRESSED_CAP {
            self.suppressed.pop_front();
        }
        self.suppressed.push_back(h);
    }
}

fn load_last_hash(app: &AppHandle) -> Option<[u8; 32]> {
    let value = app.store(STORE_FILE).ok()?.get(KEY_LAST_HASH)?;
    serde_json::from_value(value).ok()
}

fn save_last_hash(app: &AppHandle, h: [u8; 32]) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set(KEY_LAST_HASH, serde_json::to_value(h).unwrap_or_default());
        let _ = store.save();
    }
}
