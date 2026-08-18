//! `secd update` / `secd update --check`.
//!
//! Fetch latest.json → download binary+sig → sha256 then signature →
//! `secd.new`, fsync, rename. Fail closed: dest bytes unchanged, staging gone.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde_json::Value;

pub const MANIFEST_URL: &str =
    "https://github.com/Appsynergy-io/secd/releases/latest/download/latest.json";
pub const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "git.appsynergy.io",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];
pub const COSIGN_PUB: &str = include_str!("../keys/cosign.pub");

const BIN_CAP: usize = 64 * 1024 * 1024;
const TEXT_CAP: usize = 256 * 1024;

pub struct Release {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub sig: String,
}

pub fn run() -> anyhow::Result<()> {
    let matches = crate::cli().get_matches();
    let check = matches
        .subcommand_matches("update")
        .map(|s| s.get_flag("check"))
        .unwrap_or(false);
    if !check {
        crate::skills_install::run()?;
    }
    let dest = std::env::current_exe().context("current exe")?;
    apply_from_url(&dest, MANIFEST_URL, check, COSIGN_PUB)
}

pub fn apply_from_url(
    dest: &Path,
    manifest_url: &str,
    check_only: bool,
    pubkey_pem: &str,
) -> anyhow::Result<()> {
    if !url_allowed(manifest_url) {
        anyhow::bail!("refusing host");
    }
    if pacman_owns(dest) {
        anyhow::bail!("pacman owns this path");
    }
    let raw = https_get(manifest_url, TEXT_CAP)?;
    let rel = parse_manifest(&raw, target_triple()?)?;
    let same = match fs::read(dest) {
        Ok(cur) => eq_hex(&sha256_hex(&cur), &rel.sha256),
        Err(_) => false,
    };
    if check_only {
        if same {
            println!("secd {} up to date", env!("CARGO_PKG_VERSION"));
        } else {
            println!("secd {} available", rel.version);
        }
        return Ok(());
    }
    if same {
        println!("secd {} up to date", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let payload = https_get(&rel.url, BIN_CAP)?;
    let sig = https_get(&rel.sig, TEXT_CAP)?;
    apply(dest, &payload, &rel.sha256, &sig, pubkey_pem)?;
    println!("secd {}", rel.version);
    Ok(())
}

pub fn target_triple() -> anyhow::Result<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Ok("x86_64-unknown-linux-musl")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Ok("aarch64-apple-darwin")
    } else {
        anyhow::bail!("unsupported target")
    }
}

fn host_allowed(host: &str) -> bool {
    ALLOWED_HOSTS
        .iter()
        .any(|allowed| host.eq_ignore_ascii_case(allowed))
}

pub fn url_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    if rest.contains('@') {
        return false;
    }
    let hostport = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if hostport.is_empty() {
        return false;
    }
    match hostport.split_once(':') {
        None => host_allowed(hostport),
        Some((h, p)) => host_allowed(h) && p == "443",
    }
}

/// Next hop for 301/302/307/308. `None` if `status` is not a redirect.
pub fn next_redirect(
    status: u16,
    location: Option<&str>,
    current_url: &str,
) -> anyhow::Result<Option<String>> {
    if !matches!(status, 301 | 302 | 307 | 308) {
        return Ok(None);
    }
    let loc = location
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("redirect missing location"))?;
    let next = if loc.starts_with("https://") {
        loc.to_string()
    } else if loc.starts_with("http://") || loc.starts_with("//") {
        anyhow::bail!("refusing host");
    } else {
        let origin = https_origin(current_url)?;
        if loc.starts_with('/') {
            format!("{origin}{loc}")
        } else {
            format!("{origin}/{loc}")
        }
    };
    if !url_allowed(&next) {
        anyhow::bail!("refusing host");
    }
    Ok(Some(next))
}

fn https_origin(url: &str) -> anyhow::Result<String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| anyhow!("refusing host"))?;
    if rest.contains('@') {
        anyhow::bail!("refusing host");
    }
    let hostport = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if hostport.is_empty() {
        anyhow::bail!("refusing host");
    }
    Ok(format!("https://{hostport}"))
}

pub fn parse_manifest(raw: &[u8], triple: &str) -> anyhow::Result<Release> {
    let v: Value = serde_json::from_slice(raw).context("manifest json")?;
    let version = v
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest missing version"))?;
    let t = v
        .get("targets")
        .and_then(Value::as_object)
        .and_then(|m| m.get(triple))
        .ok_or_else(|| anyhow!("manifest missing target {triple}"))?;
    let url = t
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest missing url"))?;
    let sha256 = t
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest missing sha256"))?;
    let sig = t
        .get("sig")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest missing sig"))?;
    if !url_allowed(url) || !url_allowed(sig) {
        anyhow::bail!("refusing host");
    }
    Ok(Release {
        version: version.to_string(),
        url: url.to_string(),
        sha256: sha256.to_string(),
        sig: sig.to_string(),
    })
}

