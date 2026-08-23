//! Shared T7 harness. Fixture may live in this crate; never print it.

#![allow(dead_code)]

use std::fs::{self, File};
use std::future::Future;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use secd_core::seal;
use secd_web::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;
use zeroize::Zeroize;

pub const FIXTURE: &str = "t7-fix-G1tEa-tok-do-not-print-7a3c";
pub const LOCKED: &str = "secd: locked — run secd";
pub const GITEA_HOST: &str = "git.appsynergy.io";
pub const GITEA_URL: &str = "https://git.appsynergy.io";

const ROOT_PEM: &[u8] = include_bytes!("../../keys/appsynergy-root.pem");

pub static ENV: Mutex<()> = Mutex::new(());
static SEQ: AtomicU64 = AtomicU64::new(1);
static ASSETS: OnceLock<Assets> = OnceLock::new();

pub struct Assets {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub secd: PathBuf,
    pub redir: PathBuf,
    pub git: PathBuf,
}

pub struct Harness {
    pub home: PathBuf,
    pub runtime: PathBuf,
    port: u16,
    _data: PathBuf,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = with_secd_env(&self.home, Some(&self.runtime), secd::keyring::delete);
        let _ = fs::remove_dir_all(&self.home);
        let _ = fs::remove_dir_all(&self.runtime);
        let _ = fs::remove_dir_all(&self._data);
    }
}

pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_var(key: &str, val: impl AsRef<std::ffi::OsStr>) {
    // SAFETY: caller holds ENV; only T7 tests mutate these keys.
    unsafe { std::env::set_var(key, val) }
}

pub fn remove_var(key: &str) {
    // SAFETY: caller holds ENV.
    unsafe { std::env::remove_var(key) }
}

pub fn with_secd_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
    with_secd_env(home, None, f)
}

pub fn with_secd_env<T>(home: &Path, runtime: Option<&Path>, f: impl FnOnce() -> T) -> T {
    let _g = env_lock();
    let prev_home = std::env::var_os("SECD_HOME");
    let prev_rt = std::env::var_os("XDG_RUNTIME_DIR");
    set_var("SECD_HOME", home);
    if let Some(rt) = runtime {
        set_var("XDG_RUNTIME_DIR", rt);
    }
    let out = f();
    match prev_home {
        Some(v) => set_var("SECD_HOME", v),
        None => remove_var("SECD_HOME"),
    }
    if runtime.is_some() {
        match prev_rt {
            Some(v) => set_var("XDG_RUNTIME_DIR", v),
            None => remove_var("XDG_RUNTIME_DIR"),
        }
    }
    out
}

pub fn assets() -> &'static Assets {
    ASSETS.get_or_init(build_assets)
}

pub fn gitea_blob(url: &str, user: &str) -> Vec<u8> {
    json!({"token": FIXTURE, "url": url, "user": user})
        .to_string()
        .into_bytes()
}

pub fn leaked(hay: &str) -> bool {
    hay.contains(FIXTURE)
}

pub fn assert_no_value(hay: &str, what: &str) {
    assert!(!leaked(hay), "fixture value leaked on {what}");
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    openssl::sha::sha256(bytes)
}

pub fn utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn isolated_secd(args: &[&str]) -> Command {
    let home = unique("iso-home");
    let runtime = unique("iso-run");
    fs::create_dir_all(&home).expect("iso home");
    fs::create_dir_all(&runtime).expect("iso runtime");
    let mut c = Command::new(env!("CARGO_BIN_EXE_secd"));
    c.args(args)
        .env("SECD_HOME", home)
        .env("XDG_RUNTIME_DIR", runtime)
        .env_remove("GITEA_TOKEN")
        .env_remove("GITEA_URL")
        .env_remove("GITEA_USER")
        .stdin(Stdio::null());
    c
}

