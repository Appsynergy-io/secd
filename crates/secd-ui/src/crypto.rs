//! Client AEAD and wraps. Same locked params as secd-core. JS cannot zeroize; we overwrite.
//! The DEK lives only in a Zeroizing signal for the tab's lifetime — never in
//! storage — so a reload drops it until the next sign-in.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde_json::{json, Value};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const ARGON2_M_KIB: u32 = 19_456;
const ARGON2_T: u32 = 2;
const ARGON2_P: u32 = 1;

#[derive(Clone, Debug)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CryptoError {
    Key,
    Aead,
    Truncated,
    Hex,
    Rng,
    Prf,
    Factor,
}

pub fn mint_dek() -> [u8; KEY_LEN] {
    let mut dek = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut dek);
    dek
}

pub fn zeroize_bytes(buf: &mut [u8]) {
    buf.zeroize();
}

pub fn seal(key: &[u8], name: &str, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = cipher(key)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|_| CryptoError::Rng)?;
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: name.as_bytes(),
            },
        )
        .map_err(|_| CryptoError::Aead)?;
    let mut blob = Vec::with_capacity(NONCE_LEN + ct.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct);
    Ok(blob)
}

pub fn open(key: &[u8], name: &str, blob: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if blob.len() < NONCE_LEN + 1 {
        return Err(CryptoError::Truncated);
    }
    let cipher = cipher(key)?;
    let nonce = XNonce::from_slice(&blob[..NONCE_LEN]);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &blob[NONCE_LEN..],
                aad: name.as_bytes(),
            },
        )
        .map_err(|_| CryptoError::Aead)
}

fn cipher(key: &[u8]) -> Result<XChaCha20Poly1305, CryptoError> {
    if key.len() != KEY_LEN {
        return Err(CryptoError::Key);
    }
    XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::Key)
}

pub fn wrap_password(dek: &[u8], password: &[u8]) -> Result<Wrap, CryptoError> {
    if dek.len() != KEY_LEN {
        return Err(CryptoError::Key);
    }
    let mut salt = [0u8; SALT_LEN];
    OsRng
        .try_fill_bytes(&mut salt)
        .map_err(|_| CryptoError::Rng)?;
    let mut kek = derive_password_kek(password, &salt)?;
    let blob = seal(&kek, "password", dek)?;
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

pub fn unwrap_password(wrap: &Wrap, password: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if wrap.factor != Factor::Password {
        return Err(CryptoError::Factor);
    }
    let salt_hex = wrap.salt.as_deref().ok_or(CryptoError::Factor)?;
    let mut salt = from_hex(salt_hex)?;
    if salt.len() != SALT_LEN {
        salt.zeroize();
        return Err(CryptoError::Hex);
    }
    let mut kek = derive_password_kek(password, &salt)?;
    salt.zeroize();
    let blob = from_hex(&wrap.blob)?;
    let dek = open(&kek, "password", &blob);
    kek.zeroize();
    dek
}

pub fn wrap_passkey(dek: &[u8], prf: &[u8], cred_id: &str) -> Result<Wrap, CryptoError> {
    if dek.len() != KEY_LEN {
        return Err(CryptoError::Key);
    }
    let mut kek = prf_kek(prf)?;
    let blob = seal(&kek, "passkey", dek)?;
    kek.zeroize();
    Ok(Wrap {
        factor: Factor::Passkey,
        cred_id: Some(cred_id.to_string()),
        salt: None,
        blob: to_hex(&blob),
    })
}

pub fn unwrap_passkey(wrap: &Wrap, prf: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if wrap.factor != Factor::Passkey {
        return Err(CryptoError::Factor);
    }
    let mut kek = prf_kek(prf)?;
    let blob = from_hex(&wrap.blob)?;
    let dek = open(&kek, "passkey", &blob);
    kek.zeroize();
    dek
}

pub fn wrap_from_json(v: &Value) -> Option<Wrap> {
    let factor = match v.get("factor")?.as_str()? {
        "passkey" => Factor::Passkey,
        "password" => Factor::Password,
        _ => return None,
    };
    Some(Wrap {
        factor,
        cred_id: v
            .get("cred_id")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        salt: v.get("salt").and_then(|x| x.as_str()).map(str::to_string),
        blob: v.get("blob")?.as_str()?.to_string(),
    })
}

pub fn wrap_to_json(w: &Wrap) -> Value {
    let mut m = serde_json::Map::new();
    let factor = match w.factor {
        Factor::Passkey => "passkey",
        Factor::Password => "password",
    };
    m.insert("factor".into(), json!(factor));
    if let Some(c) = &w.cred_id {
        m.insert("cred_id".into(), json!(c));
    }
    if let Some(s) = &w.salt {
        m.insert("salt".into(), json!(s));
    }
    m.insert("blob".into(), json!(w.blob));
    Value::Object(m)
}

pub fn wraps_from_json(v: &Value) -> Vec<Wrap> {
    v.get("wraps")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(wrap_from_json).collect())
        .unwrap_or_default()
}

