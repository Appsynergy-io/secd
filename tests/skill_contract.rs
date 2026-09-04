#![allow(non_snake_case)]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const SKILLS: [&str; 2] = ["skills/grok/SKILL.md", "skills/claude/SKILL.md"];

const FIXTURES: &[&str] = &[
    "t7-fix-G1tEa-tok-do-not-print-7a3c",
    "t4-fixture-value-do-not-print",
    "fixture-secret-bytes-T_NO_LEAK_DEBUG",
    "fixture-aead-plaintext",
];

static SEQ: AtomicU64 = AtomicU64::new(1);

fn skill_bytes(path: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn skill_text(path: &str) -> String {
    String::from_utf8(skill_bytes(path)).unwrap_or_else(|_| panic!("{path} is not utf-8"))
}

#[test]
fn T_SKILL_IDENTICAL() {
    let grok = skill_bytes(SKILLS[0]);
    let claude = skill_bytes(SKILLS[1]);
    assert_eq!(grok, claude, "grok and claude SKILL.md bytes differ");
}

#[test]
fn T_SKILL_LEN() {
    for path in SKILLS {
        let n = skill_text(path).lines().count();
        assert!(n <= 80, "{path} has {n} lines");
    }
}

#[test]
fn T_SKILL_TABLE() {
    let names: Vec<String> = secd::cli()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    assert!(!names.is_empty(), "clap has no subcommands");
    for path in SKILLS {
        let text = skill_text(path);
        for name in &names {
            let needle = format!("`secd {name}`");
            assert!(text.contains(&needle), "{path} missing {needle}");
        }
        assert!(text.contains("`secd`"), "{path} missing `secd`");
    }
}

#[test]
fn T_SKILL_NO_GET() {
    for path in SKILLS {
        assert!(
            !skill_text(path).contains("`secd get`"),
            "{path} contains `secd get`"
        );
    }
}

#[test]
fn T_SKILL_NO_FORCE() {
    for path in SKILLS {
        let text = skill_text(path);
        assert!(!text.contains("--force"), "{path} contains --force");
        assert!(
            !text.contains("import-legacy"),
            "{path} contains import-legacy"
        );
    }
}

#[test]
fn T_SKILL_NO_FIXTURE() {
    let out = Command::new("git")
        .args(["ls-files", "--", "*.md"])
        .output()
        .expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed");
    let tracked = String::from_utf8(out.stdout).expect("utf8");
    for file in tracked.lines().filter(|l| !l.is_empty()) {
        let text = fs::read_to_string(file).unwrap_or_else(|e| panic!("read {file}: {e}"));
        for fix in FIXTURES {
            assert!(!text.contains(fix), "{file} contains a fixture value");
        }
    }
}

#[test]
fn T_SKILL_INSTALL() {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let home = std::env::temp_dir().join(format!("secd-t8-home-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(&home).expect("home");
    let _env = common_env();
    let _reset = HomeReset::swap(&home);
    secd::skills_install::run().expect("skills_install::run");
    let dests = secd::skills_install::dest_files();
    assert_eq!(dests[0], home.join(".claude/skills/secd/SKILL.md"));
    assert_eq!(dests[1], home.join(".grok/skills/secd/SKILL.md"));
    for path in &dests {
        let got = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_eq!(got, secd::skills_install::SKILL, "{}", path.display());
    }
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn T_SKILL_RETIRES_SDXD() {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let home = std::env::temp_dir().join(format!("secd-t8-old-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&home);
    let stale = [
        home.join(".claude/skills/sdxd"),
        home.join(".grok/skills/sdxd"),
    ];
    for dir in &stale {
        fs::create_dir_all(dir).expect("stale skill dir");
        fs::write(dir.join("SKILL.md"), b"old skill").expect("stale SKILL.md");
    }
    let _env = common_env();
    let _reset = HomeReset::swap(&home);
    secd::skills_install::run().expect("skills_install::run");
    assert_eq!(secd::skills_install::stale_dirs(), stale);
    for dir in &stale {
        assert!(!dir.exists(), "{} survived the install", dir.display());
    }
    for path in secd::skills_install::dest_files() {
        assert!(path.exists(), "{} was not written", path.display());
    }
    // Idempotent: the second run has nothing to remove and must not complain.
    secd::skills_install::run().expect("second skills_install::run");
    let _ = fs::remove_dir_all(&home);
}

struct HomeReset {
    prev: Option<std::ffi::OsString>,
}

impl HomeReset {
    fn swap(home: &Path) -> Self {
        let prev = std::env::var_os("HOME");
        // SAFETY: this file's tests hold ENV for the swap; Drop restores HOME.
        unsafe { std::env::set_var("HOME", home) };
        Self { prev }
    }
}

impl Drop for HomeReset {
    fn drop(&mut self) {
        match self.prev.take() {
            // SAFETY: paired with swap; ENV still held by the test.
            Some(v) => unsafe { std::env::set_var("HOME", v) },
            None => unsafe { std::env::remove_var("HOME") },
        }
    }
}

fn common_env() -> std::sync::MutexGuard<'static, ()> {
    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ENV.lock().unwrap_or_else(|e| e.into_inner())
}
