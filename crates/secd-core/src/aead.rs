use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

use crate::{Error, Secret};

const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
/// Nonce (24) plus at least one ciphertext byte. Shorter blobs fail before decrypt.
const MIN_BLOB: usize = NONCE_LEN + 1;

/// Seal `plaintext` with XChaCha20-Poly1305. AAD is `name` UTF-8. Blob is nonce || ciphertext.
pub fn seal(key: &[u8], name: &str, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    let cipher = cipher(key)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|_| Error::Rng)?;
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: name.as_bytes(),
            },
        )
        .map_err(|_| Error::Aead)?;
    let mut blob = Vec::with_capacity(NONCE_LEN + ct.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct);
    Ok(blob)
}

/// Open `blob` (nonce || ciphertext). AAD is `name` UTF-8. Blobs shorter than 25 bytes fail.
pub fn open(key: &[u8], name: &str, blob: &[u8]) -> Result<Secret, Error> {
    if blob.len() < MIN_BLOB {
        return Err(Error::Truncated);
    }
    let cipher = cipher(key)?;
    let nonce = XNonce::from_slice(&blob[..NONCE_LEN]);
    let pt = cipher
        .decrypt(
            nonce,
            Payload {
                msg: &blob[NONCE_LEN..],
                aad: name.as_bytes(),
            },
        )
        .map_err(|_| Error::Aead)?;
    Ok(Secret::new(pt))
}

fn cipher(key: &[u8]) -> Result<XChaCha20Poly1305, Error> {
    if key.len() != KEY_LEN {
        return Err(Error::Key);
    }
    XChaCha20Poly1305::new_from_slice(key).map_err(|_| Error::Key)
}