impl Harness {
    pub fn new(entries: &[(&str, Vec<u8>)]) -> Self {
        let with_meta: Vec<(&str, Vec<u8>, Value)> = entries
            .iter()
            .map(|(name, plain)| (*name, plain.clone(), json!({})))
            .collect();
        Self::new_with_meta(&with_meta)
    }

    /// Like `new`, but seeds each entry with the given `meta` instead of `{}`.
    pub fn new_with_meta(entries: &[(&str, Vec<u8>, Value)]) -> Self {
        let a = assets();
        let home = unique("home");
        let runtime = unique("run");
        let data = unique("data");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&runtime).expect("runtime");
        fs::create_dir_all(&data).expect("data");

        let state = AppState::open(&data).expect("appstate");
        let (_id, token) = state
            .sessions
            .create_device("t7@secd.test", "t7host")
            .expect("device session");
        let mut dek = [0u8; 32];
        File::open("/dev/urandom")
            .expect("urandom")
            .read_exact(&mut dek)
            .expect("dek");

        let mut body_entries = Vec::new();
        for (name, plain, meta) in entries {
            let blob = seal(&dek, name, plain).expect("seal");
            body_entries.push(json!({
                "name": name,
                "ciphertext": hex::encode(blob),
                "meta": meta,
            }));
        }
        let app = secd_web::app(state.clone());
        let (status, _) = block_on(put_vault(&app, &json!({ "entries": body_entries }), &token));
        assert_eq!(status, StatusCode::OK, "seed vault");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let addr = listener.local_addr().expect("addr");
        let tls = secd_web::tls::rustls_config(&a.cert, &a.key).expect("tls");
        let serve_app = secd_web::app(state);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("server rt");
            rt.block_on(async move {
                axum_server::from_tcp_rustls(listener, tls)
                    .serve(serve_app.into_make_service_with_connect_info::<SocketAddr>())
                    .await
                    .expect("serve");
            });
        });
        wait_port(addr);

        write_session(&home, token.as_bytes());
        with_secd_env(&home, Some(&runtime), || {
            secd::keyring::store(&dek).expect("store")
        });
        // Child cannot see a parent session keyring. Seed the EPERM fallback
        // so load() finds the DEK when user keyctl is denied.
        seed_runtime_dek(&home, &runtime, &dek);
        dek.zeroize();

        Self {
            home,
            runtime,
            port: addr.port(),
            _data: data,
        }
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(&assets().secd);
        c.args(args)
            .env("SECD_HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("LD_PRELOAD", &assets().redir)
            .env("SECD_TEST_REDIR", format!("127.0.0.1:{}", self.port))
            .env_remove("GITEA_TOKEN")
            .env_remove("GITEA_URL")
            .env_remove("GITEA_USER")
            .stdin(Stdio::null());
        c
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("secd")
    }

    pub fn git_credential(&self, request: &str) -> Output {
        let req = self.home.join("git-req");
        fs::write(&req, request.as_bytes()).expect("req");
        Command::new(&assets().git)
            .env("SECD_BIN", &assets().secd)
            .env("SECD_GIT_REQFILE", &req)
            .env("SECD_HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("LD_PRELOAD", &assets().redir)
            .env("SECD_TEST_REDIR", format!("127.0.0.1:{}", self.port))
            .env_remove("GITEA_TOKEN")
            .env_remove("GITEA_URL")
            .env_remove("GITEA_USER")
            .output()
            .expect("git wrapper")
    }
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
    let out = Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-sha256", "-days", "1", "-nodes", "-keyout",
        ])
        .arg(&ca_key)
        .arg("-out")
        .arg(&ca)
        .args([
            "-subj",
            "/CN=T7 Test CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
        ])
        .output()
        .expect("openssl ca");
    assert!(out.status.success(), "openssl ca failed");
    let out = Command::new("openssl")
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
    let out = Command::new("openssl")
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

    let src = fs::read(env!("CARGO_BIN_EXE_secd")).expect("read secd");
    assert!(
        src.windows(ROOT_PEM.len()).any(|w| w == ROOT_PEM),
        "embedded root PEM not found in secd"
    );
    let secd_bytes = replace_all(&src, ROOT_PEM, &padded);
    let secd = dir.join("secd");
    fs::write(&secd, secd_bytes).expect("write patched secd");
    chmod_exec(&secd);

    let redir_c = dir.join("redir.c");
    fs::write(&redir_c, REDIR_C).expect("redir.c");
    let redir = dir.join("redir.so");
    let st = Command::new("cc")
        .args(["-shared", "-fPIC", "-o"])
        .arg(&redir)
        .arg(&redir_c)
        .arg("-ldl")
        .status()
        .expect("cc redir");
    assert!(st.success(), "cc redir.so failed");

    let git_c = dir.join("git.c");
    fs::write(&git_c, GIT_C).expect("git.c");
    let git = dir.join("git");
    let st = Command::new("cc")
        .arg("-o")
        .arg(&git)
        .arg(&git_c)
        .status()
        .expect("cc git");
    assert!(st.success(), "cc git wrapper failed");

    Assets {
        cert,
        key,
        secd,
        redir,
        git,
    }
}

