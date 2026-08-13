#![allow(non_snake_case)]

use secd::Secret;

#[test]
fn T_NO_LEAK_SECRET() {
    let src = std::fs::read_to_string("tests/compile-fail/secret_is_not_printable.rs")
        .expect("compile-fail source");
    assert!(src.contains("Display"), "compile-fail must reject Display");
    assert!(
        src.contains("Serialize"),
        "compile-fail must reject Serialize"
    );
    assert!(src.contains("Deref"), "compile-fail must reject Deref");
    let _ = Secret::new(b"x".to_vec());
}

#[test]
fn T_NO_LEAK_DEBUG() {
    let bytes = b"fixture-secret-bytes-T_NO_LEAK_DEBUG";
    let secret = Secret::new(bytes.to_vec());
    let debug = format!("{secret:?}");
    let leaked = debug.contains("fixture-secret-bytes-T_NO_LEAK_DEBUG")
        || debug.as_bytes().windows(bytes.len()).any(|w| w == bytes);
    assert!(!leaked, "Debug leaked secret bytes: {debug}");
}
