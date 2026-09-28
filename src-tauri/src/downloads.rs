//! What becomes of a received file once it is on disk.
//!
//! Desktop writes it straight into the Downloads folder and reveals it there.
//! Android cannot: since version 10 that folder belongs to the system, and a
//! file written to the app's own directory is invisible in the Files app —
//! received, but nowhere the user can find it. The only supported way in is
//! MediaStore, which lives in Kotlin, so `DownloadsPlugin` does that work and
//! this module calls it.
//!
//! Compiled on every platform, with a desktop implementation that says no, so
//! that callers and commands stay free of platform conditionals.

use serde::Deserialize;

use crate::error::Result;
use crate::state::FileMeta;

/// Where a saved file ended up, in both forms the app needs: what opens it, and
/// what to show the user.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub uri: String,
    /// The name the file ended up with, which is not the one asked for when the
    /// folder already held one by that name.
    pub name: String,
}

#[cfg(target_os = "android")]
mod imp {
    use serde::Serialize;
    use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
    use tauri::{AppHandle, Manager, Wry};

    use super::Saved;
    use crate::error::{AppError, Result};

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SaveArgs<'a> {
        path: &'a str,
        name: &'a str,
        mime: &'a str,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct UriArgs<'a> {
        uri: &'a str,
        mime: &'a str,
    }

    pub fn init() -> TauriPlugin<Wry> {
        Builder::new("downloads")
            .setup(|app, api| {
                app.manage(api.register_android_plugin("com.nearclip", "DownloadsPlugin")?);
                Ok(())
            })
            .build()
    }

    fn call<T: serde::de::DeserializeOwned>(
        app: &AppHandle,
        command: &str,
        payload: impl Serialize,
    ) -> Result<T> {
        handle(app)?
            .run_mobile_plugin(command, payload)
            .map_err(|e| AppError::msg(e.to_string()))
    }

    fn handle(app: &AppHandle) -> Result<tauri::State<'_, PluginHandle<Wry>>> {
        app.try_state::<PluginHandle<Wry>>()
            .ok_or_else(|| AppError::msg("downloads plugin is not available"))
    }

    /// Async because the copy takes seconds for a large file: the blocking call
    /// would hold a Tokio worker that the other sessions and timers need.
    pub async fn save(app: &AppHandle, path: &str, name: &str, mime: &str) -> Result<Saved> {
        handle(app)?
            .run_mobile_plugin_async("save", SaveArgs { path, name, mime })
            .await
            .map_err(|e| AppError::msg(e.to_string()))
    }

    pub fn open(app: &AppHandle, uri: &str, mime: &str) -> Result<()> {
        call(app, "open", UriArgs { uri, mime })
    }

    pub fn share(app: &AppHandle, uri: &str, mime: &str) -> Result<()> {
        call(app, "share", UriArgs { uri, mime })
    }
}

#[cfg(not(target_os = "android"))]
mod imp {
    use tauri::plugin::TauriPlugin;
    use tauri::{AppHandle, Wry};

    use super::Saved;
    use crate::error::{AppError, Result};

    pub fn init() -> TauriPlugin<Wry> {
        tauri::plugin::Builder::new("downloads").build()
    }

    /// Desktop writes received files to Downloads itself; nothing to hand over.
    pub async fn save(_app: &AppHandle, _path: &str, _name: &str, _mime: &str) -> Result<Saved> {
        Err(unsupported())
    }

    pub fn open(_app: &AppHandle, _uri: &str, _mime: &str) -> Result<()> {
        Err(unsupported())
    }

    pub fn share(_app: &AppHandle, _uri: &str, _mime: &str) -> Result<()> {
        Err(unsupported())
    }

    fn unsupported() -> AppError {
        AppError::msg("This device opens received files from their folder")
    }
}

pub use imp::{init, open, share};

/// Hands a freshly received file to the system on Android, and reports where it
/// went. On every other platform the file is already where the user looks.
pub async fn store(
    app: &tauri::AppHandle,
    path: &std::path::Path,
    meta: FileMeta,
) -> Result<FileMeta> {
    if cfg!(not(target_os = "android")) {
        return Ok(meta);
    }
    let saved = imp::save(app, &path.to_string_lossy(), &meta.name, &meta.mime).await?;
    // The staging copy is this side's to clean up.
    let _ = std::fs::remove_file(path);
    Ok(FileMeta {
        path: Some(format!("Download/NearClip/{}", saved.name)),
        uri: Some(saved.uri),
        name: saved.name,
        ..meta
    })
}
