#![allow(non_snake_case)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(1);

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("secd-t8-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).expect("scratch");
    Scratch(p)
}

struct Signed {
    _dir: Scratch,
    dest: PathBuf,
    orig: Vec<u8>,
    payload: Vec<u8>,
    sha: String,
    sig: Vec<u8>,
    pub_pem: String,
}

fn signed(orig: &[u8], payload: &[u8]) -> Signed {
    let dir = scratch("upd");
    let dest = dir.0.join("secd");
    fs::write(&dest, orig).expect("orig dest");

    let key = dir.0.join("key.pem");
    let pubp = dir.0.join("pub.pem");
    let pay = dir.0.join("payload");
    let sigp = dir.0.join("sig");
    fs::write(&pay, payload).expect("payload");

    let st = Command::new("openssl")
        .args([
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-out",
        ])
        .arg(&key)
        .status()
        .expect("openssl genpkey");
    assert!(st.success(), "openssl genpkey");

    let st = Command::new("openssl")
        .args(["pkey", "-in"])
        .arg(&key)
        .args(["-pubout", "-out"])
        .arg(&pubp)
        .status()
        .expect("openssl pubout");
    assert!(st.success(), "openssl pubout");

    let st = Command::new("openssl")
        .args(["dgst", "-sha256", "-sign"])
        .arg(&key)
        .arg("-out")
        .arg(&sigp)
        .arg(&pay)
        .status()
        .expect("openssl sign");
    assert!(st.success(), "openssl sign");

    let pub_pem = fs::read_to_string(&pubp).expect("pub pem");
    let sig = fs::read(&sigp).expect("sig");
    let sha = secd::update::sha256_hex(payload);
    assert_eq!(sha.len(), 64, "sha256 hex");

    Signed {
        _dir: dir,
        dest,
        orig: orig.to_vec(),
        payload: payload.to_vec(),
        sha,
        sig,
        pub_pem,
    }
}

fn staging(dest: &Path) -> PathBuf {
    dest.parent().expect("dest parent").join("secd.new")
}

fn assert_argv0_untouched(s: &Signed) {
    let now = fs::read(&s.dest).expect("read dest");
    assert_eq!(now, s.orig, "argv[0] bytes changed");
    assert!(!staging(&s.dest).exists(), "staging secd.new remains");
}

#[test]
fn T_UPD_BAD_HASH() {
    let s = signed(b"argv0-old", b"payload-new");
    let bad = "0".repeat(64);
    let err = secd::update::apply(&s.dest, &s.payload, &bad, &s.sig, &s.pub_pem);
    assert!(err.is_err(), "bad hash must fail");
    assert_argv0_untouched(&s);
}

#[test]
fn T_UPD_BAD_SIG() {
    let s = signed(b"argv0-old", b"payload-new");
    let mut bad = s.sig.clone();
    assert!(!bad.is_empty(), "empty signature");
    bad[0] ^= 0xff;
    let err = secd::update::apply(&s.dest, &s.payload, &s.sha, &bad, &s.pub_pem);
    assert!(err.is_err(), "bad sig must fail");
    assert_argv0_untouched(&s);
}

#[test]
fn T_UPD_WRONG_HOST() {
    let dir = scratch("host");
    let dest = dir.0.join("secd");
    let orig = b"argv0-old";
    fs::write(&dest, orig).expect("orig dest");

    let err = secd::update::apply_from_url(
        &dest,
        "https://evil.example/latest.json",
        false,
        "-----BEGIN PUBLIC KEY-----\nMIIB\n-----END PUBLIC KEY-----\n",
    );
    assert!(err.is_err(), "wrong host must refuse");
    assert_eq!(fs::read(&dest).expect("read dest"), orig, "argv[0] changed");
    assert!(!staging(&dest).exists(), "staging written");

    assert!(!secd::update::url_allowed("https://evil.example/x"));
    assert!(!secd::update::url_allowed(""));
    assert!(!secd::update::url_allowed("http://git.appsynergy.io/x"));
    assert!(!secd::update::url_allowed("http://github.com/x"));
    assert!(!secd::update::url_allowed(
        "https://git.appsynergy.io.evil.com/x"
    ));
    assert!(!secd::update::url_allowed("https://github.com.evil.com/x"));
    assert!(!secd::update::url_allowed(
        "https://user@git.appsynergy.io/x"
    ));
    assert!(!secd::update::url_allowed("https://user@github.com/x"));

    let triple = secd::update::target_triple().expect("triple");
    let manifest = format!(
        r#"{{"version":"9.9.9","targets":{{"{triple}":{{"url":"https://evil.example/secd","sha256":"{}","sig":"https://evil.example/secd.sig"}}}}}}"#,
        "ab".repeat(32)
    );
    let parsed = secd::update::parse_manifest(manifest.as_bytes(), triple);
    assert!(parsed.is_err(), "manifest url not git.appsynergy.io");
    assert_eq!(fs::read(&dest).expect("read dest"), orig, "argv[0] changed");
    assert!(!staging(&dest).exists(), "staging written");
}

