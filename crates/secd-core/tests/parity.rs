#![allow(non_snake_case)]

use std::fs;
use std::path::Path;

#[test]
fn T_CRYPTO_PARITY_FIXTURE() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crypto-parity.json");
    assert!(path.is_file(), "crypto-parity fixture missing");
    let committed = fs::read_to_string(&path).expect("read fixture");
    if committed != secd_core::parity::json() {
        panic!("crypto-parity fixture is stale");
    }
    secd_core::parity::verify().expect("secd-core opens the fixture");
}