pub fn pacman_owns(path: &Path) -> bool {
    let Some(p) = path.to_str() else {
        return false;
    };
    let pacman = "pacman";
    let status = std::process::Command::new(pacman)
        .args(["-Qo", p])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(status, Ok(s) if s.success())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    sha256_bytes(bytes).map(hex::encode).unwrap_or_default()
}

pub fn apply(
    dest: &Path,
    payload: &[u8],
    expected_sha256: &str,
    signature: &[u8],
    pubkey_pem: &str,
) -> anyhow::Result<()> {
    if pacman_owns(dest) {
        anyhow::bail!("pacman owns this path");
    }
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("dest has no parent"))?;
    fs::create_dir_all(parent)?;
    let staging = parent.join("secd.new");
    let _ = fs::remove_file(&staging);
    struct Guard(PathBuf);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let guard = Guard(staging.clone());
    write_staging(&staging, payload)?;
    let written = fs::read(&staging).context("read staging")?;
    let got = sha256_bytes(&written)?;
    if !eq_hex(&hex::encode(&got), expected_sha256) {
        anyhow::bail!("sha256 mismatch");
    }
    verify_sig(&written, signature, pubkey_pem)?;
    fsync_path(&staging)?;
    fs::rename(&staging, dest).context("rename secd.new")?;
    std::mem::forget(guard);
    fsync_path(parent)?;
    Ok(())
}

