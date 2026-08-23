#![allow(non_snake_case)]

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use secd_import_legacy::{buffer_is_zeroed, ImportError, Options, AFTER_EACH_WIPE};
use serde_json::Value;

const FIXTURE: &str = "t9-fix-IMP-val-do-not-print-c4e1";
const FIXTURE_ALT: &str = "t9-fix-IMP-alt-do-not-print-9b2d";
const NAME_A: &str = "kv/alpha";
const NAME_B: &str = "kv/beta";
const NAME_JSON: &str = "js/one";
const NO_SDXD: &str = "old secrets CLI not on PATH";
const EMPTY_VAULT: &str = r#"{"entries":[]}"#;
const ROOT_PEM: &[u8] = include_bytes!("../../../keys/appsynergy-root.pem");

static SEQ: AtomicU64 = AtomicU64::new(1);
static ASSETS: OnceLock<Assets> = OnceLock::new();
static ZERO_HITS: Mutex<Vec<bool>> = Mutex::new(Vec::new());

struct Assets {
    bin: PathBuf,
    redir: PathBuf,
    ca: PathBuf,
    cert: PathBuf,
    key: PathBuf,
    sdxd_dir: PathBuf,
    vault_py: PathBuf,
    ca_padded: Vec<u8>,
}

struct Harness {
    dir: PathBuf,
    home: PathBuf,
    runtime: PathBuf,
    put_log: PathBuf,
    state: PathBuf,
    clobber: PathBuf,
    server: Child,
    port: u16,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn work_root() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_secd-import-legacy"))
        .parent()
        .and_then(Path::parent)
        .expect("target dir")
        .join("t9-import")
}

fn unique(tag: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let root = work_root();
    fs::create_dir_all(&root).expect("t9 root");
    root.join(format!("{tag}-{}-{n}", std::process::id()))
}

fn pty_script() -> &'static Path {
    static PTY: OnceLock<PathBuf> = OnceLock::new();
    PTY.get_or_init(|| {
        let p = work_root().join("pty_run.py");
        fs::create_dir_all(work_root()).expect("t9 root");
        fs::write(&p, PTY_PY).expect("pty_run.py");
        p
    })
}

fn restore_script() -> &'static Path {
    static RESTORE: OnceLock<PathBuf> = OnceLock::new();
    RESTORE.get_or_init(|| {
        let p = work_root().join("put_body.py");
        fs::create_dir_all(work_root()).expect("t9 root");
        fs::write(&p, RESTORE_PY).expect("put_body.py");
        p
    })
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn leaked(hay: &str) -> bool {
    hay.contains(FIXTURE) || hay.contains(FIXTURE_ALT)
}

fn chmod_exec(path: &Path) {
    let mut p = fs::metadata(path).expect("meta").permissions();
    p.set_mode(0o755);
    fs::set_permissions(path, p).expect("chmod");
}

fn dek_desc(home: &Path) -> String {
    let raw = home.to_string_lossy();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in raw.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("secd-dek-{h:016x}")
}

fn write_0600(path: &Path, bytes: &[u8]) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).expect("mkdir");
        if let Ok(meta) = fs::metadata(dir) {
            let mut perms = meta.permissions();
            perms.set_mode(0o700);
            let _ = fs::set_permissions(dir, perms);
        }
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .expect("open 0600");
    f.write_all(bytes).expect("write");
    f.flush().expect("flush");
    let mut perms = f.metadata().expect("meta").permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms).expect("mode");
}

fn reflink_copy(src: &Path, dst: &Path) {
    let st = Command::new("/usr/bin/cp")
        .args(["--reflink=auto", "--"])
        .arg(src)
        .arg(dst)
        .status()
        .expect("cp");
    assert!(st.success(), "cp --reflink failed");
}

fn patch_embedded_ca(path: &Path, repl: &[u8]) {
    assert_eq!(repl.len(), ROOT_PEM.len());
    let data = fs::read(path).expect("read importer");
    let n = ROOT_PEM.len();
    let mut offs = Vec::new();
    let mut i = 0;
    while i + n <= data.len() {
        if data[i] == ROOT_PEM[0] && &data[i..i + n] == ROOT_PEM {
            offs.push(i);
            i += n;
        } else {
            i += 1;
        }
    }
    assert!(!offs.is_empty(), "embedded root PEM not found");
    drop(data);
    let mut f = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open importer");
    for off in offs {
        f.seek(SeekFrom::Start(off as u64)).expect("seek");
        f.write_all(repl).expect("patch pem");
    }
    f.flush().expect("flush patch");
}

