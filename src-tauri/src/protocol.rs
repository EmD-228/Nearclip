//! Wire format: `u32` big-endian length prefix + body.
//!
//! Most bodies are UTF-8 JSON of a [`Wire`] value, with binary fields
//! (keys, nonces, signatures, ciphertext) as base64 strings. File data travels
//! in sealed binary frames instead: the length has its top bit set and the body
//! is `nonce || AES-256-GCM ciphertext`. Only devices that announced
//! [`FEATURE_FILES`] ever receive one.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::crypto;
use crate::error::{AppError, Result};
use crate::state::PairedVia;

pub const PROTO_VERSION: u8 = 1;
pub const DEFAULT_PORT: u16 = 47821;
pub const SERVICE_TYPE: &str = "_nearclip._tcp.local.";
pub const MAX_FRAME: usize = 2 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
/// Raw bytes per sealed file chunk.
pub const FILE_CHUNK_BYTES: usize = 512 * 1024;
/// Announced in `SessionAck` by devices that accept `FileStart`.
pub const FEATURE_FILES: &str = "files";
const BINARY_FRAME: u32 = 1 << 31;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Wire {
    PairRequest {
        v: u8,
        device_id: String,
        name: String,
        id_pk: String,
        commit: String,
        /// Sender's listening port, so the responder can reach it back later
        /// (the connection's source port is ephemeral). 0 = unknown.
        #[serde(default)]
        port: u16,
        /// One-time token read from the responder's QR code; lets both sides
        /// confirm without comparing a code.
        #[serde(default)]
        token: Option<String>,
        /// How the initiator found the responder, so both sides record the
        /// same pairing method.
        #[serde(default)]
        via: Option<PairedVia>,
    },
    PairResponse {
        device_id: String,
        name: String,
        id_pk: String,
        eph: String,
        nonce: String,
        /// True when the responder accepted `PairRequest::token`.
        #[serde(default)]
        token_ok: bool,
    },
    PairReveal {
        eph: String,
        nonce: String,
        sig: String,
    },
    PairSig {
        sig: String,
    },
    SessionInit {
        v: u8,
        device_id: String,
        salt: String,
        /// Sender's listening port (see `PairRequest::port`).
        #[serde(default)]
        port: u16,
    },
    SessionAck {
        device_id: String,
        salt: String,
        /// Optional capabilities, e.g. [`FEATURE_FILES`]. Absent from older
        /// devices, which only exchange text.
        #[serde(default)]
        features: Vec<String>,
    },
    /// Encrypted [`Plain`] payload: `n` = base64 nonce, `c` = base64 ciphertext.
    Enc {
        n: String,
        c: String,
    },
    Error {
        code: String,
        msg: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Plain {
    Clipboard {
        id: String,
        ts: i64,
        sender: String,
        text: String,
    },
    Ack {
        id: String,
    },
    /// Announces a file to a device that listed [`FEATURE_FILES`]. The receiver
    /// answers `Ack` when ready, then gets the bytes as sealed binary frames and
    /// a `FileEnd`, which it answers with `Ack` once the file is verified and saved.
    FileStart {
        id: String,
        ts: i64,
        sender: String,
        name: String,
        size: u64,
        mime: String,
    },
    FileEnd {
        id: String,
        /// Base64 SHA-256 of the whole file.
        sha256: String,
    },
    PairAck {
        accepted: bool,
    },
    /// The sender removed this pairing; the receiver should forget it too.
    Unpair,
    Ping,
}

pub fn b64e(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

pub fn b64d(s: &str) -> Result<Vec<u8>> {
    B64.decode(s)
        .map_err(|_| AppError::protocol("invalid base64"))
}

pub fn b64d_array<const N: usize>(s: &str) -> Result<[u8; N]> {
    let v = b64d(s)?;
    v.try_into()
        .map_err(|_| AppError::protocol(format!("expected {N} bytes")))
}

pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, msg: &Wire) -> Result<()> {
    let body = serde_json::to_vec(msg)?;
    if body.len() > MAX_FRAME {
        return Err(AppError::protocol("frame too large"));
    }
    w.write_all(&(body.len() as u32).to_be_bytes()).await?;
    w.write_all(&body).await?;
    w.flush().await?;
    Ok(())
}

pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Wire> {
    match read_any_frame(r).await? {
        Frame::Wire(wire) => Ok(wire),
        Frame::Sealed(_) => Err(AppError::protocol("unexpected binary frame")),
    }
}

pub enum Frame {
    Wire(Wire),
    /// Body of a sealed binary frame, still encrypted: open it with [`open_sealed`].
    Sealed(Vec<u8>),
}

pub async fn read_any_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Frame> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let header = u32::from_be_bytes(len_buf);
    let len = (header & !BINARY_FRAME) as usize;
    if len > MAX_FRAME {
        return Err(AppError::protocol("incoming frame too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    if header & BINARY_FRAME != 0 {
        Ok(Frame::Sealed(body))
    } else {
        Ok(Frame::Wire(serde_json::from_slice(&body)?))
    }
}

/// Encrypts `bytes` and writes them as one sealed binary frame.
pub async fn write_sealed<W: AsyncWrite + Unpin>(
    w: &mut W,
    key: &[u8; 32],
    bytes: &[u8],
) -> Result<()> {
    let (nonce, ct) = crypto::seal(key, bytes)?;
    let len = nonce.len() + ct.len();
    if len > MAX_FRAME {
        return Err(AppError::protocol("frame too large"));
    }
    w.write_all(&(len as u32 | BINARY_FRAME).to_be_bytes())
        .await?;
    w.write_all(&nonce).await?;
    w.write_all(&ct).await?;
    w.flush().await?;
    Ok(())
}

pub fn open_sealed(key: &[u8; 32], body: &[u8]) -> Result<Vec<u8>> {
    let (nonce, ct) = body
        .split_first_chunk::<12>()
        .ok_or_else(|| AppError::protocol("sealed frame too short"))?;
    crypto::open(key, nonce, ct)
}

pub fn encrypt_plain(key: &[u8; 32], plain: &Plain) -> Result<Wire> {
    let bytes = serde_json::to_vec(plain)?;
    let (nonce, ct) = crypto::seal(key, &bytes)?;
    Ok(Wire::Enc {
        n: b64e(&nonce),
        c: b64e(&ct),
    })
}

pub fn decrypt_wire(key: &[u8; 32], wire: &Wire) -> Result<Plain> {
    match wire {
        Wire::Enc { n, c } => {
            let nonce: [u8; 12] = b64d_array(n)?;
            let ct = b64d(c)?;
            let pt = crypto::open(key, &nonce, &ct)?;
            Ok(serde_json::from_slice(&pt)?)
        }
        Wire::Error { code, msg } => Err(AppError::Remote {
            code: code.clone(),
            msg: msg.clone(),
        }),
        other => Err(AppError::protocol(format!(
            "expected encrypted frame, got {other:?}"
        ))),
    }
}

pub fn error_frame(code: &str, msg: impl Into<String>) -> Wire {
    Wire::Error {
        code: code.to_string(),
        msg: msg.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frame_roundtrip_over_duplex() {
        let (mut a, mut b) = tokio::io::duplex(4096);
        let msg = Wire::PairSig { sig: "abc".into() };
        write_frame(&mut a, &msg).await.unwrap();
        let got = read_frame(&mut b).await.unwrap();
        assert_eq!(got, msg);
    }

    #[tokio::test]
    async fn oversize_frame_is_rejected() {
        let (mut a, mut b) = tokio::io::duplex(64);
        let len = (MAX_FRAME as u32 + 1).to_be_bytes();
        tokio::spawn(async move {
            let _ = a.write_all(&len).await;
        });
        assert!(matches!(
            read_frame(&mut b).await,
            Err(AppError::Protocol(_))
        ));
    }

    #[test]
    fn encrypt_decrypt_plain_roundtrip() {
        let key: [u8; 32] = crypto::random_bytes();
        let plain = Plain::Clipboard {
            id: "1".into(),
            ts: 42,
            sender: "dev".into(),
            text: "hello".into(),
        };
        let wire = encrypt_plain(&key, &plain).unwrap();
        assert_eq!(decrypt_wire(&key, &wire).unwrap(), plain);
        let other: [u8; 32] = crypto::random_bytes();
        assert!(decrypt_wire(&other, &wire).is_err());
    }

    #[tokio::test]
    async fn sealed_frames_interleave_with_json_frames() {
        let key: [u8; 32] = crypto::random_bytes();
        let data = vec![42u8; FILE_CHUNK_BYTES];
        let (mut a, mut b) = tokio::io::duplex(4 * 1024 * 1024);
        write_sealed(&mut a, &key, &data).await.unwrap();
        write_frame(&mut a, &Wire::PairSig { sig: "x".into() })
            .await
            .unwrap();

        let Frame::Sealed(body) = read_any_frame(&mut b).await.unwrap() else {
            panic!("expected a sealed frame");
        };
        assert_eq!(open_sealed(&key, &body).unwrap(), data);
        assert!(open_sealed(&crypto::random_bytes(), &body).is_err());
        assert!(matches!(read_frame(&mut b).await, Ok(Wire::PairSig { .. })));
    }

    #[tokio::test]
    async fn text_only_reader_rejects_sealed_frames() {
        let key: [u8; 32] = crypto::random_bytes();
        let (mut a, mut b) = tokio::io::duplex(4096);
        write_sealed(&mut a, &key, b"chunk").await.unwrap();
        assert!(matches!(
            read_frame(&mut b).await,
            Err(AppError::Protocol(_))
        ));
    }
}