pub fn verify_sig(payload: &[u8], signature: &[u8], pubkey_pem: &str) -> anyhow::Result<()> {
    let dir = scratch_dir()?;
    struct Wipe(PathBuf);
    impl Drop for Wipe {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _wipe = Wipe(dir.clone());
    let pay = dir.join("p");
    let key = dir.join("k");
    let sigp = dir.join("s");
    fs::write(&pay, payload)?;
    fs::write(&key, pubkey_pem.as_bytes())?;
    for cand in sig_candidates(signature) {
        fs::write(&sigp, &cand)?;
        if openssl_verify(&key, &sigp, &pay) || cosign_verify(&key, &sigp, &pay) {
            return Ok(());
        }
    }
    fs::write(&sigp, signature)?;
    if cosign_verify(&key, &sigp, &pay) {
        return Ok(());
    }
    anyhow::bail!("signature")
}

fn sha256_bytes(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let openssl = "openssl";
    let mut child = std::process::Command::new(openssl)
        .args(["dgst", "-sha256", "-binary"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("openssl")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(bytes)?;
    }
    let out = child.wait_with_output().context("openssl sha256")?;
    if !out.status.success() || out.stdout.len() != 32 {
        anyhow::bail!("openssl sha256");
    }
    Ok(out.stdout)
}

fn scratch_dir() -> anyhow::Result<PathBuf> {
    let mut b = [0u8; 8];
    File::open("/dev/urandom")
        .context("urandom")?
        .read_exact(&mut b)
        .context("urandom read")?;
    let p = std::env::temp_dir().join(format!("secd-upd-{}", hex::encode(b)));
    fs::create_dir(&p).with_context(|| format!("mkdir {}", p.display()))?;
    let mut perms = fs::metadata(&p)?.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&p, perms)?;
    Ok(p)
}

fn openssl_verify(key: &Path, sig: &Path, payload: &Path) -> bool {
    let openssl = "openssl";
    std::process::Command::new(openssl)
        .args([
            "dgst",
            "-sha256",
            "-verify",
            key.to_str().unwrap_or(""),
            "-signature",
            sig.to_str().unwrap_or(""),
            payload.to_str().unwrap_or(""),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn cosign_verify(key: &Path, sig: &Path, payload: &Path) -> bool {
    let cosign = "cosign";
    std::process::Command::new(cosign)
        .args([
            "verify-blob",
            "--key",
            key.to_str().unwrap_or(""),
            "--signature",
            sig.to_str().unwrap_or(""),
            "--insecure-ignore-tlog",
            "--insecure-ignore-sct",
            payload.to_str().unwrap_or(""),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn write_staging(path: &Path, payload: &[u8]) -> anyhow::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o755)
        .open(path)
        .with_context(|| format!("write {}", path.display()))?;
    f.write_all(payload)?;
    f.sync_all()?;
    Ok(())
}

fn fsync_path(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

fn eq_hex(got: &str, expected: &str) -> bool {
    let exp = expected
        .split_whitespace()
        .next()
        .unwrap_or(expected)
        .trim();
    got.eq_ignore_ascii_case(exp)
}

fn sig_candidates(signature: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    if let Ok(s) = std::str::from_utf8(signature) {
        if let Some(b) = b64(s.trim()) {
            out.push(b);
        }
    }
    out.push(signature.to_vec());
    out
}

fn b64(s: &str) -> Option<Vec<u8>> {
    let s: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if s.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut n = 0u32;
    for &c in &s {
        if c == b'=' {
            break;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((buf >> n) as u8);
            buf &= (1 << n) - 1;
        }
    }
    Some(out)
}

fn https_get(url: &str, cap: usize) -> anyhow::Result<Vec<u8>> {
    const MAX_HOPS: u8 = 5;
    let cfg = Arc::new(client_config()?);
    let mut url = url.to_string();
    let mut hops = 0u8;
    loop {
        if !url_allowed(&url) {
            anyhow::bail!("refusing host");
        }
        let (host, path) = split_https(&url)?;
        let name = ServerName::try_from(host.clone()).map_err(|_| anyhow!("server name"))?;
        let conn = ClientConnection::new(Arc::clone(&cfg), name).context("tls client")?;
        let tcp =
            TcpStream::connect((host.as_str(), 443)).with_context(|| format!("connect {host}"))?;
        tcp.set_nodelay(true)?;
        tcp.set_read_timeout(Some(Duration::from_secs(60)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(30)))?;
        let mut tls = StreamOwned::new(conn, tcp);
        let req = format!(
            "GET {path} HTTP/1.0\r\nHost: {host}\r\nAccept: */*\r\nConnection: close\r\n\r\n"
        );
        tls.write_all(req.as_bytes())?;
        tls.flush()?;
        let raw = read_limited(&mut tls, cap.saturating_add(4096))?;
        let (status, location, body) = parse_http(&raw)?;
        match next_redirect(status, location.as_deref(), &url)? {
            Some(next) => {
                hops = hops.saturating_add(1);
                if hops > MAX_HOPS {
                    anyhow::bail!("too many redirects");
                }
                url = next;
            }
            None => {
                if status != 200 {
                    anyhow::bail!("GET {host}{path} HTTP {status}");
                }
                if body.len() > cap {
                    anyhow::bail!("response too large");
                }
                return Ok(body);
            }
        }
    }
}

fn split_https(url: &str) -> anyhow::Result<(String, String)> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| anyhow!("refusing host"))?;
    if rest.contains('@') {
        anyhow::bail!("refusing host");
    }
    let (hostport, pathq) = rest.split_once('/').unwrap_or((rest, ""));
    let host = hostport.split(':').next().unwrap_or(hostport);
    if !host_allowed(host) {
        anyhow::bail!("refusing host");
    }
    Ok((host.to_string(), format!("/{pathq}")))
}

fn client_config() -> anyhow::Result<ClientConfig> {
    let roots = system_roots()?;
    let mut cfg =
        ClientConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .context("tls1.3")?
            .with_root_certificates(roots)
            .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(cfg)
}

fn system_roots() -> anyhow::Result<RootCertStore> {
    let mut roots = RootCertStore::empty();
    for path in [
        "/etc/ssl/certs/ca-certificates.crt",
        "/etc/ssl/cert.pem",
        "/etc/pki/tls/certs/ca-bundle.crt",
        "/etc/ssl/ca-bundle.pem",
    ] {
        let Ok(pem) = fs::read(path) else {
            continue;
        };
        let mut cur = Cursor::new(pem);
        for cert in rustls_pemfile::certs(&mut cur).flatten() {
            let _ = roots.add(cert);
        }
        if !roots.is_empty() {
            return Ok(roots);
        }
    }
    anyhow::bail!("no system CA")
}

fn read_limited(r: &mut impl Read, cap: usize) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        let n = r.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        if out.len().saturating_add(n) > cap {
            anyhow::bail!("response too large");
        }
        out.extend_from_slice(&tmp[..n]);
    }
    Ok(out)
}

fn parse_http(raw: &[u8]) -> anyhow::Result<(u16, Option<String>, Vec<u8>)> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n").map(|i| i + 2))
        .ok_or_else(|| anyhow!("bad http"))?;
    let head = std::str::from_utf8(&raw[..split.min(raw.len())]).context("http head")?;
    let status: u16 = head
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("status"))?
        .parse()
        .context("status parse")?;
    let location = head.lines().skip(1).find_map(|line| {
        let line = line.trim_end_matches('\r');
        let (k, v) = line.split_once(':')?;
        k.eq_ignore_ascii_case("location")
            .then(|| v.trim().to_string())
            .filter(|s| !s.is_empty())
    });
    Ok((status, location, raw[split..].to_vec()))
}
