#![allow(non_snake_case)]
//! The release scripts had no coverage at all: they were first executed on
//! main, where a failure is already a published failure. Two of the first four
//! release runs failed that way. merge-latest-json.sh is pure data
//! transformation, so it is the cheapest of them to pin down.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(1);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch() -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("secd-merge-{}-{n}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn fragment(dir: &PathBuf, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("write fragment");
    path
}

fn merge(args: &[&str]) -> Output {
    Command::new(root().join("scripts/merge-latest-json.sh"))
        .args(args)
        .output()
        .expect("run merge-latest-json.sh")
}

fn target(triple: &str) -> String {
    format!(
        r#"{{"sha256":"{sha}","sig":"https://example/{triple}.sig","url":"https://example/{triple}"}}"#,
        sha = "a".repeat(64),
    )
}

fn manifest(version: &str, triple: &str) -> String {
    format!(
        r#"{{"version":"{version}","targets":{{"{triple}":{}}}}}"#,
        target(triple)
    )
}

/// Two platform fragments merge into one manifest naming both.
#[test]
fn T_MERGE_LATEST_UNION() {
    let dir = scratch();
    let linux = fragment(
        &dir,
        "linux.json",
        &manifest("0.1.10", "x86_64-unknown-linux-musl"),
    );
    let darwin = fragment(
        &dir,
        "darwin.json",
        &manifest("0.1.10", "aarch64-apple-darwin"),
    );
    let out = dir.join("latest.json");

    let result = merge(&[
        "-o",
        out.to_str().unwrap(),
        linux.to_str().unwrap(),
        darwin.to_str().unwrap(),
    ]);
    assert!(
        result.status.success(),
        "merge failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let merged = fs::read_to_string(&out).expect("merged manifest");
    assert!(merged.contains("x86_64-unknown-linux-musl"), "{merged}");
    assert!(merged.contains("aarch64-apple-darwin"), "{merged}");
    assert!(merged.contains(r#""version": "0.1.10""#), "{merged}");
    fs::remove_dir_all(&dir).ok();
}

/// Fragments from different versions must never be merged: that is how a
/// half-released manifest would claim to describe a version it does not.
#[test]
fn T_MERGE_LATEST_VERSION_MISMATCH() {
    let dir = scratch();
    let a = fragment(
        &dir,
        "a.json",
        &manifest("0.1.10", "x86_64-unknown-linux-musl"),
    );
    let b = fragment(&dir, "b.json", &manifest("0.1.11", "aarch64-apple-darwin"));
    let out = dir.join("latest.json");

    let result = merge(&[
        "-o",
        out.to_str().unwrap(),
        a.to_str().unwrap(),
        b.to_str().unwrap(),
    ]);
    assert!(!result.status.success(), "mismatched versions were merged");
    assert!(!out.exists(), "a manifest was written despite the mismatch");
    fs::remove_dir_all(&dir).ok();
}

/// A target missing `sig` would publish a manifest `secd update` cannot verify.
#[test]
fn T_MERGE_LATEST_MISSING_KEY() {
    let dir = scratch();
    let bad = fragment(
        &dir,
        "bad.json",
        &format!(
            r#"{{"version":"0.1.10","targets":{{"x86_64-unknown-linux-musl":{{"sha256":"{}","url":"https://example/x"}}}}}}"#,
            "a".repeat(64)
        ),
    );
    let out = dir.join("latest.json");

    let result = merge(&["-o", out.to_str().unwrap(), bad.to_str().unwrap()]);
    assert!(
        !result.status.success(),
        "a target without sig was accepted"
    );
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("sig"),
        "error did not name the missing key: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_dir_all(&dir).ok();
}

/// install.sh verifies against a key embedded in itself, because fetching
/// cosign.pub from the release you are verifying is circular. That embedded
/// copy has to stay equal to the one the release publishes.
#[test]
fn T_INSTALL_PUBKEY() {
    let script = fs::read_to_string(root().join("packaging/install.sh")).expect("install.sh");
    let embedded = script
        .split_once("<<'SECDpub_EOF'\n")
        .and_then(|(_, rest)| rest.split_once("\nSECDpub_EOF"))
        .map(|(body, _)| format!("{body}\n"))
        .expect("install.sh has no embedded cosign.pub heredoc");
    let on_disk = fs::read_to_string(root().join("keys/cosign.pub")).expect("keys/cosign.pub");
    assert_eq!(
        embedded, on_disk,
        "the pubkey embedded in packaging/install.sh differs from keys/cosign.pub"
    );

    assert!(
        script.contains("openssl dgst -sha256 -verify"),
        "install.sh no longer verifies the signature"
    );
}
