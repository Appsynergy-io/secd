#![allow(non_snake_case)]

mod common;

use std::io::Write;
use std::process::Stdio;

use zeroize::Zeroize;

use common::{assert_no_value, gitea_blob, sha256, utf8, Harness, FIXTURE, GITEA_HOST, GITEA_URL};

fn request(host: &str) -> String {
    format!("protocol=https\nhost={host}\n\n")
}

fn password_field(body: &[u8]) -> Option<&[u8]> {
    for line in body.split(|b| *b == b'\n') {
        if let Some(rest) = line.strip_prefix(b"password=") {
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

#[test]
fn T_GIT_PARENT_NOT_GIT() {
    let h = Harness::new(&[("kv/gitea", gitea_blob(GITEA_URL, "t7user"))]);
    let mut child = h.command(&["git-credential"]);
    let mut proc = child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git-credential");
    if let Some(stdin) = proc.stdin.as_mut() {
        let _ = stdin.write_all(request(GITEA_HOST).as_bytes());
    }
    let out = proc.wait_with_output().expect("git-credential");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(
        !stdout.contains("password="),
        "non-git parent must not print a password line"
    );
    assert_no_value(&stdout, "git-not-git stdout");
    assert_no_value(&stderr, "git-not-git stderr");
}

#[test]
fn T_GIT_WRONG_HOST() {
    let h = Harness::new(&[("kv/gitea", gitea_blob(GITEA_URL, "t7user"))]);
    let out = h.git_credential(&request("evil.test"));
    let mut stdout = out.stdout;
    let stderr = utf8(&out.stderr);
    assert!(
        password_field(&stdout).is_none(),
        "wrong host must refuse a password line"
    );
    let text = utf8(&stdout);
    stdout.zeroize();
    assert_no_value(&text, "git-wrong-host stdout");
    assert_no_value(&stderr, "git-wrong-host stderr");
}

#[test]
fn T_GIT_OK() {
    let h = Harness::new(&[("kv/gitea", gitea_blob(GITEA_URL, "t7user"))]);
    let out = h.git_credential(&request(GITEA_HOST));
    let mut body = out.stdout;
    assert!(!body.is_empty(), "password field buffer must be non-empty");
    let want = sha256(FIXTURE.as_bytes());
    let got = match password_field(&body) {
        Some(p) => sha256(p),
        None => {
            body.zeroize();
            panic!("matching host must produce a password field");
        }
    };
    body.zeroize();
    assert_eq!(got, want, "password hash mismatch");
    let stderr = utf8(&out.stderr);
    assert_no_value(&stderr, "git-ok stderr");
}
