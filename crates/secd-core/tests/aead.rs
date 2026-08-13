#![allow(non_snake_case)]

use secd_core::{open, seal, Error};

const KEY: [u8; 32] = [
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00,
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
];
const NAME: &str = "kv/gitea/token";
const PLAIN: &[u8] = b"fixture-aead-plaintext";

#[test]
fn T_AEAD_ROUNDTRIP() {
    let blob = seal(&KEY, NAME, PLAIN).expect("seal");
    let opened = open(&KEY, NAME, &blob).expect("open");
    assert_eq!(opened.as_bytes(), PLAIN);
}

#[test]
fn T_AEAD_WRONG_NAME() {
    let blob = seal(&KEY, NAME, PLAIN).expect("seal");
    let err =
        open(&KEY, "kv/gitea/other", &blob).expect_err("open with a different name must fail");
    assert_eq!(err, Error::Aead);
}

#[test]
fn T_AEAD_TRUNCATED() {
    let blob = seal(&KEY, NAME, PLAIN).expect("seal");
    assert!(blob.len() >= 25, "sealed blob must be at least 25 bytes");
    for n in 0..25 {
        let err = open(&KEY, NAME, &blob[..n]).expect_err("blob shorter than 25 bytes must fail");
        assert_eq!(err, Error::Truncated, "len {n}");
        let err = open(&KEY, NAME, &vec![0u8; n]).expect_err("zero blob shorter than 25 must fail");
        assert_eq!(err, Error::Truncated, "zeros len {n}");
    }
}

#[test]
fn T_AEAD_FLIP_BYTE() {
    let blob = seal(&KEY, NAME, PLAIN).expect("seal");
    assert!(blob.len() > 24, "blob is nonce || ciphertext");
    for i in 0..blob.len() {
        for bit in 0..8 {
            let mut flipped = blob.clone();
            flipped[i] ^= 1 << bit;
            let err = open(&KEY, NAME, &flipped).expect_err("any bit flip in ciphertext must fail");
            assert_eq!(err, Error::Aead, "byte {i} bit {bit}");
        }
    }
    let opened = open(&KEY, NAME, &blob).expect("unflipped blob still opens");
    assert_eq!(opened.as_bytes(), PLAIN);
}

#[test]
fn T_AEAD_EMPTY() {
    let blob = seal(&KEY, NAME, b"").expect("seal empty");
    let opened = open(&KEY, NAME, &blob).expect("open empty");
    assert!(opened.is_empty());
    assert_eq!(opened.as_bytes(), b"");
}