fn replace_all(hay: &[u8], needle: &[u8], repl: &[u8]) -> Vec<u8> {
    assert_eq!(needle.len(), repl.len());
    let mut out = hay.to_vec();
    let n = needle.len();
    let mut i = 0;
    while i + n <= out.len() {
        if &out[i..i + n] == needle {
            out[i..i + n].copy_from_slice(repl);
            i += n;
        } else {
            i += 1;
        }
    }
    out
}

fn unique(tag: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("secd-t7-{tag}-{}-{n}", std::process::id()))
}

fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt")
        .block_on(f)
}

async fn put_vault(app: &Router, body: &Value, bearer: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::PUT)
        .uri("/api/v1/vault")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .body(Body::from(body.to_string().into_bytes()))
        .expect("req");
    let res = app.clone().oneshot(req).await.expect("oneshot");
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn wait_port(addr: SocketAddr) {
    for _ in 0..100 {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("tls server did not start");
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

fn seed_runtime_dek(home: &Path, runtime: &Path, dek: &[u8]) {
    let dir = runtime.join("secd");
    fs::create_dir_all(&dir).expect("runtime secd");
    let mut perms = fs::metadata(&dir).expect("runtime meta").permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&dir, perms).expect("runtime 0700");
    let path = dir.join(dek_desc(home));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .expect("runtime dek");
    f.write_all(dek).expect("runtime dek write");
    f.flush().expect("runtime dek flush");
    let mut perms = f.metadata().expect("dek meta").permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&path, perms).expect("runtime dek 0600");
}

fn write_session(home: &Path, token: &[u8]) {
    let path = home.join("login.session");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .expect("session");
    f.write_all(token).expect("session write");
    f.flush().expect("session flush");
    let mut perms = f.metadata().expect("meta").permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&path, perms).expect("session mode");
}

fn chmod_exec(path: &Path) {
    let mut p = fs::metadata(path).expect("meta").permissions();
    p.set_mode(0o755);
    fs::set_permissions(path, p).expect("chmod");
}

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

const GIT_C: &str = r#"
#include <fcntl.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>
int main(void) {
    const char *req = getenv("SECD_GIT_REQFILE");
    const char *bin = getenv("SECD_BIN");
    pid_t pid;
    int st = 0;
    if (!bin)
        return 127;
    pid = fork();
    if (pid < 0)
        return 127;
    if (pid == 0) {
        if (req) {
            int fd = open(req, O_RDONLY);
            if (fd >= 0) {
                if (dup2(fd, 0) >= 0) {}
                close(fd);
            }
        }
        char *args[] = {"secd", "git-credential", 0};
        execv(bin, args);
        _exit(127);
    }
    if (waitpid(pid, &st, 0) < 0)
        return 127;
    if (WIFEXITED(st))
        return WEXITSTATUS(st);
    return 1;
}
"#;
