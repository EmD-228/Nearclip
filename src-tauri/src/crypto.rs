//! Pure cryptographic helpers. No I/O, fully unit-tested.
//!
//! Pairing: X25519 ECDH on ephemeral keys, transcript-bound via HKDF, producing
//! a long-term pairing key and a 6-digit short authentication string (SAS).
//! Sessions: per-connection key from the pairing key + two fresh salts.
//! Payloads: AES-256-GCM with random 96-bit nonces.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

use crate::error::{AppError, Result};

pub const PAIR_TRANSCRIPT_PREFIX: &[u8] = b"nearclip-pair-v1";
const INFO_PAIRING_KEY: &[u8] = b"nearclip pairing key v1";
const INFO_SAS: &[u8] = b"nearclip sas v1";
const INFO_SESSION: &[u8] = b"nearclip session v1";

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).expect("OS randomness unavailable");
    buf
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Commitment to the initiator's ephemeral key, sent before the responder
/// reveals its own contribution. Prevents an active MITM from grinding
/// ephemeral keys until both victims see the same SAS.
pub fn commit(eph_pk: &[u8; 32], nonce: &[u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(eph_pk);
    buf.extend_from_slice(nonce);
    sha256(&buf)
}

/// Builds the pairing transcript. Both sides must call this with the
/// initiator (A) parameters first and responder (B) parameters second.
pub fn pairing_transcript(
    id_pk_a: &[u8; 32],
    eph_a: &[u8; 32],
    nonce_a: &[u8; 32],
    id_pk_b: &[u8; 32],
    eph_b: &[u8; 32],
    nonce_b: &[u8; 32],
) -> Vec<u8> {
    let mut t = Vec::with_capacity(PAIR_TRANSCRIPT_PREFIX.len() + 6 * 32);
    t.extend_from_slice(PAIR_TRANSCRIPT_PREFIX);
    t.extend_from_slice(id_pk_a);
    t.extend_from_slice(eph_a);
    t.extend_from_slice(nonce_a);
    t.extend_from_slice(id_pk_b);
    t.extend_from_slice(eph_b);
    t.extend_from_slice(nonce_b);
    t
}

pub struct PairingMaterial {
    pub pairing_key: [u8; 32],
    /// Six decimal digits, zero-padded, e.g. "048391".
    pub sas: String,
}

pub fn derive_pairing(shared: &[u8; 32], transcript: &[u8]) -> PairingMaterial {
    let salt = sha256(transcript);
    let hk = Hkdf::<Sha256>::new(Some(&salt), shared);

    let mut pairing_key = [0u8; 32];
    hk.expand(INFO_PAIRING_KEY, &mut pairing_key)
        .expect("32 bytes is a valid HKDF output length");

    let mut sas_bytes = [0u8; 4];
    hk.expand(INFO_SAS, &mut sas_bytes)
        .expect("4 bytes is a valid HKDF output length");
    let sas_num = u32::from_be_bytes(sas_bytes) % 1_000_000;

    PairingMaterial {
        pairing_key,
        sas: format!("{sas_num:06}"),
    }
}

pub fn derive_session_key(
    pairing_key: &[u8; 32],
    salt_a: &[u8; 32],
    salt_b: &[u8; 32],
) -> [u8; 32] {
    let mut salt = Vec::with_capacity(64);
    salt.extend_from_slice(salt_a);
    salt.extend_from_slice(salt_b);
    let hk = Hkdf::<Sha256>::new(Some(&salt), pairing_key);
    let mut key = [0u8; 32];
    hk.expand(INFO_SESSION, &mut key)
        .expect("32 bytes is a valid HKDF output length");
    key
}

pub fn seal(key: &[u8; 32], plaintext: &[u8]) -> Result<([u8; 12], Vec<u8>)> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::Crypto)?;
    let nonce_bytes: [u8; 12] = random_bytes();
    let nonce = Nonce::<aes_gcm::aead::consts::U12>::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| AppError::Crypto)?;
    Ok((nonce_bytes, ciphertext))
}

pub fn open(key: &[u8; 32], nonce: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::Crypto)?;
    let nonce = Nonce::<aes_gcm::aead::consts::U12>::from(*nonce);
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| AppError::Crypto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::{PublicKey, StaticSecret};

    fn ecdh_pair() -> ([u8; 32], [u8; 32]) {
        let a = StaticSecret::from(random_bytes::<32>());
        let b = StaticSecret::from(random_bytes::<32>());
        let pa = PublicKey::from(&a);
        let pb = PublicKey::from(&b);
        (
            a.diffie_hellman(&pb).to_bytes(),
            b.diffie_hellman(&pa).to_bytes(),
        )
    }

    #[test]
    fn both_sides_derive_same_pairing_material() {
        let (shared_a, shared_b) = ecdh_pair();
        assert_eq!(shared_a, shared_b);
        let t = b"transcript";
        let ma = derive_pairing(&shared_a, t);
        let mb = derive_pairing(&shared_b, t);
        assert_eq!(ma.pairing_key, mb.pairing_key);
        assert_eq!(ma.sas, mb.sas);
        assert_eq!(ma.sas.len(), 6);
        assert!(ma.sas.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn different_transcript_gives_different_sas_and_key() {
        let (shared, _) = ecdh_pair();
        let m1 = derive_pairing(&shared, b"t1");
        let m2 = derive_pairing(&shared, b"t2");
        assert_ne!(m1.pairing_key, m2.pairing_key);
    }

    #[test]
    fn mitm_with_two_shared_secrets_yields_different_keys() {
        // Attacker in the middle ends up with a different shared secret on each leg.
        let (shared_a, _) = ecdh_pair();
        let (shared_b, _) = ecdh_pair();
        let t = b"same transcript";
        assert_ne!(
            derive_pairing(&shared_a, t).pairing_key,
            derive_pairing(&shared_b, t).pairing_key
        );
    }

    #[test]
    fn commit_mismatch_is_detectable() {
        let eph: [u8; 32] = random_bytes();
        let nonce: [u8; 32] = random_bytes();
        let other: [u8; 32] = random_bytes();
        assert_eq!(commit(&eph, &nonce), commit(&eph, &nonce));
        assert_ne!(commit(&eph, &nonce), commit(&other, &nonce));
    }

    #[test]
    fn seal_open_roundtrip_and_tamper_fails() {
        let key: [u8; 32] = random_bytes();
        let (nonce, mut ct) = seal(&key, b"hello clipboard").unwrap();
        assert_eq!(open(&key, &nonce, &ct).unwrap(), b"hello clipboard");
        ct[0] ^= 0x01;
        assert!(open(&key, &nonce, &ct).is_err());
        let wrong: [u8; 32] = random_bytes();
        ct[0] ^= 0x01;
        assert!(open(&wrong, &nonce, &ct).is_err());
    }

    #[test]
    fn session_key_is_deterministic_and_salt_sensitive() {
        let pk: [u8; 32] = random_bytes();
        let s1: [u8; 32] = random_bytes();
        let s2: [u8; 32] = random_bytes();
        assert_eq!(
            derive_session_key(&pk, &s1, &s2),
            derive_session_key(&pk, &s1, &s2)
        );
        assert_ne!(
            derive_session_key(&pk, &s1, &s2),
            derive_session_key(&pk, &s2, &s1)
        );
    }
}