fn wait_port(addr: SocketAddr) {
    for _ in 0..100 {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("tls server did not start on {addr}");
}

fn assets() -> &'static Assets {
    ASSETS.get_or_init(build_assets)
}

fn build_assets() -> Assets {
    let dir = unique("assets");
    fs::create_dir_all(&dir).expect("assets dir");

    let ca = dir.join("ca.pem");
    let ca_key = dir.join("ca.key");
    let cert = dir.join("tls.crt");
    let key = dir.join("tls.key");
    let csr = dir.join("leaf.csr");
    let ext = dir.join("leaf.ext");
    let out = Command::new("/usr/bin/openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-sha256", "-days", "1", "-nodes", "-keyout",
        ])
        .arg(&ca_key)
        .arg("-out")
        .arg(&ca)
        .args([
            "-subj",
            "/CN=T9 Test CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
        ])
        .output()
        .expect("openssl ca");
    assert!(out.status.success(), "openssl ca failed");
    let out = Command::new("/usr/bin/openssl")
        .args(["req", "-newkey", "rsa:2048", "-nodes", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&csr)
        .args(["-subj", "/CN=secd.imabee.com"])
        .output()
        .expect("openssl csr");
    assert!(out.status.success(), "openssl csr failed");
    fs::write(
        &ext,
        "subjectAltName=DNS:secd.imabee.com\n\
         basicConstraints=CA:FALSE\n\
         keyUsage=digitalSignature,keyEncipherment\n\
         extendedKeyUsage=serverAuth\n",
    )
    .expect("ext");
    let out = Command::new("/usr/bin/openssl")
        .args(["x509", "-req", "-in"])
        .arg(&csr)
        .arg("-CA")
        .arg(&ca)
        .arg("-CAkey")
        .arg(&ca_key)
        .args(["-CAcreateserial", "-out"])
        .arg(&cert)
        .args(["-days", "1", "-extfile"])
        .arg(&ext)
        .output()
        .expect("openssl leaf");
    assert!(out.status.success(), "openssl leaf failed");

    let pem = fs::read(&ca).expect("read ca");
    assert!(
        pem.len() <= ROOT_PEM.len(),
        "test CA longer than embedded root PEM"
    );
    let mut padded = pem;
    padded.resize(ROOT_PEM.len(), b'\n');
    let src = Path::new(env!("CARGO_BIN_EXE_secd-import-legacy"));
    let bin = dir.join("secd-import-legacy");
    reflink_copy(src, &bin);
    patch_embedded_ca(&bin, &padded);
    chmod_exec(&bin);

    let redir_c = dir.join("redir.c");
    fs::write(&redir_c, REDIR_C).expect("redir.c");
    let redir = dir.join("redir.so");
    let st = Command::new("/usr/bin/cc")
        .args(["-shared", "-fPIC", "-o"])
        .arg(&redir)
        .arg(&redir_c)
        .arg("-ldl")
        .status()
        .expect("cc redir");
    assert!(st.success(), "cc redir.so failed");

    let sdxd_dir = dir.join("sdxd-bin");
    fs::create_dir_all(&sdxd_dir).expect("sdxd dir");
    let sdxd = sdxd_dir.join("sdxd");
    fs::write(&sdxd, SDXD_SH).expect("sdxd");
    chmod_exec(&sdxd);

    let vault_py = dir.join("vault.py");
    fs::write(&vault_py, VAULT_PY).expect("vault.py");

    Assets {
        bin,
        redir,
        ca,
        cert,
        key,
        sdxd_dir,
        vault_py,
        ca_padded: padded,
    }
}

impl Harness {
    fn new() -> Self {
        Self::seeded(EMPTY_VAULT)
    }

