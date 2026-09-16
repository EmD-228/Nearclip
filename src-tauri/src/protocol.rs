//! Wire format: `u32` big-endian length prefix + UTF-8 JSON of a [`Wire`] value.
//! Binary fields (keys, nonces, signatures, ciphertext) are base64 strings.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::crypto;
use crate::error::{AppError, Result};

pub const PROTO_VERSION: u8 = 1;
pub const DEFAULT_PORT: u16 = 47821;
pub const SERVICE_TYPE: &str = "_copynapaste._tcp.local.";
pub const MAX_FRAME: usize = 2 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Wire {
    PairRequest {
        v: u8,
        device_id: String,
        name: String,
        id_pk: String,
        commit: String,
    },
    PairResponse {
        device_id: String,
        name: String,
        id_pk: String,
        eph: String,
        nonce: String,
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
    },
    SessionAck {
        device_id: String,
        salt: String,
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
    PairAck {
        accepted: bool,
    },
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
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(AppError::protocol("incoming frame too large"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
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
        Wire::Error { code, msg } => Err(AppError::protocol(format!("{code}: {msg}"))),
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
}
