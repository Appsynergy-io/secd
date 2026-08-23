//! Black-box import from the old CLI (`sdxd`) into a live secd session.
//! Never writes a secret value to this process's stdout, stderr, or logs.

use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;

use secd::login::Unlocked;
use secd::policy::{Entry, Row, VaultLoad};
use secd_core::{check_name, infer, providers, Secret};
use serde_json::{json, Map, Value};
use zeroize::{Zeroize, Zeroizing};

const MSG_NOTTY: &str = "not a TTY";
const MSG_NO_SDXD: &str = "old secrets CLI not on PATH";
const MSG_SDXD_LOCKED: &str = "old CLI locked — run sdxd";
const MSG_SECD_LOCKED: &str = "secd locked — run secd";
const USAGE: &str = "usage: secd-import-legacy [--prefix P] [--dry-run] [--verify] [--overwrite] \
     [--snapshot PATH]";
const MSG_NO_SNAPSHOT: &str = "--snapshot PATH is required for a run that writes: \
     PUT /api/v1/vault replaces the whole vault";

pub type WipeHook = fn(&[u8]);

/// After each name, invoked with the get-buffer (already zeroed). Tests replace this.
pub static AFTER_EACH_WIPE: Mutex<Option<WipeHook>> = Mutex::new(None);

/// One run of the importer.
#[derive(Default)]
pub struct Options {
    /// Only sdxd names that start with this are considered.
    pub prefix: Option<String>,
    /// Print the plan, write nothing.
    pub dry_run: bool,
    /// Compare each sdxd value with the secd value, write nothing.
    pub verify: bool,
    /// Allow a name that is already in the vault to be replaced.
    pub overwrite: bool,
    /// Where to put the whole-vault pre-image. Required for a run that writes.
    pub snapshot: Option<PathBuf>,
}

impl Options {
    /// True for a run that reaches `PUT /api/v1/vault`.
    fn writes(&self) -> bool {
        !self.dry_run && !self.verify
    }
}

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
    let opts = match parse_args(std::env::args_os().skip(1)) {
        Ok(o) => o,
        Err(e) => return e.exit(),
    };
    match import(&opts) {
        Ok(()) => 0,
        Err(e) => e.exit(),
    }
}

/// Parse the flags after argv[0].
pub fn parse_args<I: IntoIterator<Item = OsString>>(argv: I) -> Result<Options, ImportError> {
    let mut opts = Options::default();
    let mut it = argv.into_iter();
    while let Some(raw) = it.next() {
        let Some(arg) = raw.to_str() else {
            return Err(usage());
        };
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f, Some(v.to_string())),
            None => (arg, None),
        };
        match flag {
            "--dry-run" if inline.is_none() => opts.dry_run = true,
            "--verify" if inline.is_none() => opts.verify = true,
            "--overwrite" if inline.is_none() => opts.overwrite = true,
            "--prefix" => opts.prefix = Some(take_value(inline, &mut it)?),
            "--snapshot" => opts.snapshot = Some(PathBuf::from(take_value(inline, &mut it)?)),
            _ => return Err(usage()),
        }
    }
    Ok(opts)
}

fn usage() -> ImportError {
    ImportError::Refused(USAGE.to_string())
}

fn take_value(
    inline: Option<String>,
    it: &mut impl Iterator<Item = OsString>,
) -> Result<String, ImportError> {
    if let Some(v) = inline {
        return Ok(v);
    }
    match it.next() {
        Some(v) => {
            let s = v.into_string().map_err(|_| usage())?;
            if s.starts_with("--") {
                return Err(usage());
            }
            Ok(s)
        }
        None => Err(usage()),
    }
}

