//! System tray icon with Show / Auto-sync / Quit and close-to-tray helpers.

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::error::{AppError, Result};
use crate::state::{save_settings, AppState};
use crate::{clipboard::ClipboardCmd, commands};

pub fn build(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let auto_sync = state.settings.lock().unwrap().auto_sync;

    let show = MenuItem::with_id(app, "show", "Show nearclip", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let autosync = CheckMenuItem::with_id(
        app,
        "autosync",
        "Auto-sync clipboard",
        true,
        auto_sync,
        None::<&str>,
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let items: [&dyn IsMenuItem<Wry>; 5] = [&show, &sep1, &autosync, &sep2, &quit];
    let menu = Menu::with_items(app, &items)?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| AppError::msg("no default window icon"))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("nearclip")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "autosync" => toggle_autosync(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    *state.tray_autosync.lock().unwrap() = Some(autosync);
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_autosync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = {
        let mut s = state.settings.lock().unwrap();
        s.auto_sync = !s.auto_sync;
        s.clone()
    };
    if settings.auto_sync {
        let _ = state.clipboard_tx.send(ClipboardCmd::ResetBaseline);
    }
    commands::apply_settings_side_effects(app, &settings);
    if let Err(e) = save_settings(app, &settings) {
        log::warn!("saving settings failed: {e}");
    }
    let _ = app.emit("settings-changed", settings);
}

/// Keeps the tray check item in sync with the settings.
pub fn sync_autosync_check(app: &AppHandle, checked: bool) {
    let state = app.state::<AppState>();
    let guard = state.tray_autosync.lock().unwrap();
    if let Some(item) = guard.as_ref() {
        let _ = item.set_checked(checked);
    }
}
