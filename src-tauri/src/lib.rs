mod clipboard;
mod commands;
mod crypto;
mod discovery;
mod error;
mod identity;
mod pairing;
mod protocol;
mod secrets;
mod session;
mod state;
mod transfer;
mod transport;
#[cfg(desktop)]
mod tray;

/// Mobile has no tray; keep the call sites platform-agnostic.
#[cfg(mobile)]
mod tray {
    use tauri::{AppHandle, Manager};

    pub fn show_main(app: &AppHandle) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_focus();
        }
    }

    pub fn sync_autosync_check(_app: &AppHandle, _checked: bool) {}
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;

use tauri::{Manager, WindowEvent};

use crate::identity::Identity;
use crate::state::{load_history, load_peers, load_settings, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_clipboard_manager::init());

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }));
    }

    builder = builder
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for(
                    "nearclip_lib",
                    if cfg!(debug_assertions) {
                        log::LevelFilter::Debug
                    } else {
                        log::LevelFilter::Info
                    },
                )
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init());

    #[cfg(mobile)]
    {
        // Share target: other apps can send text to NearClip through the OS share sheet.
        // Barcode scanner: pairing by scanning the QR code a desktop shows.
        builder = builder
            .plugin(tauri_plugin_sharehub::init())
            .plugin(tauri_plugin_barcode_scanner::init());
    }

    #[cfg(desktop)]
    {
        // On macOS the plugin defaults to a Launch Agent.
        builder = builder.plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--minimized"])
                .build(),
        );
    }

    builder
        .setup(|app| {
            let handle = app.handle().clone();

            secrets::init(&handle)?;
            let identity = Identity::load_or_create(&handle)?;
            log::info!("device id {}", identity.device_id);
            let settings = load_settings(&handle);
            let peers = load_peers(&handle);
            let history = load_history(&handle);
            let (clipboard_tx, clipboard_rx) = std::sync::mpsc::channel();

            app.manage(AppState {
                identity,
                settings: Mutex::new(settings.clone()),
                peers: Mutex::new(peers),
                discovered: Mutex::new(HashMap::new()),
                history: Mutex::new(history),
                pairings: Mutex::new(HashMap::new()),
                pair_token: Mutex::new(None),
                listen_port: AtomicU16::new(0),
                clipboard_tx,
                discovery: Mutex::new(None),
                outgoing: Mutex::new(HashMap::new()),
                #[cfg(desktop)]
                tray_autosync: Mutex::new(None),
            });

            clipboard::spawn(handle.clone(), clipboard_rx);

            let listener = tauri::async_runtime::block_on(transport::bind())?;
            let port = listener.local_addr()?.port();
            log::info!("listening on port {port}");
            handle
                .state::<AppState>()
                .listen_port
                .store(port, Ordering::Relaxed);
            transport::start_listener(handle.clone(), listener);

            discovery::set_enabled(&handle, settings.discovery);
            discovery::start_stale_sweep(&handle);

            #[cfg(desktop)]
            {
                if let Err(e) = tray::build(&handle) {
                    log::error!("tray setup failed: {e}");
                }
            }
            commands::apply_settings_side_effects(&handle, &settings);

            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Mobile can only read the clipboard in the foreground: check it
            // whenever the app comes back (auto-sync decides whether to send).
            // Desktop polls instead.
            if cfg!(mobile) && matches!(event, WindowEvent::Focused(true)) {
                let _ = window
                    .app_handle()
                    .state::<AppState>()
                    .clipboard_tx
                    .send(clipboard::ClipboardCmd::SyncNow);
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .app_handle()
                    .state::<AppState>()
                    .settings
                    .lock()
                    .map(|s| s.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_identity,
            commands::set_device_name,
            commands::list_devices,
            commands::start_pairing,
            commands::confirm_pairing,
            commands::cancel_pairing,
            commands::unpair,
            commands::send_text,
            commands::send_file_begin,
            commands::send_file_chunk,
            commands::send_file_end,
            commands::send_file_abort,
            commands::copy_to_clipboard,
            commands::read_clipboard,
            commands::pair_by_address,
            commands::create_pair_qr,
            commands::pair_by_qr,
            commands::get_history,
            commands::clear_history,
            commands::get_settings,
            commands::set_settings,
            commands::get_listen_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