/// Import without the TTY gate (library / tests).
pub fn import(opts: &Options) -> Result<(), ImportError> {
    if find_sdxd().is_none() {
        return Err(ImportError::NoSdxd);
    }
    let snapshot = preflight(opts)?;
    let unlocked = require_secd()?;
    let loaded = load_vault(&unlocked)?;
    // Fail closed on a write. The load drops every entry this DEK cannot open
    // and the save is a whole-vault replace, so saving now would delete them.
    // --dry-run and --verify write nothing; they still run.
    match loaded.drop_refusal() {
        Some(refusal) if opts.writes() => return Err(ImportError::Refused(refusal)),
        Some(_) => eprintln!(
            "vault: {} of {} entries did not decode",
            loaded.dropped(),
            loaded.raw
        ),
        None => {}
    }
    let VaultLoad {
        mut entries,
        body,
        before,
        ..
    } = loaded;
    let names = wanted_names(opts)?;
    if opts.verify {
        return verify(entries, &names);
    }
    let collisions: Vec<String> = names
        .iter()
        .filter(|n| entries.iter().any(|e| &e.name == *n))
        .cloned()
        .collect();
    if opts.dry_run {
        print_plan(&names, &collisions);
        return Ok(());
    }
    if !collisions.is_empty() && !opts.overwrite {
        return Err(ImportError::Refused(format!(
            "{} name(s) already in the vault; re-run with --overwrite: {}",
            collisions.len(),
            collisions.join(", ")
        )));
    }
    // The pre-image goes down before the first mutation: PUT replaces the whole
    // vault, so this one file restores it in one call.
    let path = snapshot.expect("invariant: preflight requires --snapshot for a writing run");
    write_snapshot(path, &body)?;
    let mut imported = Vec::new();
    for name in names {
        let meta = sdxd_info(&name);
        let mut buf = sdxd_get(&name)?;
        apply_one(
            &mut entries,
            name.clone(),
            &buf,
            entry_meta(&name, &buf, meta),
        );
        wipe_get_buffer(&mut buf);
        imported.push(name);
    }
    save_vault(&unlocked, &entries, &before)?;
    for name in &imported {
        println!("{name}");
    }
    println!("imported {}", imported.len());
    Ok(())
}

/// Refuse a writing run that has no fresh snapshot path, before anything moves.
fn preflight(opts: &Options) -> Result<Option<&Path>, ImportError> {
    if !opts.writes() {
        return Ok(None);
    }
    let Some(path) = opts.snapshot.as_deref() else {
        return Err(ImportError::Refused(MSG_NO_SNAPSHOT.to_string()));
    };
    if path.exists() {
        return Err(ImportError::Refused(format!(
            "snapshot {} exists; refusing to overwrite a pre-image",
            path.display()
        )));
    }
    Ok(Some(path))
}

/// The whole-vault pre-image. Ciphertext, never plaintext.
///
/// On disk before it is returned, file and directory entry both: the vault is
/// about to be replaced, and a pre-image still in the page cache is no
/// pre-image at all.
fn write_snapshot(path: &Path, body: &str) -> Result<(), ImportError> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| ImportError::Refused(format!("snapshot {}: {e}", path.display())))?;
    f.write_all(body.as_bytes())
        .and_then(|()| f.sync_all())
        .map_err(|e| ImportError::Other(e.into()))?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| ImportError::Other(e.into()))
}

/// The sdxd names this run covers, in `sdxd ls` order, each one once.
fn wanted_names(opts: &Options) -> Result<Vec<String>, ImportError> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for name in sdxd_ls()? {
        if check_name(&name).is_err() {
            continue;
        }
        if let Some(p) = &opts.prefix {
            if !name.starts_with(p.as_str()) {
                continue;
            }
        }
        if seen.insert(name.clone()) {
            out.push(name);
        }
    }
    Ok(out)
}

fn print_plan(names: &[String], collisions: &[String]) {
    for name in names {
        if collisions.iter().any(|c| c == name) {
            println!("{name} overwrite");
        } else {
            println!("{name} new");
        }
    }
    println!(
        "dry-run: {} new, {} overwrite",
        names.len() - collisions.len(),
        collisions.len()
    );
}

