#![allow(non_snake_case)]

use secd_core::redact;

const VALUE: &str = "complete-secret-value";

#[test]
fn T_REDACT_VALUE() {
    let output = "token=complete-secret-value; done";
    let got = redact(output, &[VALUE]);
    assert!(!got.contains(VALUE), "value must be replaced, got {got:?}");
    assert_eq!(got, "token=[redacted]; done");

    let twice = "complete-secret-value and complete-secret-value";
    let got = redact(twice, &[VALUE]);
    assert!(
        !got.contains(VALUE),
        "every occurrence is replaced: {got:?}"
    );
    assert_eq!(got, "[redacted] and [redacted]");
}

#[test]
fn T_REDACT_PARTIAL() {
    assert_eq!(redact("complete-secret", &[VALUE]), "complete-secret");
    assert_eq!(redact("secret-value", &[VALUE]), "secret-value");
    assert_eq!(redact("complete", &[VALUE]), "complete");
    assert_eq!(
        redact("prefix complete-secret suffix", &[VALUE]),
        "prefix complete-secret suffix"
    );
}
