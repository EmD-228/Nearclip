use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("Store error: {0}")]
    Store(#[from] tauri_plugin_store::Error),
    #[error("mDNS error: {0}")]
    Mdns(#[from] mdns_sd::Error),
    #[cfg(desktop)]
    #[error("Credential store error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Cryptographic verification failed")]
    Crypto,
    #[error("Unknown peer: this device is not paired with the sender")]
    UnknownPeer,
    #[error("Device is not reachable on the local network")]
    DeviceUnavailable,
    #[error("Protocol error: {0}")]
    Protocol(String),
    /// An error frame the other device sent, e.g. `too_large`.
    #[error("{msg} ({code})")]
    Remote { code: String, msg: String },
    #[error("Operation timed out")]
    Timeout,
    #[error("Pairing was rejected")]
    PairingRejected,
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }
    pub fn protocol(s: impl Into<String>) -> Self {
        AppError::Protocol(s.into())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<tokio::time::error::Elapsed> for AppError {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        AppError::Timeout
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
