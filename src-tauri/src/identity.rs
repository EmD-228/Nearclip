//! Long-lived Ed25519 device identity, persisted in `identity.json`.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::crypto;
use crate::error::{AppError, Result};
use crate::protocol::{b64d_array, b64e};

const STORE_FILE: &str = "identity.json";
const KEY_SEED: &str = "seed";

pub struct Identity {
    signing_key: SigningKey,
    pub device_id: String,
}

impl Identity {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        let device_id = device_id_from_pk(&signing_key.verifying_key().to_bytes());
        Identity {
            signing_key,
            device_id,
        }
    }

    pub fn load_or_create(app: &AppHandle) -> Result<Self> {
        let store = app.store(STORE_FILE)?;
        if let Some(v) = store.get(KEY_SEED) {
            if let Some(s) = v.as_str() {
                if let Ok(seed) = b64d_array::<32>(s) {
                    return Ok(Identity::from_seed(seed));
                }
            }
            log::warn!("identity seed unreadable, generating a new identity");
        }
        let seed: [u8; 32] = crypto::random_bytes();
        store.set(KEY_SEED, json!(b64e(&seed)));
        store.save()?;
        Ok(Identity::from_seed(seed))
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn public_key_b64(&self) -> String {
        b64e(&self.public_key())
    }

    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.signing_key.sign(msg).to_bytes()
    }

    /// Human-readable fingerprint: device id grouped by four hex chars.
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.device_id)
    }
}

pub fn fingerprint_of(device_id: &str) -> String {
    device_id
        .as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 128-bit fingerprint of the Ed25519 public key, hex encoded (32 chars).
pub fn device_id_from_pk(pk: &[u8; 32]) -> String {
    hex::encode(&crypto::sha256(pk)[..16])
}

pub fn verify(pk: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> Result<()> {
    let vk = VerifyingKey::from_bytes(pk).map_err(|_| AppError::Crypto)?;
    let sig = Signature::from_bytes(sig);
    vk.verify_strict(msg, &sig).map_err(|_| AppError::Crypto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_and_stable_device_id() {
        let seed: [u8; 32] = crypto::random_bytes();
        let id1 = Identity::from_seed(seed);
        let id2 = Identity::from_seed(seed);
        assert_eq!(id1.device_id, id2.device_id);
        assert_eq!(id1.device_id.len(), 32);
        let sig = id1.sign(b"msg");
        assert!(verify(&id1.public_key(), b"msg", &sig).is_ok());
        assert!(verify(&id1.public_key(), b"other", &sig).is_err());
        assert_eq!(fingerprint_of("abcdefgh").as_str(), "abcd efgh");
    }
}
