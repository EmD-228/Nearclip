mod clipboard;
mod commands;
mod crypto;
mod discovery;
mod error;
mod identity;
mod pairing;
mod protocol;
mod session;
mod state;
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

    #[cfg(desktop)]
    {
        builder = builder.plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
                .args(["--minimized"])
                .build(),
        );
    }

    builder
        .setup(|app| {
            let handle = app.handle().clone();

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
                listen_port: AtomicU16::new(0),
                clipboard_tx,
                discovery: Mutex::new(None),
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

            match discovery::start(&handle, port) {
                Ok(daemon) => *handle.state::<AppState>().discovery.lock().unwrap() = Some(daemon),
                Err(e) => log::error!("mDNS discovery failed to start: {e}"),
            }

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
            commands::copy_to_clipboard,
            commands::get_history,
            commands::clear_history,
            commands::get_settings,
            commands::set_settings,
            commands::get_listen_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
