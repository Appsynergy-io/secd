#![allow(non_snake_case)]

mod common;

use std::io::Write;
use std::process::Stdio;

use zeroize::Zeroize;

use common::{
    assert_no_value, gitea_blob, github_blob, sha256, utf8, Harness, FIXTURE, GITEA_HOST, GITEA_URL,
};

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

fn github_meta() -> serde_json::Value {
    serde_json::json!({"provider": "github", "fields": ["token", "user"]})
}

fn username_field(body: &[u8]) -> Option<&[u8]> {
    for line in body.split(|b| *b == b'\n') {
        if let Some(rest) = line.strip_prefix(b"username=") {
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

#[test]
fn T_GIT_ACTION_GET() {
    // What git actually runs. Before this, clap rejected the argument and the
    // helper answered nothing at all.
    let h = Harness::new(&[("kv/gitea", gitea_blob(GITEA_URL, "t7user"))]);
    let out = h.git_credential_action(&request(GITEA_HOST), "get");
    let mut body = out.stdout;
    let got = match password_field(&body) {
        Some(p) => sha256(p),
        None => {
            body.zeroize();
            panic!("`get` must produce a password field");
        }
    };
    body.zeroize();
    assert_eq!(got, sha256(FIXTURE.as_bytes()), "password hash mismatch");
    assert_no_value(&utf8(&out.stderr), "git-get stderr");
}

#[test]
fn T_GIT_ACTION_ERASE() {
    // `store` and `erase` are git reporting, not asking. Answering them hands
    // out a credential nobody requested.
    let h = Harness::new(&[("kv/gitea", gitea_blob(GITEA_URL, "t7user"))]);
    for action in ["store", "erase"] {
        let out = h.git_credential_action(&request(GITEA_HOST), action);
        let mut body = out.stdout;
        let leaked = password_field(&body).is_some();
        let text = utf8(&body);
        body.zeroize();
        assert!(!leaked, "{action} must not produce a password field");
        assert_no_value(&text, "git-erase stdout");
        assert_no_value(&utf8(&out.stderr), "git-erase stderr");
    }
}

#[test]
fn T_GIT_GITHUB() {
    // github's schema carries no url, so the host is the provider's own.
    let h = Harness::new_with_meta(&[("kv/github", github_blob("t7user"), github_meta())]);
    let out = h.git_credential_action(&request("github.com"), "get");
    let mut body = out.stdout;
    let user = username_field(&body).map(<[u8]>::to_vec);
    let got = match password_field(&body) {
        Some(p) => sha256(p),
        None => {
            body.zeroize();
            panic!("a github bundle must serve github.com");
        }
    };
    body.zeroize();
    assert_eq!(got, sha256(FIXTURE.as_bytes()), "password hash mismatch");
    assert_eq!(user.as_deref(), Some(&b"t7user"[..]), "username mismatch");
    assert_no_value(&utf8(&out.stderr), "git-github stderr");
}

#[test]
fn T_GIT_GITHUB_WRONG_HOST() {
    // A github bundle is not a fallback for every host git cannot place.
    let h = Harness::new_with_meta(&[("kv/github", github_blob("t7user"), github_meta())]);
    let out = h.git_credential_action(&request("evil.test"), "get");
    let mut body = out.stdout;
    let leaked = password_field(&body).is_some();
    let text = utf8(&body);
    body.zeroize();
    assert!(!leaked, "github must not answer for another host");
    assert_no_value(&text, "git-github-wrong-host stdout");
    assert_no_value(&utf8(&out.stderr), "git-github-wrong-host stderr");
}

#[test]
fn T_GIT_GITHUB_SIBLINGS() {
    let h = Harness::new_with_meta(&[
        (
            "kv/github/token",
            FIXTURE.as_bytes().to_vec(),
            github_meta(),
        ),
        ("kv/github/user", b"t7user".to_vec(), github_meta()),
    ]);
    let out = h.git_credential_action(&request("github.com"), "get");
    let mut body = out.stdout;
    let user = username_field(&body).map(<[u8]>::to_vec);
    let got = match password_field(&body) {
        Some(p) => sha256(p),
        None => {
            body.zeroize();
            panic!("a github sibling pair with provider meta must serve github.com");
        }
    };
    body.zeroize();
    assert_eq!(got, sha256(FIXTURE.as_bytes()), "password hash mismatch");
    assert_eq!(user.as_deref(), Some(&b"t7user"[..]), "username mismatch");
    assert_no_value(&utf8(&out.stderr), "git-github-siblings stderr");
}
