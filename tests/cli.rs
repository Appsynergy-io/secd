#![allow(non_snake_case)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use zeroize::Zeroize;

use common::{
    assert_no_value, env_lock, gitea_blob, isolated_secd, remove_var, set_var, sha256, utf8,
    with_secd_home, Harness, FIXTURE, GITEA_URL, LOCKED,
};

static SEQ: AtomicU64 = AtomicU64::new(1);

fn tmp(tag: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("secd-t7-cli-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).expect("tmp");
    p
}

#[test]
fn T_CLI_LS_NO_VALUE() {
    let h = Harness::new(&[("kv/note", FIXTURE.as_bytes().to_vec())]);
    let out = h.run(&["ls"]);
    assert!(out.status.success(), "ls failed");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(stdout.contains("kv/note"), "ls must print the name");
    assert_no_value(&stdout, "ls stdout");
    assert_no_value(&stderr, "ls stderr");
}

#[test]
fn T_CLI_INFO_NO_VALUE() {
    let h = Harness::new(&[("kv/note", FIXTURE.as_bytes().to_vec())]);
    let out = h.run(&["info", "kv/note"]);
    assert!(out.status.success(), "info failed");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(stdout.contains("kv/note"), "info must print the name");
    assert!(stdout.contains("bytes"), "info must print length");
    assert_no_value(&stdout, "info stdout");
    assert_no_value(&stderr, "info stderr");
}

#[test]
fn T_CLI_PROVIDERS_ENV() {
    let out = isolated_secd(&["providers"]).output().expect("providers");
    assert!(out.status.success(), "providers failed");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(
        stdout.contains("GITEA_TOKEN"),
        "providers must print GITEA_TOKEN"
    );
    assert_no_value(&stdout, "providers stdout");
    assert_no_value(&stderr, "providers stderr");
}

#[test]
fn T_CLI_GEN_LEN() {
    let h = Harness::new(&[]);
    let out = h.run(&["gen", "kv/t7gen"]);
    assert!(out.status.success(), "gen failed");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert_no_value(&stdout, "gen stdout");
    assert_no_value(&stderr, "gen stderr");
    let ok = gen_name_len_only(&stdout, "kv/t7gen");
    assert!(ok, "gen must print name and length only");
}

fn gen_name_len_only(stdout: &str, name: &str) -> bool {
    let t = stdout.trim();
    let Some((n, rest)) = t.split_once(' ') else {
        return false;
    };
    if n != name || rest.is_empty() {
        return false;
    }
    if t.contains('\n') {
        return false;
    }
    rest.bytes().all(|b| b.is_ascii_digit())
}

#[test]
fn T_CLI_GITEA_ZERO() {
    let h = Harness::new(&[("kv/note", FIXTURE.as_bytes().to_vec())]);
    let out = h.run(&["gitea"]);
    assert_eq!(out.status.code(), Some(2), "zero bundles must exit 2");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(
        stderr.contains("no gitea credential"),
        "zero bundles locked sentence"
    );
    assert_no_value(&stdout, "gitea-zero stdout");
    assert_no_value(&stderr, "gitea-zero stderr");
}

#[test]
fn T_CLI_GITEA_TWO() {
    let h = Harness::new(&[
        ("kv/alpha", gitea_blob(GITEA_URL, "t7a")),
        ("kv/beta", gitea_blob("https://gitea.example", "t7b")),
    ]);
    let out = h.run(&["gitea"]);
    assert_eq!(out.status.code(), Some(2), "two bundles must exit 2");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(stderr.contains("--bundle"), "two bundles mention --bundle");
    let names: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(names.contains(&"kv/alpha"), "names include kv/alpha");
    assert!(names.contains(&"kv/beta"), "names include kv/beta");
    for line in &names {
        assert!(
            *line == "kv/alpha" || *line == "kv/beta",
            "stdout must be names only"
        );
    }
    assert_no_value(&stdout, "gitea-two stdout");
    assert_no_value(&stderr, "gitea-two stderr");
}

#[test]
fn T_CLI_GITEA_ONE() {
    let h = Harness::new(&[("kv/gitea", gitea_blob(GITEA_URL, "t7user"))]);
    let side = h.home.join("child-token");
    let script = format!(
        "printf %s \"$GITEA_TOKEN\" > '{}'; echo \"$GITEA_TOKEN\"",
        side.display()
    );
    let out = h.run(&["gitea", "--", "sh", "-c", &script]);
    assert!(out.status.success(), "gitea one failed");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert_no_value(&stdout, "gitea-one parent stdout");
    assert_no_value(&stderr, "gitea-one parent stderr");
    let mut got = fs::read(&side).expect("child side channel");
    assert!(!got.is_empty(), "child must see GITEA_TOKEN");
    let digest = sha256(&got);
    got.zeroize();
    assert_eq!(
        digest,
        sha256(FIXTURE.as_bytes()),
        "child token hash mismatch"
    );
}

#[test]
fn T_CLI_GITEA_REDACT() {
    let h = Harness::new(&[("kv/gitea", gitea_blob(GITEA_URL, "t7user"))]);
    let out = h.run(&[
        "gitea",
        "--",
        "sh",
        "-c",
        "echo \"$GITEA_TOKEN\"; echo \"$GITEA_TOKEN\" >&2",
    ]);
    assert!(out.status.success(), "gitea redact failed");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert_no_value(&stdout, "gitea-redact stdout");
    assert_no_value(&stderr, "gitea-redact stderr");
}

#[test]
fn T_CLI_WITH_COLLISION() {
    let dir = tmp("coll");
    let flag = dir.join("ran");
    let script = format!("echo ran > '{}'", flag.display());
    let out = isolated_secd(&[
        "run", "--with", "aws=a", "--with", "s3=b", "--", "sh", "-c", &script,
    ])
    .output()
    .expect("run collision");
    assert!(!out.status.success(), "collision must be an error");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(
        stderr.contains("collision") || stdout.contains("collision"),
        "collision error"
    );
    assert!(!flag.exists(), "child must not run on collision");
    assert_no_value(&stdout, "collision stdout");
    assert_no_value(&stderr, "collision stderr");
}

#[test]
fn T_CLI_LOCKED() {
    let out = isolated_secd(&["ls"]).output().expect("locked ls");
    assert!(!out.status.success(), "locked must exit nonzero");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(
        stderr.contains(LOCKED) || stdout.contains(LOCKED),
        "locked sentence"
    );
    assert_no_value(&stdout, "locked stdout");
    assert_no_value(&stderr, "locked stderr");
}

#[test]
fn T_KEYRING_ROUNDTRIP() {
    let home = tmp("keyring");
    let runtime = tmp("keyring-run");
    let mut dek = [0u8; 32];
    fs::File::open("/dev/urandom")
        .expect("urandom")
        .read_exact(&mut dek)
        .expect("dek");
    common::with_secd_env(&home, Some(&runtime), || {
        secd::keyring::store(&dek).expect("store");
        let loaded = secd::keyring::load().expect("load");
        let ok = sha256(loaded.as_bytes()) == sha256(&dek);
        drop(loaded);
        assert!(ok, "loaded DEK hash mismatch");
        secd::logout::run().expect("logout");
        assert!(
            secd::keyring::load().is_none(),
            "logout must delete the DEK"
        );
    });
    dek.zeroize();
}

#[test]
fn T_SESSION_MODE() {
    let home = tmp("session");
    with_secd_home(&home, || {
        secd::login::save_session("t7-session").expect("save_session");
        let path = secd::login::session_path();
        let meta = fs::metadata(&path).expect("session meta");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "login.session must be 0600");
    });
}

