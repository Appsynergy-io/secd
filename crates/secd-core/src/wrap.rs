use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::OsRng;
use zeroize::Zeroize;

use crate::{open, seal, Error, Secret};

const DEK_LEN: usize = 32;
const KEK_LEN: usize = 32;
const SALT_LEN: usize = 16;
const ARGON2_M_KIB: u32 = 19_456;
const ARGON2_T: u32 = 2;
const ARGON2_P: u32 = 1;

const FACTOR_PASSKEY: &str = "passkey";
const FACTOR_PASSWORD: &str = "password";

/// Wrap row: `{factor: "passkey"|"password", cred_id?, salt?: hex, blob: hex}`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wrap {
    pub factor: Factor,
    pub cred_id: Option<String>,
    pub salt: Option<String>,
    pub blob: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Factor {
    Passkey,
    Password,
}

impl Factor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passkey => FACTOR_PASSKEY,
            Self::Password => FACTOR_PASSWORD,
        }
    }
}

/// Wrap `dek` (32 bytes) with argon2id KEK (m=19456 KiB, t=2, p=1, salt 16, out 32).
pub fn wrap_password(dek: &[u8], password: &[u8]) -> Result<Wrap, Error> {
    check_dek(dek)?;
    let mut salt = [0u8; SALT_LEN];
    OsRng.try_fill_bytes(&mut salt).map_err(|_| Error::Rng)?;
    let mut kek = derive_password_kek(password, &salt)?;
    let blob = seal(&kek, FACTOR_PASSWORD, dek)?;
    kek.zeroize();
    let salt_hex = to_hex(&salt);
    salt.zeroize();
    Ok(Wrap {
        factor: Factor::Password,
        cred_id: None,
        salt: Some(salt_hex),
        blob: to_hex(&blob),
    })
}

pub fn unwrap_password(wrap: &Wrap, password: &[u8]) -> Result<Secret, Error> {
    if wrap.factor != Factor::Password {
        return Err(Error::Factor);
    }
    let salt_hex = wrap.salt.as_deref().ok_or(Error::Factor)?;
    let mut salt = from_hex(salt_hex)?;
    if salt.len() != SALT_LEN {
        salt.zeroize();
        return Err(Error::Hex);
    }
    let mut kek = derive_password_kek(password, &salt)?;
    salt.zeroize();
    let blob = from_hex(&wrap.blob)?;
    let dek = open(&kek, FACTOR_PASSWORD, &blob);
    kek.zeroize();
    dek
}

/// Wrap `dek` with KEK = first 32 bytes of WebAuthn PRF. Shorter PRF is refused.
pub fn wrap_passkey(dek: &[u8], prf: &[u8], cred_id: &str) -> Result<Wrap, Error> {
    check_dek(dek)?;
    let mut kek = prf_kek(prf)?;
    let blob = seal(&kek, FACTOR_PASSKEY, dek)?;
    kek.zeroize();
    Ok(Wrap {
        factor: Factor::Passkey,
        cred_id: Some(cred_id.to_string()),
        salt: None,
        blob: to_hex(&blob),
    })
}

pub fn unwrap_passkey(wrap: &Wrap, prf: &[u8]) -> Result<Secret, Error> {
    if wrap.factor != Factor::Passkey {
        return Err(Error::Factor);
    }
    let mut kek = prf_kek(prf)?;
    let blob = from_hex(&wrap.blob)?;
    let dek = open(&kek, FACTOR_PASSKEY, &blob);
    kek.zeroize();
    dek
}

/// Remove the wrap at `index`. Error if it is the last factor.
pub fn remove_wrap(wraps: &mut Vec<Wrap>, index: usize) -> Result<Wrap, Error> {
    if wraps.len() <= 1 {
        return Err(Error::LastFactor);
    }
    if index >= wraps.len() {
        return Err(Error::Factor);
    }
    Ok(wraps.remove(index))
}

fn check_dek(dek: &[u8]) -> Result<(), Error> {
    if dek.len() != DEK_LEN {
        return Err(Error::Key);
    }
    Ok(())
}

fn prf_kek(prf: &[u8]) -> Result<[u8; KEK_LEN], Error> {
    if prf.len() < KEK_LEN {
        return Err(Error::Prf);
    }
    let mut kek = [0u8; KEK_LEN];
    kek.copy_from_slice(&prf[..KEK_LEN]);
    Ok(kek)
}

fn derive_password_kek(password: &[u8], salt: &[u8]) -> Result<[u8; KEK_LEN], Error> {
    let params = Params::new(ARGON2_M_KIB, ARGON2_T, ARGON2_P, Some(KEK_LEN))
        .expect("invariant: locked argon2id params are valid");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut kek = [0u8; KEK_LEN];
    argon2
        .hash_password_into(password, salt, &mut kek)
        .map_err(|_| Error::Aead)?;
    Ok(kek)
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn from_hex(s: &str) -> Result<Vec<u8>, Error> {
    if !s.len().is_multiple_of(2) {
        return Err(Error::Hex);
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Result<u8, Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(Error::Hex),
    }
}