    /// A vault that already holds `state`, the body `GET /api/v1/vault` returns.
    fn seeded(state: &str) -> Self {
        let a = assets();
        let dir = unique("h");
        let home = dir.join("home");
        let runtime = dir.join("run");
        let put_log = dir.join("put.log");
        let state_path = dir.join("vault.json");
        let clobber = dir.join("clobber.json");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(runtime.join("secd")).expect("runtime");
        write_0600(&home.join("login.session"), b"t9-token\n");
        write_0600(&runtime.join("secd").join(dek_desc(&home)), &[0x44; 32]);
        write_0600(&put_log, b"");
        write_0600(&state_path, state.as_bytes());

        let mut server = Command::new("/usr/bin/python3")
            .arg(&a.vault_py)
            .arg(&a.cert)
            .arg(&a.key)
            .arg(&put_log)
            .arg(&state_path)
            .arg(&clobber)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("vault server");
        let stdout = server.stdout.take().expect("vault stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("vault port");
        let port: u16 = line.trim().parse().unwrap_or_else(|_| {
            let err = server.stderr.take().map(|e| {
                let mut s = String::new();
                let _ = BufReader::new(e).read_line(&mut s);
                s
            });
            panic!("vault port: {line:?} stderr={err:?}");
        });
        wait_port(SocketAddr::from(([127, 0, 0, 1], port)));
        Self {
            dir,
            home,
            runtime,
            put_log,
            state: state_path,
            clobber,
            server,
            port,
        }
    }

    fn child_env(&self, cmd: &mut Command) {
        let a = assets();
        cmd.env("SECD_HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("LD_PRELOAD", &a.redir)
            .env("SECD_TEST_REDIR", format!("127.0.0.1:{}", self.port))
            .env("PATH", &a.sdxd_dir)
            .env_remove("GITEA_TOKEN");
    }

    fn run_notty(&self) -> Output {
        let mut cmd = Command::new(&assets().bin);
        self.child_env(&mut cmd);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("import notty")
    }

    fn run_tty(&self, args: &[&str], envs: &[(&str, &str)]) -> Output {
        pty_run(&assets().bin, args, |cmd| {
            self.child_env(cmd);
            for (k, v) in envs {
                cmd.env(k, v);
            }
        })
    }

    /// A writing run, with a snapshot path this test has not used before.
    fn import(&self, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let snap = unique("snap");
        let mut argv = vec!["--snapshot", snap.to_str().expect("utf8 snapshot path")];
        argv.extend_from_slice(args);
        self.run_tty(&argv, envs)
    }

    fn put_count(&self) -> usize {
        let raw = fs::read_to_string(&self.put_log).unwrap_or_default();
        raw.lines().filter(|l| l.starts_with("PUT")).count()
    }

    /// What the server would return from `GET /api/v1/vault` right now.
    fn stored(&self) -> Vec<Value> {
        let raw = fs::read_to_string(&self.state).unwrap_or_default();
        serde_json::from_str::<Value>(&raw)
            .ok()
            .and_then(|v| v.get("entries").and_then(Value::as_array).cloned())
            .unwrap_or_default()
    }

    fn stored_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .stored()
            .iter()
            .filter_map(|e| e.get("name").and_then(Value::as_str))
            .map(str::to_string)
            .collect();
        names.sort();
        names
    }

    /// Install a whole-vault save that lands right after the importer's PUT.
    fn arm_clobber(&self, doc: &str) {
        write_0600(&self.clobber, doc.as_bytes());
    }

    /// `PUT /api/v1/vault` with a body of the caller's choosing, as a hand
    /// restore of a pre-image would. The status the server answered with.
    fn put_body(&self, body: &str) -> u16 {
        let mut child = Command::new("/usr/bin/python3")
            .arg(restore_script())
            .arg(&assets().ca)
            .arg(self.port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("put client");
        {
            let mut sin = child.stdin.take().expect("put stdin");
            sin.write_all(body.as_bytes()).expect("put body");
        }
        let out = child.wait_with_output().expect("put client");
        assert!(out.status.success(), "put client: {}", utf8(&out.stderr));
        utf8(&out.stdout).trim().parse().expect("status code")
    }
}

/// The entries of a whole-vault pre-image, by name, sorted.
fn snapshot_names(path: &Path) -> Vec<String> {
    let raw = fs::read_to_string(path).expect("snapshot");
    let doc: Value = serde_json::from_str(&raw).expect("snapshot json");
    let mut names: Vec<String> = doc
        .get("entries")
        .and_then(Value::as_array)
        .expect("snapshot entries")
        .iter()
        .filter_map(|e| e.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    names.sort();
    names
}

/// The same body as `GET /api/v1/vault` hands it out: one `version` per entry.
fn with_version(body: &str) -> String {
    let mut doc: Value = serde_json::from_str(body).expect("body json");
    for e in doc
        .get_mut("entries")
        .and_then(Value::as_array_mut)
        .expect("entries")
    {
        e["version"] = Value::from(1);
    }
    doc.to_string()
}

fn pty_run(bin: &Path, args: &[&str], env: impl FnOnce(&mut Command)) -> Output {
    let err_path = unique("err");
    let _ = fs::remove_file(&err_path);
    let mut cmd = Command::new("/usr/bin/python3");
    cmd.arg(pty_script())
        .arg(&err_path)
        .arg(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    env(&mut cmd);
    let out = cmd.output().expect("pty_run");
    let stderr = fs::read(&err_path).unwrap_or_default();
    let _ = fs::remove_file(&err_path);
    Output {
        status: out.status,
        stdout: out.stdout,
        stderr,
    }
}

fn wipe_hook(buf: &[u8]) {
    let z = buffer_is_zeroed(buf);
    ZERO_HITS.lock().unwrap_or_else(|e| e.into_inner()).push(z);
    if let Ok(path) = std::env::var("T9_ZEROIZE_OUT") {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(if z { b"1\n" } else { b"0\n" });
        }
    }
}

fn patched_test_exe() -> PathBuf {
    static PATCHED: OnceLock<PathBuf> = OnceLock::new();
    PATCHED
        .get_or_init(|| {
            let src = std::env::current_exe().expect("current_exe");
            let dst = work_root().join(format!("import-test-{}", std::process::id()));
            let _ = fs::remove_file(&dst);
            reflink_copy(&src, &dst);
            patch_embedded_ca(&dst, &assets().ca_padded);
            chmod_exec(&dst);
            dst
        })
        .clone()
}

fn zeroize_worker() {
    let _guard = HookGuard;
    ZERO_HITS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *AFTER_EACH_WIPE.lock().unwrap_or_else(|e| e.into_inner()) = Some(wipe_hook);
    let snapshot = std::env::var_os("T9_SNAPSHOT")
        .map(PathBuf::from)
        .expect("T9_SNAPSHOT");
    let opts = Options {
        snapshot: Some(snapshot),
        ..Options::default()
    };
    match secd_import_legacy::import(&opts) {
        Ok(()) => {}
        Err(ImportError::NoSdxd) => panic!("import: NoSdxd"),
        Err(ImportError::SdxdLocked) => panic!("import: SdxdLocked"),
        Err(ImportError::SecdLocked) => panic!("import: SecdLocked"),
        Err(ImportError::Refused(m)) => panic!("import refused: {m}"),
        Err(ImportError::Mismatch(m)) => panic!("import: {m}"),
        Err(ImportError::Other(e)) => panic!("import: {e}"),
    }
    let hits = ZERO_HITS.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(hits.len(), 2, "hook after each name");
    assert!(hits.iter().all(|&z| z), "test hooks must see buffer zeroed");
}

struct HookGuard;

impl Drop for HookGuard {
    fn drop(&mut self) {
        *AFTER_EACH_WIPE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        ZERO_HITS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

#[test]
fn T_IMP_NOTTY() {
    let h = Harness::new();
    let out = h.run_notty();
    assert!(!out.status.success(), "non-TTY must exit != 0");
    assert_eq!(h.put_count(), 0, "non-TTY must not PUT");
    assert!(!leaked(&utf8(&out.stdout)), "fixture leaked on stdout");
    assert!(!leaked(&utf8(&out.stderr)), "fixture leaked on stderr");
}

#[test]
fn T_IMP_NO_SDXD() {
    let empty = unique("nopath");
    fs::create_dir_all(&empty).expect("empty path");
    let bin = Path::new(env!("CARGO_BIN_EXE_secd-import-legacy"));
    let out = pty_run(bin, &[], |cmd| {
        cmd.env("PATH", &empty)
            .env_remove("LD_PRELOAD")
            .env_remove("SECD_TEST_REDIR");
    });
    let _ = fs::remove_dir_all(&empty);
    assert_eq!(out.status.code(), Some(2), "missing sdxd must exit 2");
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(
        stderr.contains(NO_SDXD) || stdout.contains(NO_SDXD),
        "missing sdxd locked sentence"
    );
}

#[test]
fn T_IMP_COUNT() {
    let h = Harness::new();
    let snap = unique("snap");
    let out = h.run_tty(&["--snapshot", snap.to_str().expect("utf8")], &[]);
    assert!(out.status.success(), "import failed: {}", utf8(&out.stderr));
    let stdout = utf8(&out.stdout);
    let stderr = utf8(&out.stderr);
    assert!(stdout.contains(NAME_A), "must print {NAME_A}");
    assert!(stdout.contains(NAME_B), "must print {NAME_B}");
    assert!(stdout.contains("imported 2"), "must print imported count");
    assert!(!leaked(&stdout), "fixture value leaked on stdout");
    assert!(!leaked(&stderr), "fixture value leaked on stderr");

    let mode = fs::metadata(&snap).expect("snapshot written").permissions();
    assert_eq!(mode.mode() & 0o777, 0o600, "snapshot must be 0600");
    let pre = fs::read_to_string(&snap).expect("snapshot utf8");
    assert!(pre.contains("entries"), "snapshot is not a vault body");
    assert!(!leaked(&pre), "snapshot holds plaintext");
    assert_eq!(snapshot_names(&snap), Vec::<String>::new(), "pre-image");
    assert_eq!(
        h.put_body(&pre),
        200,
        "the pre-image must PUT back as it is"
    );
    assert!(
        h.stored_names().is_empty(),
        "the restore must undo the import"
    );
}

#[test]
fn T_IMP_ZEROIZE() {
    if std::env::var_os("T9_ZEROIZE_WORKER").is_some() {
        zeroize_worker();
        return;
    }

    let h = Harness::new();
    let hits = unique("zero-hits");
    let snap = unique("snap");
    write_0600(&hits, b"");
    let mut cmd = Command::new(patched_test_exe());
    h.child_env(&mut cmd);
    let out = cmd
        .env("T9_ZEROIZE_WORKER", "1")
        .env("T9_ZEROIZE_OUT", &hits)
        .env("T9_SNAPSHOT", &snap)
        .args(["--exact", "T_IMP_ZEROIZE"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("zeroize worker");
    assert!(
        out.status.success(),
        "import worker failed: {}",
        utf8(&out.stderr)
    );
    let raw = fs::read_to_string(&hits).unwrap_or_default();
    let bits: Vec<bool> = raw
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l == "1")
        .collect();
    assert_eq!(bits.len(), 2, "hook after each name");
    assert!(bits.iter().all(|&z| z), "test hooks must see buffer zeroed");
    assert!(!leaked(&utf8(&out.stdout)), "fixture leaked on stdout");
    assert!(!leaked(&utf8(&out.stderr)), "fixture leaked on stderr");
}

/// An entry this DEK cannot open is dropped on load, and the save is a
/// whole-vault replace, so saving would delete it. Refuse instead.
#[test]
fn T_IMP_UNDECODABLE() {
    let h =
        Harness::seeded(r#"{"entries":[{"name":"kv/gamma","ciphertext":"deadbeef","meta":{}}]}"#);
    let out = h.import(&[], &[]);
    assert!(!out.status.success(), "must refuse an undecodable entry");
    assert_eq!(h.put_count(), 0, "must not PUT");
    assert_eq!(
        h.stored_names(),
        vec!["kv/gamma".to_string()],
        "the entry must still be there"
    );
    let said = format!("{}{}", utf8(&out.stdout), utf8(&out.stderr));
    assert!(said.contains("did not decode"), "must say why: {said}");
    assert!(!leaked(&said), "fixture leaked");
}

/// `PUT /api/v1/vault` replaces the whole vault; without the pre-image there is
/// nothing to restore from.
#[test]
fn T_IMP_SNAPSHOT_REQUIRED() {
    let h = Harness::new();
    let out = h.run_tty(&[], &[]);
    assert_eq!(out.status.code(), Some(2), "no --snapshot must exit 2");
    assert_eq!(h.put_count(), 0, "must not PUT");
    let said = format!("{}{}", utf8(&out.stdout), utf8(&out.stderr));
    assert!(said.contains("--snapshot"), "must name the flag: {said}");
}

#[test]
fn T_IMP_SNAPSHOT_EXISTS() {
    let h = Harness::new();
    let snap = unique("snap");
    write_0600(&snap, b"earlier pre-image\n");
    let out = h.run_tty(&["--snapshot", snap.to_str().expect("utf8")], &[]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "an existing snapshot must exit 2"
    );
    assert_eq!(h.put_count(), 0, "must not PUT");
    assert_eq!(
        fs::read_to_string(&snap).expect("snapshot"),
        "earlier pre-image\n",
        "must not overwrite a pre-image"
    );
}

/// There is no compare-and-set on the vault, so a save landing right after this
/// one wins. The read-back cannot prevent that; it must not stay silent.
#[test]
fn T_IMP_READ_BACK() {
    let h = Harness::new();
    let first = h.import(&["--prefix", NAME_A], &[]);
    assert!(
        first.status.success(),
        "seed import: {}",
        utf8(&first.stderr)
    );

    h.arm_clobber(r#"{"entries":[{"name":"kv/other","ciphertext":"abcdef","meta":{}}]}"#);
    let snap = unique("snap");
    let out = h.run_tty(
        &["--overwrite", "--snapshot", snap.to_str().expect("utf8")],
        &[],
    );
    assert!(!out.status.success(), "a clobbered write must exit != 0");
    assert_eq!(
        snapshot_names(&snap),
        vec![NAME_A.to_string()],
        "the pre-image is the vault before the write"
    );
    let stdout = utf8(&out.stdout);
    assert!(
        !stdout.contains("imported"),
        "must not report a write it could not confirm: {stdout}"
    );
    let said = format!("{stdout}{}", utf8(&out.stderr));
    assert!(
        said.contains("changed under the write"),
        "must say so: {said}"
    );
    assert!(!leaked(&said), "fixture leaked");
}

#[test]
fn T_IMP_PREFIX() {
    let h = Harness::new();
    let names = format!("{NAME_A} {NAME_B} {NAME_JSON}");
    let out = h.import(&["--prefix", "kv/"], &[("T9_SDXD_NAMES", names.as_str())]);
    assert!(out.status.success(), "import failed: {}", utf8(&out.stderr));
    let stdout = utf8(&out.stdout);
    assert!(
        stdout.contains("imported 2"),
        "must import only kv/: {stdout}"
    );
    assert!(!stdout.contains(NAME_JSON), "out of prefix: {stdout}");
    assert_eq!(
        h.stored_names(),
        vec![NAME_A.to_string(), NAME_B.to_string()]
    );
    assert!(!leaked(&stdout), "fixture leaked");
}

#[test]
fn T_IMP_DRY_RUN() {
    let h = Harness::new();
    let first = h.import(&["--prefix", NAME_A], &[]);
    assert!(
        first.status.success(),
        "seed import: {}",
        utf8(&first.stderr)
    );
    assert_eq!(h.put_count(), 1);

    // No --snapshot: a dry run writes nothing, so it needs no pre-image.
    let out = h.run_tty(&["--dry-run"], &[]);
    assert!(
        out.status.success(),
        "dry run failed: {}",
        utf8(&out.stderr)
    );
    let stdout = utf8(&out.stdout);
    assert!(
        stdout.contains(&format!("{NAME_A} overwrite")),
        "must call the collision an overwrite: {stdout}"
    );
    assert!(
        stdout.contains(&format!("{NAME_B} new")),
        "must call the new name new: {stdout}"
    );
    assert!(
        stdout.contains("dry-run: 1 new, 1 overwrite"),
        "must print totals: {stdout}"
    );
    assert_eq!(h.put_count(), 1, "a dry run must not PUT");
    assert_eq!(h.stored_names(), vec![NAME_A.to_string()]);
    assert!(!leaked(&stdout), "fixture leaked");
}

#[test]
fn T_IMP_NO_OVERWRITE() {
    let h = Harness::new();
    let first = h.import(&["--prefix", NAME_A], &[]);
    assert!(
        first.status.success(),
        "seed import: {}",
        utf8(&first.stderr)
    );
    assert_eq!(h.put_count(), 1);

    let out = h.import(&[], &[]);
    assert_eq!(out.status.code(), Some(2), "a collision must exit 2");
    assert_eq!(h.put_count(), 1, "must not PUT over a name");
    let said = format!("{}{}", utf8(&out.stdout), utf8(&out.stderr));
    assert!(said.contains("--overwrite"), "must name the flag: {said}");
    assert_eq!(h.stored_names(), vec![NAME_A.to_string()]);

    let snap = unique("snap");
    let out = h.run_tty(
        &["--overwrite", "--snapshot", snap.to_str().expect("utf8")],
        &[],
    );
    assert!(
        out.status.success(),
        "--overwrite must be accepted: {}",
        utf8(&out.stderr)
    );
    assert_eq!(
        h.stored_names(),
        vec![NAME_A.to_string(), NAME_B.to_string()]
    );

    // The pre-image is the vault as the run found it, and it goes back as it is.
    assert_eq!(
        snapshot_names(&snap),
        vec![NAME_A.to_string()],
        "the pre-image is the state before the run"
    );
    let pre = fs::read_to_string(&snap).expect("snapshot utf8");
    assert!(!pre.contains("version"), "PUT rejects a version key: {pre}");
    assert_eq!(h.put_body(&pre), 200, "the pre-image must restore");
    assert_eq!(
        h.stored_names(),
        vec![NAME_A.to_string()],
        "the restore must undo the run"
    );
    assert_eq!(
        h.put_body(&with_version(&pre)),
        400,
        "the route takes name, ciphertext and meta only"
    );
}

#[test]
fn T_IMP_VERIFY() {
    let h = Harness::new();
    let first = h.import(&[], &[]);
    assert!(
        first.status.success(),
        "seed import: {}",
        utf8(&first.stderr)
    );

    let out = h.run_tty(&["--verify"], &[]);
    assert!(out.status.success(), "verify failed: {}", utf8(&out.stderr));
    let stdout = utf8(&out.stdout);
    assert!(stdout.contains(&format!("{NAME_A} ok")), "{stdout}");
    assert!(stdout.contains(&format!("{NAME_B} ok")), "{stdout}");
    assert!(!stdout.contains("MISMATCH"), "{stdout}");
    assert!(!leaked(&stdout), "fixture leaked");

    // Same names, a different value in the old CLI.
    let out = h.run_tty(&["--verify"], &[("T9_SDXD_VALUE", FIXTURE_ALT)]);
    assert!(!out.status.success(), "a mismatch must exit != 0");
    let stdout = utf8(&out.stdout);
    assert!(stdout.contains(&format!("{NAME_A} MISMATCH")), "{stdout}");
    assert!(stdout.contains(&format!("{NAME_B} MISMATCH")), "{stdout}");
    assert_eq!(h.put_count(), 1, "verify must not PUT");
    assert!(!leaked(&stdout), "fixture leaked");
    assert!(!leaked(&utf8(&out.stderr)), "fixture leaked on stderr");
}

/// The console draws one masked row without `meta.fields`.
#[test]
fn T_IMP_FIELDS() {
    let h = Harness::new();
    let out = h.import(&[], &[("T9_SDXD_NAMES", NAME_JSON)]);
    assert!(out.status.success(), "import failed: {}", utf8(&out.stderr));
    let stored = h.stored();
    let entry = stored
        .iter()
        .find(|e| e.get("name").and_then(Value::as_str) == Some(NAME_JSON))
        .expect("stored entry");
    let fields: Vec<&str> = entry
        .get("meta")
        .and_then(|m| m.get("fields"))
        .and_then(Value::as_array)
        .expect("meta.fields")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(fields.contains(&"api_key"), "fields: {fields:?}");
    assert!(fields.contains(&"url"), "fields: {fields:?}");
    let raw = serde_json::to_string(entry).expect("entry json");
    assert!(!leaked(&raw), "fixture leaked into meta");
}

const SDXD_SH: &str = r#"#!/bin/sh
names="${T9_SDXD_NAMES:-kv/alpha kv/beta}"
val="${T9_SDXD_VALUE:-t9-fix-IMP-val-do-not-print-c4e1}"
case "$1" in
  ls) for n in $names; do printf '%s\n' "$n"; done ;;
  info) printf '%s\n' 'note: lab' 'service: xai' ;;
  get)
    if [ "$2" = "--force" ]; then
      case "$3" in
        js/*) printf '{"api_key":"%s","url":"https://lab.test"}\n' "$val" ;;
        *) printf '%s\n' "$val" ;;
      esac
      exit 0
    fi
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;

const VAULT_PY: &str = r#"
import json, os, ssl, sys, traceback
from http.server import BaseHTTPRequestHandler, HTTPServer

CERT, KEY, PUT_LOG, STATE, CLOBBER = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

def load():
    try:
        with open(STATE) as f:
            return json.load(f)
    except Exception:
        return {"entries": []}

def store(doc):
    tmp = STATE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(doc, f)
    os.replace(tmp, STATE)

class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        return

    def _send(self, code, data):
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=UTF-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)
        try:
            self.wfile.flush()
        except Exception:
            pass
        try:
            self.connection.unwrap()
        except Exception:
            pass

    def do_GET(self):
        # The real route returns a version per entry, which PUT then rejects.
        rows = []
        for e in load().get("entries", []):
            row = dict(e)
            row["version"] = 1
            rows.append(row)
        self._send(200, json.dumps({"entries": rows}).encode())

    def do_PUT(self):
        n = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(n) if n else b""
        try:
            doc = json.loads(raw.decode())
        except Exception:
            doc = {}
        entries = doc.get("entries", [])
        allowed = ("name", "ciphertext", "meta")
        for e in entries:
            if not isinstance(e, dict) or any(k not in allowed for k in e):
                self._send(400, b'{"error":"plaintext"}')
                return
        store({"entries": entries})
        with open(PUT_LOG, "ab") as f:
            f.write(b"PUT\n")
        # A whole-vault save landing right after this one. PUT takes no version,
        # so the last writer wins and only a read-back notices.
        if os.path.exists(CLOBBER):
            with open(CLOBBER) as f:
                store(json.load(f))
        self._send(200, b'{"ok":true}')

class S(HTTPServer):
    def get_request(self):
        sock, addr = self.socket.accept()
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.minimum_version = ssl.TLSVersion.TLSv1_3
        ctx.load_cert_chain(CERT, KEY)
        try:
            ctx.set_alpn_protocols(["http/1.1"])
        except ssl.SSLError:
            pass
        try:
            return ctx.wrap_socket(sock, server_side=True), addr
        except Exception:
            traceback.print_exc()
            sock.close()
            raise

httpd = S(("127.0.0.1", 0), H)
print(httpd.server_address[1], flush=True)
httpd.serve_forever()
"#;

const RESTORE_PY: &str = r#"
import socket, ssl, sys

ca, port = sys.argv[1], int(sys.argv[2])
body = sys.stdin.buffer.read()
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
ctx.minimum_version = ssl.TLSVersion.TLSv1_3
ctx.load_verify_locations(ca)
raw = socket.create_connection(("127.0.0.1", port), timeout=10)
sock = ctx.wrap_socket(raw, server_hostname="secd.imabee.com")
head = (
    "PUT /api/v1/vault HTTP/1.1\r\n"
    "Host: secd.imabee.com\r\n"
    "Authorization: Bearer t9-token\r\n"
    "Content-Type: application/json\r\n"
    f"Content-Length: {len(body)}\r\n"
    "Connection: close\r\n\r\n"
).encode()
sock.sendall(head + body)
buf = b""
while True:
    chunk = sock.recv(65536)
    if not chunk:
        break
    buf += chunk
print(buf.split(b"\r\n", 1)[0].decode().split(" ")[1])
"#;

const PTY_PY: &str = r#"
import os, pty, select, subprocess, sys

err_path = sys.argv[1]
argv = sys.argv[2:]
master, slave = pty.openpty()
with open(err_path, "wb") as err:
    p = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=err, close_fds=True)
os.close(slave)
out = bytearray()
try:
    while True:
        timeout = 0.05 if p.poll() is not None else 0.2
        r, _, _ = select.select([master], [], [], timeout)
        if r:
            try:
                chunk = os.read(master, 8192)
            except OSError:
                break
            if not chunk:
                break
            out.extend(chunk)
        elif p.poll() is not None:
            break
finally:
    os.close(master)
rc = p.wait()
sys.stdout.buffer.write(out)
raise SystemExit(rc if rc is not None and rc >= 0 else 1)
"#;

const REDIR_C: &str = r#"
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <dlfcn.h>
#include <netdb.h>
#include <netinet/in.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>

int getaddrinfo(const char *node, const char *service,
                const struct addrinfo *hints, struct addrinfo **res) {
    static int (*real_getaddrinfo)(const char *, const char *, const struct addrinfo *, struct addrinfo **) = 0;
    if (!real_getaddrinfo)
        real_getaddrinfo = dlsym(RTLD_NEXT, "getaddrinfo");
    if (node && strcmp(node, "secd.imabee.com") == 0)
        return real_getaddrinfo("192.168.101.122", service, hints, res);
    return real_getaddrinfo(node, service, hints, res);
}

int connect(int sockfd, const struct sockaddr *addr, socklen_t addrlen) {
    static int (*real_connect)(int, const struct sockaddr *, socklen_t) = 0;
    if (!real_connect)
        real_connect = dlsym(RTLD_NEXT, "connect");
    if (addr && addr->sa_family == AF_INET && addrlen >= (socklen_t)sizeof(struct sockaddr_in)) {
        struct sockaddr_in copy = *(const struct sockaddr_in *)addr;
        unsigned ip = ntohl(copy.sin_addr.s_addr);
        unsigned port = ntohs(copy.sin_port);
        if (port == 443 && ip == 0xC0A8657Au) {
            const char *redir = getenv("SECD_TEST_REDIR");
            unsigned hip = 0x7F000001u;
            unsigned hport = 18443;
            if (redir) {
                char buf[64];
                strncpy(buf, redir, sizeof buf - 1);
                buf[sizeof buf - 1] = 0;
                char *col = strchr(buf, ':');
                if (col) {
                    *col = 0;
                    hport = (unsigned)atoi(col + 1);
                    hip = ntohl(inet_addr(buf));
                }
            }
            copy.sin_addr.s_addr = htonl(hip);
            copy.sin_port = htons((unsigned short)hport);
            return real_connect(sockfd, (struct sockaddr *)&copy, addrlen);
        }
    }
    return real_connect(sockfd, addr, addrlen);
}
"#;
