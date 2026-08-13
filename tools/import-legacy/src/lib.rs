//! Black-box import from the old CLI (`sdxd`) into a live secd session.
//! Never writes a secret value to this process's stdout, stderr, or logs.

use std::fs;
use std::io::{self, IsTerminal};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;

use secd::login::Unlocked;
use secd::policy::Entry;
use secd_core::{check_name, infer, providers, Secret};
use serde_json::{json, Map, Value};
use zeroize::{Zeroize, Zeroizing};

const MSG_NOTTY: &str = "not a TTY";
const MSG_NO_SDXD: &str = "old secrets CLI not on PATH";
const MSG_SDXD_LOCKED: &str = "old CLI locked — run sdxd";
const MSG_SECD_LOCKED: &str = "secd locked — run secd";

pub type WipeHook = fn(&[u8]);

/// After each name, invoked with the get-buffer (already zeroed). Tests replace this.
pub static AFTER_EACH_WIPE: Mutex<Option<WipeHook>> = Mutex::new(None);

/// Capture `sdxd get` stdout. One trailing newline stripped when the bytes are UTF-8.
/// `Drop` of the returned `Zeroizing` zeros the buffer.
pub fn take_get_bytes(raw: Vec<u8>) -> Zeroizing<Vec<u8>> {
    let mut v = raw;
    if std::str::from_utf8(&v).is_ok() && v.last() == Some(&b'\n') {
        v.pop();
        if v.last() == Some(&b'\r') {
            v.pop();
        }
    }
    Zeroizing::new(v)
}

/// Zero `buf` in place. Then fire `AFTER_EACH_WIPE` if set.
pub fn wipe_get_buffer(buf: &mut Zeroizing<Vec<u8>>) {
    buf.zeroize();
    let hook = AFTER_EACH_WIPE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .copied();
    if let Some(f) = hook {
        f(buf);
    }
}

pub fn buffer_is_zeroed(buf: &[u8]) -> bool {
    buf.iter().all(|&b| b == 0)
}

pub fn stdin_stdout_are_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

pub fn find_sdxd() -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let p = dir.join("sdxd");
        if is_exe(&p) {
            return Some(p);
        }
    }
    None
}