#[test]
fn T_UPD_HOST_GITHUB() {
    assert!(secd::update::url_allowed(
        "https://github.com/Appsynergy-io/secd/releases/latest/download/latest.json"
    ));
    assert!(secd::update::url_allowed(
        "https://github.com/Appsynergy-io/secd/releases/download/v0.1.10/secd-x86_64-unknown-linux-musl"
    ));
    assert!(secd::update::url_allowed(
        "https://release-assets.githubusercontent.com/foo"
    ));
    assert!(secd::update::url_allowed(
        "https://objects.githubusercontent.com/foo"
    ));
    assert!(secd::update::url_allowed("https://git.appsynergy.io/x"));

    let triple = secd::update::target_triple().expect("triple");
    let url = "https://github.com/Appsynergy-io/secd/releases/download/v0.1.10/secd-x86_64-unknown-linux-musl";
    let sig = "https://github.com/Appsynergy-io/secd/releases/download/v0.1.10/secd-x86_64-unknown-linux-musl.sig";
    let manifest = format!(
        r#"{{"version":"0.1.10","targets":{{"{triple}":{{"url":"{url}","sha256":"{}","sig":"{sig}"}}}}}}"#,
        "ab".repeat(32)
    );
    let parsed =
        secd::update::parse_manifest(manifest.as_bytes(), triple).expect("github.com asset urls");
    assert_eq!(parsed.url, url);
    assert_eq!(parsed.sig, sig);
}

#[test]
fn T_UPD_REDIRECT_OFFLIST() {
    let dir = scratch("redir");
    let dest = dir.0.join("secd");
    let orig = b"argv0-old";
    fs::write(&dest, orig).expect("orig dest");

    assert!(
        secd::update::next_redirect(302, Some("https://evil.example/x"), "https://github.com/x",)
            .is_err(),
        "off-list Location must fail"
    );

    let err = secd::update::apply_from_url(
        &dest,
        "https://evil.example/latest.json",
        false,
        "-----BEGIN PUBLIC KEY-----\nMIIB\n-----END PUBLIC KEY-----\n",
    );
    assert!(err.is_err(), "wrong host must refuse");
    assert_eq!(fs::read(&dest).expect("read dest"), orig, "argv[0] changed");
    assert!(!staging(&dest).exists(), "staging written");

    assert_eq!(
        secd::update::next_redirect(
            302,
            Some("https://release-assets.githubusercontent.com/path"),
            "https://github.com/x",
        )
        .expect("on-list redirect"),
        Some("https://release-assets.githubusercontent.com/path".into())
    );
    assert!(
        secd::update::next_redirect(302, None, "https://github.com/x").is_err(),
        "redirect without Location must fail"
    );
    assert_eq!(
        secd::update::next_redirect(200, None, "https://github.com/x").expect("200"),
        None
    );
}

#[test]
fn T_UPD_GOOD() {
    let s = signed(b"argv0-old", b"payload-new-t8");
    secd::update::apply(&s.dest, &s.payload, &s.sha, &s.sig, &s.pub_pem).expect("apply");
    assert_eq!(
        fs::read(&s.dest).expect("read dest"),
        s.payload,
        "new bytes != payload"
    );
    assert!(!staging(&s.dest).exists(), "staging secd.new remains");
}

#[test]
fn T_UPD_PACMAN() {
    let dest = Path::new("/usr/bin/true");
    assert!(
        dest.is_file(),
        "/usr/bin/true missing; cannot assert pacman refuse"
    );
    if !secd::update::pacman_owns(dest) {
        return; // runner is not Arch / dest not pacman-owned
    }
    let orig = fs::read(dest).expect("read pacman dest");
    let s = signed(b"unused", b"payload-new-t8");
    let err = secd::update::apply(dest, &s.payload, &s.sha, &s.sig, &s.pub_pem);
    assert!(err.is_err(), "pacman-owned path must refuse");
    let msg = format!("{:#}", err.unwrap_err());
    assert!(msg.contains("pacman"), "refuse must name pacman, got {msg}");
    assert_eq!(
        fs::read(dest).expect("reread dest"),
        orig,
        "argv[0] changed"
    );
    assert!(
        !Path::new("/usr/bin/secd.new").exists(),
        "staging secd.new remains"
    );
}
