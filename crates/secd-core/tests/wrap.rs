#![allow(non_snake_case)]

use secd_core::{
    remove_wrap, unwrap_passkey, unwrap_password, wrap_passkey, wrap_password, Error, Factor, Wrap,
};

const DEK: [u8; 32] = [
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
    0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf,
];
const PASSWORD: &[u8] = b"twelve chars.";
const WRONG_PASSWORD: &[u8] = b"not-the-password";
const PRF: [u8; 32] = [0x5c; 32];
const WRONG_PRF: [u8; 32] = [0xa5; 32];
const CRED_ID: &str = "cred-1";

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.len().is_multiple_of(2) && s.bytes().all(|b: u8| b.is_ascii_hexdigit())
}

fn dummy_wrap(factor: Factor) -> Wrap {
    Wrap {
        factor,
        cred_id: match factor {
            Factor::Passkey => Some(CRED_ID.to_string()),
            Factor::Password => None,
        },
        salt: match factor {
            Factor::Password => Some("00".repeat(16)),
            Factor::Passkey => None,
        },
        blob: "00".repeat(72),
    }
}

#[test]
fn T_WRAP_PASSWORD() {
    let wrap = wrap_password(&DEK, PASSWORD).expect("wrap_password");
    assert_eq!(wrap.factor, Factor::Password);
    assert_eq!(wrap.factor.as_str(), "password");
    assert!(wrap.cred_id.is_none());
    let salt = wrap.salt.as_deref().expect("password wrap has salt");
    assert!(is_hex(salt), "salt is hex");
    assert_eq!(salt.len(), 32, "salt is 16 bytes");
    assert!(is_hex(&wrap.blob), "blob is hex");
    // nonce 24 + DEK 32 + Poly1305 tag 16
    assert_eq!(wrap.blob.len(), 144, "AEAD blob is 72 bytes");
    let opened = unwrap_password(&wrap, PASSWORD).expect("unwrap_password");
    assert_eq!(opened.as_bytes(), DEK.as_slice());
}

#[test]
fn T_WRAP_PASSWORD_WRONG() {
    let wrap = wrap_password(&DEK, PASSWORD).expect("wrap_password");
    let err = unwrap_password(&wrap, WRONG_PASSWORD).expect_err("wrong password must fail unwrap");
    assert_eq!(err, Error::Aead);
}

#[test]
fn T_WRAP_PASSKEY() {
    let wrap = wrap_passkey(&DEK, &PRF, CRED_ID).expect("wrap_passkey");
    assert_eq!(wrap.factor, Factor::Passkey);
    assert_eq!(wrap.factor.as_str(), "passkey");
    assert_eq!(wrap.cred_id.as_deref(), Some(CRED_ID));
    assert!(wrap.salt.is_none());
    assert!(is_hex(&wrap.blob), "blob is hex");
    assert_eq!(wrap.blob.len(), 144, "AEAD blob is 72 bytes");
    let opened = unwrap_passkey(&wrap, &PRF).expect("unwrap_passkey");
    assert_eq!(opened.as_bytes(), DEK.as_slice());
}

#[test]
fn T_WRAP_PASSKEY_WRONG() {
    let wrap = wrap_passkey(&DEK, &PRF, CRED_ID).expect("wrap_passkey");
    let err =
        unwrap_passkey(&wrap, &WRONG_PRF).expect_err("different 32-byte PRF must fail unwrap");
    assert_eq!(err, Error::Aead);
}

#[test]
fn T_WRAP_LAST_FACTOR() {
    let mut one = vec![dummy_wrap(Factor::Password)];
    assert_eq!(remove_wrap(&mut one, 0), Err(Error::LastFactor));
    assert_eq!(one.len(), 1);

    let mut wraps = vec![dummy_wrap(Factor::Password), dummy_wrap(Factor::Passkey)];
    let removed = remove_wrap(&mut wraps, 0).expect("removing a non-last factor");
    assert_eq!(removed.factor, Factor::Password);
    assert_eq!(wraps.len(), 1);
    assert_eq!(remove_wrap(&mut wraps, 0), Err(Error::LastFactor));
    assert_eq!(wraps.len(), 1);
    assert_eq!(wraps[0].factor, Factor::Passkey);
}

#[test]
fn T_WRAP_TWO_FACTORS() {
    let password = wrap_password(&DEK, PASSWORD).expect("wrap_password");
    let passkey = wrap_passkey(&DEK, &PRF, CRED_ID).expect("wrap_passkey");
    let from_password = unwrap_password(&password, PASSWORD).expect("password unwrap");
    let from_passkey = unwrap_passkey(&passkey, &PRF).expect("passkey unwrap");
    assert_eq!(from_password.as_bytes(), DEK.as_slice());
    assert_eq!(from_passkey.as_bytes(), DEK.as_slice());
    assert_eq!(from_password.as_bytes(), from_passkey.as_bytes());
}
