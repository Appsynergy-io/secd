//! Dummy vectors for the bun `crypto-parity` lane. Inputs are fixtures, not secrets.

use x25519_dalek::{PublicKey, StaticSecret};

use crate::aead::{open, seal_with_nonce, NONCE_LEN};
use crate::wrap::{unwrap_passkey, unwrap_password, wrap_passkey_at, wrap_password_at, Wrap};
use crate::Error;

const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;

const AEAD_KEY: [u8; KEY_LEN] = [
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00,
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
];
const AEAD_NAME: &str = "kv/gitea/token";
const AEAD_PLAIN: &[u8] = b"fixture-aead-plaintext";
const AEAD_NONCE: [u8; NONCE_LEN] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
];

const DEK: [u8; KEY_LEN] = [
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
    0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf,
];
const PASSWORD: &[u8] = b"twelve chars.";
const SALT: [u8; SALT_LEN] = [
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
];
const PASSWORD_NONCE: [u8; NONCE_LEN] = [
    0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
    0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
];

const PRF: [u8; KEY_LEN] = [0x5c; KEY_LEN];
const CRED_ID: &str = "cred-1";
const PASSKEY_NONCE: [u8; NONCE_LEN] = [
    0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f, 0x50,
    0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
];

/// Same dummy scalars as `tests/cli.rs`. Not a low-order point.
const EPH_SK: [u8; KEY_LEN] = [0x62; KEY_LEN];
const PEER_SK: [u8; KEY_LEN] = [0x51; KEY_LEN];
const X25519_NONCE: [u8; NONCE_LEN] = [
    0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0x70,
    0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
];

struct Fixture {
    aead_blob: Vec<u8>,
    wrap_password: Wrap,
    wrap_passkey: Wrap,
    eph_pk: [u8; KEY_LEN],
    peer_pk: [u8; KEY_LEN],
    x25519_blob: Vec<u8>,
    x25519_shared: [u8; KEY_LEN],
}

fn build() -> Result<Fixture, Error> {
    let aead_blob = seal_with_nonce(&AEAD_KEY, AEAD_NAME, AEAD_PLAIN, &AEAD_NONCE)?;
    let wrap_password = wrap_password_at(&DEK, PASSWORD, &SALT, &PASSWORD_NONCE)?;
    let wrap_passkey = wrap_passkey_at(&DEK, &PRF, CRED_ID, &PASSKEY_NONCE)?;

    let eph = StaticSecret::from(EPH_SK);
    let peer = StaticSecret::from(PEER_SK);
    let eph_pk = PublicKey::from(&eph);
    let peer_pk = PublicKey::from(&peer);
    let shared = eph.diffie_hellman(&peer_pk);
    let mut x25519_shared = [0u8; KEY_LEN];
    x25519_shared.copy_from_slice(shared.as_bytes());
    let x25519_blob = seal_with_nonce(&x25519_shared, "dek", &DEK, &X25519_NONCE)?;

    Ok(Fixture {
        aead_blob,
        wrap_password,
        wrap_passkey,
        eph_pk: *eph_pk.as_bytes(),
        peer_pk: *peer_pk.as_bytes(),
        x25519_blob,
        x25519_shared,
    })
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Deterministic JSON the `crypto-parity` lane diffs against the committed fixture.
pub fn json() -> String {
    let f = build().expect("invariant: dummy vectors use valid keys");
    format!(
        "{{\n  \"aead\": {{\n    \"blob\": \"{}\",\n    \"k\": \"{}\",\n    \"name\": \"{}\",\n    \"nonce\": \"{}\",\n    \"plaintext\": \"{}\"\n  }},\n  \"wrap_passkey\": {{\n    \"blob\": \"{}\",\n    \"cred_id\": \"{}\",\n    \"dek\": \"{}\",\n    \"prf\": \"{}\"\n  }},\n  \"wrap_password\": {{\n    \"blob\": \"{}\",\n    \"dek\": \"{}\",\n    \"password\": \"{}\",\n    \"salt\": \"{}\"\n  }},\n  \"x25519\": {{\n    \"alg\": \"x25519-xchacha20poly1305\",\n    \"blob\": \"{}\",\n    \"dek\": \"{}\",\n    \"eph_pk\": \"{}\",\n    \"eph_sk\": \"{}\",\n    \"nonce\": \"{}\",\n    \"peer_pk\": \"{}\"\n  }}\n}}\n",
        hex(&f.aead_blob),
        hex(&AEAD_KEY),
        AEAD_NAME,
        hex(&AEAD_NONCE),
        hex(AEAD_PLAIN),
        f.wrap_passkey.blob,
        CRED_ID,
        hex(&DEK),
        hex(&PRF),
        f.wrap_password.blob,
        hex(&DEK),
        hex(PASSWORD),
        f.wrap_password.salt.as_deref().expect("invariant: password wrap has salt"),
        hex(&f.x25519_blob),
        hex(&DEK),
        hex(&f.eph_pk),
        hex(&EPH_SK),
        hex(&X25519_NONCE),
        hex(&f.peer_pk),
    )
}

/// secd-core opens every vector it emits.
pub fn verify() -> Result<(), Error> {
    let f = build()?;
    let pt = open(&AEAD_KEY, AEAD_NAME, &f.aead_blob)?;
    if pt.as_bytes() != AEAD_PLAIN {
        return Err(Error::Aead);
    }
    let from_pw = unwrap_password(&f.wrap_password, PASSWORD)?;
    if from_pw.as_bytes() != DEK {
        return Err(Error::Aead);
    }
    let from_pk = unwrap_passkey(&f.wrap_passkey, &PRF)?;
    if from_pk.as_bytes() != DEK {
        return Err(Error::Aead);
    }
    let opened = open(&f.x25519_shared, "dek", &f.x25519_blob)?;
    if opened.as_bytes() != DEK {
        return Err(Error::Aead);
    }
    Ok(())
}