/// Compare each sdxd value with the secd value already in the vault. Both are
/// in this process, so this needs no digest and no key.
fn verify(mut entries: Vec<Entry>, names: &[String]) -> Result<(), ImportError> {
    let mut bad = 0usize;
    for name in names {
        let mut buf = sdxd_get(name)?;
        let same = match entries.iter_mut().find(|e| &e.name == name) {
            Some(e) => {
                let same = ct_eq(e.value.as_bytes(), &buf);
                e.value.zeroize();
                same
            }
            None => false,
        };
        wipe_get_buffer(&mut buf);
        if same {
            println!("{name} ok");
        } else {
            println!("{name} MISMATCH");
            bad += 1;
        }
    }
    if bad > 0 {
        return Err(ImportError::Mismatch(format!("verify: {bad} mismatch")));
    }
    Ok(())
}

/// Constant-time byte equality. The lengths are not secret from this process.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
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

/// `sdxd info` plus what the value itself says: the provider, and the field
/// keys the console needs to draw a row per field instead of one masked row.
fn entry_meta(name: &str, plain: &[u8], info: Value) -> Value {
    let json_keys = json_field_keys(plain);
    let keys = json_keys.clone().unwrap_or_else(|| vec![leaf_key(name)]);
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let service = info.get("service").and_then(Value::as_str);
    let provider = resolve_provider(&key_refs, service);
    let mut m = info.as_object().cloned().unwrap_or_default();
    if let Some(p) = provider {
        m.insert("provider".into(), json!(p));
    }
    if let Some(f) = json_keys {
        m.insert("fields".into(), json!(f));
    }
    Value::Object(m)
}

/// The keys of the value, when the value is a JSON object. `None` otherwise:
/// a bare value has no fields, and a guessed one would be a lie in the console.
fn json_field_keys(plain: &[u8]) -> Option<Vec<String>> {
    let text = std::str::from_utf8(plain).ok()?;
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Some(map.keys().cloned().collect()),
        _ => None,
    }
}

fn leaf_key(name: &str) -> String {
    let leaf = name.rsplit('/').next().unwrap_or(name);
    leaf.replace('-', "_")
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

fn load_vault(unlocked: &Unlocked) -> Result<VaultLoad, ImportError> {
    match secd::policy::load_vault(&unlocked.token, &unlocked.dek) {
        Ok(v) => Ok(v),
        Err(e) => Err(map_vault_err(e)),
    }
}

fn save_vault(
    unlocked: &Unlocked,
    entries: &[Entry],
    before: &BTreeMap<String, String>,
) -> Result<(), ImportError> {
    let rows: Vec<Row<'_>> = entries.iter().map(Entry::row).collect();
    match secd::policy::save_entries_read_back(&unlocked.token, &unlocked.dek, &rows, before) {
        Ok(_) => Ok(()),
        Err(e) => Err(map_vault_err(e)),
    }
}

fn map_vault_err(e: anyhow::Error) -> ImportError {
    let s = e.to_string();
    // A landed PUT that we then failed to read back is not "locked": restoring
    // the snapshot would undo a write that did happen. Match that, and the
    // clobber sentence (an entry may be named `locked`), before 401/locked.
    if s.contains("changed under the write") || s.contains("vault written") {
        ImportError::Mismatch(s)
    } else if s.contains("locked") || s.contains("401") {
        ImportError::SecdLocked
    } else {
        ImportError::Other(e)
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
    /// Refused before the vault was touched: usage, or a precondition that
    /// would have cost entries.
    Refused(String),
    /// A value did not match, or the vault changed under the write.
    Mismatch(String),
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
            Self::Refused(m) => {
                eprintln!("{m}");
                2
            }
            Self::Mismatch(m) => {
                eprintln!("{m}");
                1
            }
            Self::Other(e) => {
                eprintln!("{e}");
                1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_next_token_must_not_be_a_flag() {
        match parse_args([OsString::from("--snapshot"), OsString::from("--dry-run")]) {
            Ok(opts) => panic!(
                "must refuse a flag as --snapshot value (dry_run={})",
                opts.dry_run
            ),
            Err(ImportError::Refused(_)) => {}
            Err(_) => panic!("must be usage"),
        }
        match parse_args([OsString::from("--snapshot=--path")]) {
            Ok(opts) => {
                assert_eq!(opts.snapshot.as_deref(), Some(Path::new("--path")));
                assert!(!opts.dry_run);
            }
            Err(_) => panic!("--snapshot=--path must be accepted"),
        }
    }
}