fn is_exe(p: &Path) -> bool {
    match fs::metadata(p) {
        Ok(m) => m.is_file() && m.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// TTY gate then import. Exit 2 if not a TTY, sdxd missing, sdxd locked, or secd locked.
pub fn run() -> i32 {
    if !stdin_stdout_are_tty() {
        eprintln!("{MSG_NOTTY}");
        return 2;
    }
    match import() {
        Ok(()) => 0,
        Err(e) => e.exit(),
    }
}

/// Import without the TTY gate (library / tests).
pub fn import() -> Result<(), ImportError> {
    if find_sdxd().is_none() {
        return Err(ImportError::NoSdxd);
    }
    let unlocked = require_secd()?;
    let mut entries = load_vault(&unlocked)?;
    let names = sdxd_ls()?;
    let mut imported = Vec::new();
    for name in names {
        if check_name(&name).is_err() {
            continue;
        }
        let meta = sdxd_info(&name);
        let mut buf = sdxd_get(&name)?;
        let keys = field_keys(&name, &buf);
        let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let service = meta.get("service").and_then(Value::as_str);
        if let Some(p) = resolve_provider(&key_refs, service) {
            let mut m = meta.as_object().cloned().unwrap_or_default();
            m.insert("provider".into(), json!(p));
            apply_one(&mut entries, name.clone(), &buf, Value::Object(m));
        } else {
            apply_one(&mut entries, name.clone(), &buf, meta);
        }
        wipe_get_buffer(&mut buf);
        imported.push(name);
    }
    save_vault(&unlocked, &entries)?;
    for name in &imported {
        println!("{name}");
    }
    println!("imported {}", imported.len());
    Ok(())
}

fn apply_one(entries: &mut Vec<Entry>, name: String, plain: &[u8], meta: Value) {
    let value = Secret::new(plain.to_vec());
    if let Some(e) = entries.iter_mut().find(|e| e.name == name) {
        e.value = value;
        e.meta = meta;
    } else {
        entries.push(Entry { name, value, meta });
    }
}

fn field_keys(name: &str, plain: &[u8]) -> Vec<String> {
    if let Ok(text) = std::str::from_utf8(plain) {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) {
            return map.keys().cloned().collect();
        }
    }
    let leaf = name.rsplit('/').next().unwrap_or(name);
    vec![leaf.replace('-', "_")]
}

fn resolve_provider(keys: &[&str], service: Option<&str>) -> Option<String> {
    if let Some(p) = infer(keys) {
        return Some(p.to_string());
    }
    let s = service?;
    if providers().iter().any(|p| p.name == s) {
        Some(s.to_string())
    } else {
        None
    }
}

fn require_secd() -> Result<Unlocked, ImportError> {
    let token = secd::login::load_session().ok_or(ImportError::SecdLocked)?;
    let dek = secd::keyring::load().ok_or(ImportError::SecdLocked)?;
    Ok(Unlocked { token, dek })
}

fn load_vault(unlocked: &Unlocked) -> Result<Vec<Entry>, ImportError> {
    match secd::policy::load_entries(&unlocked.token, &unlocked.dek) {
        Ok(e) => Ok(e),
        Err(e) => {
            let s = e.to_string();
            if s.contains("locked") || s.contains("401") {
                Err(ImportError::SecdLocked)
            } else {
                Err(ImportError::Other(e))
            }
        }
    }
}

fn save_vault(unlocked: &Unlocked, entries: &[Entry]) -> Result<(), ImportError> {
    match secd::policy::save_entries(&unlocked.token, &unlocked.dek, entries) {
        Ok(()) => Ok(()),
        Err(e) => {
            let s = e.to_string();
            if s.contains("locked") || s.contains("401") {
                Err(ImportError::SecdLocked)
            } else {
                Err(ImportError::Other(e))
            }
        }
    }
}

fn sdxd_ls() -> Result<Vec<String>, ImportError> {
    let out = sdxd(&["ls"])?;
    if child_locked(&out) {
        zeroize_output(out);
        return Err(ImportError::SdxdLocked);
    }
    if !out.status.success() {
        zeroize_output(out);
        return Err(ImportError::SdxdLocked);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let names: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    Ok(names)
}

fn sdxd_info(name: &str) -> Value {
    let Ok(out) = sdxd(&["info", name]) else {
        return json!({});
    };
    if !out.status.success() {
        zeroize_output(out);
        return json!({});
    }
    parse_info(&String::from_utf8_lossy(&out.stdout))
}

fn parse_info(stdout: &str) -> Value {
    let mut meta = Map::new();
    for line in stdout.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        if k == "service" || k == "note" {
            meta.insert(k.to_string(), json!(v));
        }
    }
    Value::Object(meta)
}

fn sdxd_get(name: &str) -> Result<Zeroizing<Vec<u8>>, ImportError> {
    let out = sdxd(&["get", "--force", name])?;
    if child_locked(&out) || !out.status.success() {
        zeroize_output(out);
        return Err(ImportError::SdxdLocked);
    }
    let mut stderr = Zeroizing::new(out.stderr);
    stderr.zeroize();
    Ok(take_get_bytes(out.stdout))
}

fn sdxd(args: &[&str]) -> Result<Output, ImportError> {
    match Command::new("sdxd")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(out) => Ok(out),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(ImportError::NoSdxd),
        Err(e) => Err(ImportError::Other(e.into())),
    }
}

fn child_locked(out: &Output) -> bool {
    if out.status.success() {
        return false;
    }
    let mut blob = Vec::with_capacity(out.stdout.len() + out.stderr.len());
    blob.extend_from_slice(&out.stderr);
    blob.extend_from_slice(&out.stdout);
    let text = String::from_utf8_lossy(&blob).to_ascii_lowercase();
    blob.zeroize();
    let text = text
        .replace("swap-locked", "")
        .replace("database is locked", "")
        .replace("database table is locked", "")
        .replace("database schema is locked", "");
    text.contains("locked")
}

fn zeroize_output(mut out: Output) {
    out.stdout.zeroize();
    out.stderr.zeroize();
}

pub enum ImportError {
    NoSdxd,
    SdxdLocked,
    SecdLocked,
    Other(anyhow::Error),
}

impl ImportError {
    fn exit(self) -> i32 {
        match &self {
            Self::NoSdxd => {
                eprintln!("{MSG_NO_SDXD}");
                2
            }
            Self::SdxdLocked => {
                eprintln!("{MSG_SDXD_LOCKED}");
                2
            }
            Self::SecdLocked => {
                eprintln!("{MSG_SECD_LOCKED}");
                2
            }
            Self::Other(e) => {
                eprintln!("{e}");
                1
            }
        }
    }
}
