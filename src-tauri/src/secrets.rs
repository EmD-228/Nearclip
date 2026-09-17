//! Secrets at rest. One 32-byte master key lives in the OS credential store
//! (macOS Keychain, Windows Credential Manager, Secret Service on Linux) and
//! seals the identity seed and every pairing key inside the JSON store files,
//! so those files no longer hold usable key material. A single keychain item
//! means at most one "allow access" prompt, not one per pairing.
//!
//! Mobile has no keyring backend wired up: the master key is a file in the
//! app-private data directory, which the OS already isolates per app. Desktop
//! uses that file only when the system has no credential store at all.

use std::sync::OnceLock;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tauri::{AppHandle, Manager};

use crate::crypto::{open, random_bytes, seal};
use crate::error::{AppError, Result};
use crate::protocol::{b64d, b64d_array, b64e};

const KEY_NAME: &str = "master-key";
const KEY_FILE: &str = "master.key";

static MASTER_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Loads or creates the master key. Call once, before anything is unsealed.
pub fn init(app: &AppHandle) -> Result<()> {
    let key = load_or_create(app)?;
    let _ = MASTER_KEY.set(key);
    Ok(())
}

#[cfg(test)]
pub fn init_for_tests() {
    let _ = MASTER_KEY.set(random_bytes());
}

fn master_key() -> Result<&'static [u8; 32]> {
    MASTER_KEY
        .get()
        .ok_or_else(|| AppError::msg("secrets not initialized"))
}

/// AES-256-GCM under the master key, in the same shape as the wire `Enc` frame.
#[derive(Serialize, Deserialize)]
struct Sealed {
    n: String,
    c: String,
}

fn seal_bytes(plaintext: &[u8]) -> Result<Sealed> {
    let (nonce, ciphertext) = seal(master_key()?, plaintext)?;
    Ok(Sealed {
        n: b64e(&nonce),
        c: b64e(&ciphertext),
    })
}

fn open_bytes(sealed: &Sealed) -> Result<Vec<u8>> {
    open(master_key()?, &b64d_array(&sealed.n)?, &b64d(&sealed.c)?)
}

/// serde adapter for a 32-byte key field: written sealed, read either sealed
/// or as older versions wrote it (plain byte array, or base64 string for the
/// identity seed). Callers re-save after loading, which migrates the file.
pub fn ser_key32<S: Serializer>(key: &[u8; 32], s: S) -> std::result::Result<S::Ok, S::Error> {
    seal_bytes(key)
        .map_err(serde::ser::Error::custom)?
        .serialize(s)
}

pub fn de_key32<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<[u8; 32], D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Sealed(Sealed),
        Plain([u8; 32]),
        Base64(String),
    }
    let key = match Repr::deserialize(d)? {
        Repr::Sealed(s) => open_bytes(&s).and_then(|k| k.try_into().map_err(|_| AppError::Crypto)),
        Repr::Plain(k) => Ok(k),
        Repr::Base64(s) => b64d_array(&s),
    };
    key.map_err(serde::de::Error::custom)
}

fn load_or_create(app: &AppHandle) -> Result<[u8; 32]> {
    #[cfg(desktop)]
    match keychain_key(app) {
        Ok(key) => return Ok(key),
        // No credential store on this system (a Linux session without Secret
        // Service, say): keep the key in a file instead. Any other failure,
        // such as access denied or a locked store, must surface: silently
        // switching to a file key would re-key and orphan every pairing.
        Err(AppError::Keyring(
            keyring::Error::NoDefaultStore | keyring::Error::PlatformFailure(_),
        )) => log::warn!("no credential store available; master key kept in a file"),
        Err(e) => return Err(e),
    }
    file_key(app)
}

#[cfg(desktop)]
fn keychain_key(app: &AppHandle) -> Result<[u8; 32]> {
    let entry = keyring::Entry::new(&app.config().identifier, KEY_NAME)?;
    match entry.get_secret() {
        Ok(secret) => secret.try_into().map_err(|_| AppError::Crypto),
        Err(keyring::Error::NoEntry) => {
            let key: [u8; 32] = random_bytes();
            entry.set_secret(&key)?;
            log::info!("master key created in the credential store");
            Ok(key)
        }
        Err(e) => Err(e.into()),
    }
}

fn file_key(app: &AppHandle) -> Result<[u8; 32]> {
    let dir = app.path().app_data_dir()?;
    let path = dir.join(KEY_FILE);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(key) = b64d_array::<32>(text.trim()) {
            return Ok(key);
        }
        log::warn!("master key file unreadable, generating a new one");
    }
    let key: [u8; 32] = random_bytes();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&path, b64e(&key))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Rec {
        #[serde(serialize_with = "ser_key32", deserialize_with = "de_key32")]
        key: [u8; 32],
    }

    #[test]
    fn key32_field_roundtrips_sealed_and_rejects_tampering() {
        init_for_tests();
        let key: [u8; 32] = random_bytes();
        let json = serde_json::to_string(&Rec { key }).unwrap();
        assert!(json.contains("\"n\":"));
        assert!(!json.contains(&b64e(&key)));
        let back: Rec = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key, key);

        let mut tampered: serde_json::Value = serde_json::from_str(&json).unwrap();
        tampered["key"]["c"] = serde_json::json!(b64e(&[0u8; 48]));
        assert!(serde_json::from_value::<Rec>(tampered).is_err());
    }

    #[test]
    fn key32_field_reads_legacy_plain_formats() {
        init_for_tests();
        let key: [u8; 32] = random_bytes();
        let plain = serde_json::json!({ "key": key.to_vec() });
        assert_eq!(serde_json::from_value::<Rec>(plain).unwrap().key, key);
        let base64 = serde_json::json!({ "key": b64e(&key) });
        assert_eq!(serde_json::from_value::<Rec>(base64).unwrap().key, key);
    }
}