pub fn unwrap_any(wraps: &[Wrap], password: Option<&[u8]>, prf: Option<&[u8]>) -> Option<Vec<u8>> {
    if let Some(prf) = prf {
        for w in wraps {
            if w.factor == Factor::Passkey {
                if let Ok(dek) = unwrap_passkey(w, prf) {
                    return Some(dek);
                }
            }
        }
    }
    if let Some(password) = password {
        for w in wraps {
            if w.factor == Factor::Password {
                if let Ok(dek) = unwrap_password(w, password) {
                    return Some(dek);
                }
            }
        }
    }
    None
}

pub fn seal_dek_to_eph(dek: &[u8], their_pub: &[u8]) -> Result<Value, CryptoError> {
    if dek.len() != KEY_LEN || their_pub.len() != KEY_LEN {
        return Err(CryptoError::Key);
    }
    let mut their = [0u8; 32];
    their.copy_from_slice(their_pub);
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    let shared = secret.diffie_hellman(&PublicKey::from(their));
    let blob = seal(shared.as_bytes(), "dek", dek)?;
    Ok(json!({
        "alg": "x25519-xchacha20poly1305",
        "eph_pub": to_hex(public.as_bytes()),
        "blob": to_hex(&blob),
    }))
}

fn prf_kek(prf: &[u8]) -> Result<[u8; KEY_LEN], CryptoError> {
    if prf.len() < KEY_LEN {
        return Err(CryptoError::Prf);
    }
    let mut kek = [0u8; KEY_LEN];
    kek.copy_from_slice(&prf[..KEY_LEN]);
    Ok(kek)
}

fn derive_password_kek(password: &[u8], salt: &[u8]) -> Result<[u8; KEY_LEN], CryptoError> {
    let params = Params::new(ARGON2_M_KIB, ARGON2_T, ARGON2_P, Some(KEY_LEN))
        .expect("invariant: locked argon2id params are valid");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut kek = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password, salt, &mut kek)
        .map_err(|_| CryptoError::Aead)?;
    Ok(kek)
}

pub fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn from_hex(s: &str) -> Result<Vec<u8>, CryptoError> {
    if !s.len().is_multiple_of(2) {
        return Err(CryptoError::Hex);
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

fn hex_val(b: u8) -> Result<u8, CryptoError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(CryptoError::Hex),
    }
}

pub fn check_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 256 {
        return false;
    }
    if name.starts_with('/') || name.ends_with('/') || name.contains("..") {
        return false;
    }
    name.split('/').all(|seg| {
        !seg.is_empty()
            && seg.bytes().all(|b| {
                matches!(
                    b,
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'@' | b'-'
                )
            })
    })
}

pub fn email_ok(raw: &str) -> Option<String> {
    let s = raw.trim().to_lowercase();
    if s.is_empty() || s.len() > 254 {
        return None;
    }
    let (local, domain) = s.split_once('@')?;
    if local.is_empty() || domain.is_empty() || domain.contains('@') || !domain.contains('.') {
        return None;
    }
    Some(s)
}

pub fn password_ok(password: &str) -> bool {
    let n = password.chars().count();
    (12..=256).contains(&n)
}
