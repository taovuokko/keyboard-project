use crate::Vec;
#[cfg(feature = "crypto")]
use crate::{MAX_MAC_BYTES, NONCE_BYTES};
#[cfg(feature = "crypto")]
use chacha20poly1305::aead::{AeadInPlace, KeyInit};
#[cfg(feature = "crypto")]
use chacha20poly1305::{Key, Tag, XChaCha20Poly1305, XNonce};

#[derive(Debug, PartialEq, Eq)]
pub enum CryptoError {
    AuthFailed,
    Parse(crate::ParseError),
    Serialize(crate::SerializationError),
}

/// Minimal AEAD interface for plugging in real XChaCha20-Poly1305 or dummy.
pub trait Aead {
    fn seal(
        &self,
        nonce: &[u8],
        aad: &[u8],
        plaintext: &[u8],
        mac_len: usize,
    ) -> Result<(Vec<u8>, Vec<u8>), CryptoError>;

    fn open(
        &self,
        nonce: &[u8],
        aad: &[u8],
        ciphertext: &[u8],
        mac: &[u8],
    ) -> Result<Vec<u8>, CryptoError>;
}

/// Deterministic, non-cryptographic AEAD for simulations and tests.
pub struct DummyAead;

impl Aead for DummyAead {
    fn seal(
        &self,
        nonce: &[u8],
        aad: &[u8],
        plaintext: &[u8],
        mac_len: usize,
    ) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
        let mac = simple_tag(aad, plaintext, nonce, mac_len);
        Ok((plaintext.to_vec(), mac))
    }

    fn open(
        &self,
        nonce: &[u8],
        aad: &[u8],
        ciphertext: &[u8],
        mac: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let expected = simple_tag(aad, ciphertext, nonce, mac.len());
        if expected == mac {
            Ok(ciphertext.to_vec())
        } else {
            Err(CryptoError::AuthFailed)
        }
    }
}

/// Real XChaCha20-Poly1305 AEAD.
#[cfg(feature = "crypto")]
pub struct RealAead {
    cipher: XChaCha20Poly1305,
}

#[cfg(feature = "crypto")]
impl RealAead {
    pub fn new(key: [u8; crate::KEY_BYTES]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(&Key::from_slice(&key)),
        }
    }
}

#[cfg(feature = "crypto")]
impl Aead for RealAead {
    fn seal(
        &self,
        nonce: &[u8],
        aad: &[u8],
        plaintext: &[u8],
        mac_len: usize,
    ) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
        if nonce.len() != NONCE_BYTES {
            return Err(CryptoError::Parse(crate::ParseError::UnexpectedLength));
        }
        if mac_len != MAX_MAC_BYTES {
            return Err(CryptoError::Serialize(
                crate::SerializationError::MacLengthMismatch,
            ));
        }

        let mut buf = plaintext.to_vec();
        let tag = self
            .cipher
            .encrypt_in_place_detached(XNonce::from_slice(nonce), aad, &mut buf)
            .map_err(|_| CryptoError::AuthFailed)?;
        let mut mac = tag.to_vec();
        if mac.len() != mac_len {
            mac.truncate(mac_len.min(mac.len()));
        }

        Ok((buf, mac))
    }

    fn open(
        &self,
        nonce: &[u8],
        aad: &[u8],
        ciphertext: &[u8],
        mac: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        if nonce.len() != NONCE_BYTES {
            return Err(CryptoError::Parse(crate::ParseError::UnexpectedLength));
        }
        if mac.len() != MAX_MAC_BYTES {
            return Err(CryptoError::Parse(crate::ParseError::MacLengthMismatch));
        }

        let mut buf = ciphertext.to_vec();
        self.cipher
            .decrypt_in_place_detached(
                XNonce::from_slice(nonce),
                aad,
                &mut buf,
                Tag::from_slice(mac),
            )
            .map_err(|_| CryptoError::AuthFailed)?;
        Ok(buf)
    }
}

fn simple_tag(aad: &[u8], payload: &[u8], nonce: &[u8], mac_len: usize) -> Vec<u8> {
    let mut state: u32 = 0xA5A5_5A5A;
    for b in aad.iter().chain(payload).chain(nonce) {
        state = state.rotate_left(5) ^ (*b as u32);
        state = state.wrapping_mul(0x45d9f3b);
    }
    let mut out = Vec::with_capacity(mac_len);
    let mut bytes = state.to_le_bytes();
    while out.len() < mac_len {
        out.extend_from_slice(&bytes);
        state = state.rotate_left(7) ^ 0xA5A5_A5A5;
        bytes = state.to_le_bytes();
    }
    out.truncate(mac_len);
    out
}
