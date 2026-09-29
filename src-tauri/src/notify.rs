//! Telling the user something arrived, whatever the platform allows.
//!
//! Windows, Linux and Android go through the notification plugin. macOS cannot:
//! the plugin posts through `NSUserNotification`, which Apple retired, so
//! nothing is delivered and nothing is reported — the app never even appears
//! under Notifications in System Settings. This module talks to
//! `UserNotifications` instead, the framework the system listens to.
//!
//! macOS may still refuse, and does for any build Gatekeeper rejects, which is
//! every build without a Developer ID signature. There the Dock icon bounces:
//! the one arrival signal that needs no blessing.

use tauri::AppHandle;

/// Called once at startup, where a platform has something to ask for.
pub fn setup(app: &AppHandle) {
    imp::setup(app);
}

/// Announces one received item. Never fails loudly: a missed notification must
/// not disturb the transfer that earned it.
pub fn arrived(app: &AppHandle, title: &str, body: &str) {
    imp::arrived(app, title, body);
}

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};

    use block2::RcBlock;
    use objc2_foundation::{NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };
    use tauri::{AppHandle, Manager, UserAttentionType};

    /// Whether the system agreed to show notifications for this app. False until
    /// the answer arrives, and for good on a build macOS will not deliver for.
    static ALLOWED: AtomicBool = AtomicBool::new(false);

    /// Asks at startup rather than when the first message arrives: macOS shows
    /// its own prompt, and asking then would interrupt the arrival it is meant
    /// to announce. It also puts the app in System Settings under Notifications,
    /// where someone who refused can change their mind.
    pub fn setup(_app: &AppHandle) {
        let done = RcBlock::new(|granted: objc2::runtime::Bool, error: *mut NSError| {
            let granted = granted.as_bool();
            ALLOWED.store(granted, Ordering::Relaxed);
            match unsafe { error.as_ref() } {
                Some(error) => log::warn!("asking for notification permission failed: {error}"),
                None if !granted => log::info!("notifications are turned off for this app"),
                None => {}
            }
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &done,
            );
    }

    pub fn arrived(app: &AppHandle, title: &str, body: &str) {
        if !ALLOWED.load(Ordering::Relaxed) {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.request_user_attention(Some(UserAttentionType::Informational));
            }
            return;
        }

        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));

        // The identifier only has to be unique among pending notifications; a
        // new one every time means a second message never replaces the first.
        let id = NSString::from_str(&uuid::Uuid::new_v4().to_string());
        let request =
            UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
        let posted = RcBlock::new(|error: *mut NSError| {
            if let Some(error) = unsafe { error.as_ref() } {
                log::warn!("the system refused a notification: {error}");
            }
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&posted));
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::AppHandle;
    use tauri_plugin_notification::NotificationExt;

    /// Nothing to ask for: the plugin requests permission where a platform needs
    /// it, when it posts.
    pub fn setup(_app: &AppHandle) {}

    pub fn arrived(app: &AppHandle, title: &str, body: &str) {
        let notification = app.notification().builder().title(title).body(body);
        // Android status bar icon (a drawable name) and its tint; the plugin
        // falls back to the generic info icon otherwise.
        #[cfg(mobile)]
        let notification = notification.icon("ic_notification").icon_color("#2563eb");
        if let Err(e) = notification.show() {
            log::warn!("notification failed: {e}");
        }
    }
}