#[test]
fn T_RUN_FILE_LEASE() {
    let _g = env_lock();
    let runtime = tmp("lease");
    let prev = std::env::var_os("XDG_RUNTIME_DIR");
    set_var("XDG_RUNTIME_DIR", runtime.to_str().expect("utf8 runtime"));
    let mode_file = runtime.join("mode");
    let script = format!(
        "set -- \"$XDG_RUNTIME_DIR/secd\"/lease-*; [ -f \"$1\" ] || exit 2; stat -c %a \"$1\" > '{}'",
        mode_file.display()
    );
    secd::run::exec(
        &["sh".into(), "-c".into(), script],
        BTreeMap::new(),
        Vec::new(),
    )
    .expect("exec");
    let mode = fs::read_to_string(&mode_file).expect("mode");
    assert_eq!(mode.trim(), "600", "lease file must be 0600");
    let secd_dir = runtime.join("secd");
    if secd_dir.is_dir() {
        for e in fs::read_dir(&secd_dir).expect("lease dir") {
            let e = e.expect("dirent");
            let name = e.file_name();
            assert!(
                !name.to_string_lossy().starts_with("lease-"),
                "lease must be gone after child exit"
            );
        }
    }
    match prev {
        Some(v) => set_var("XDG_RUNTIME_DIR", v.to_str().unwrap_or("")),
        None => remove_var("XDG_RUNTIME_DIR"),
    }
}
