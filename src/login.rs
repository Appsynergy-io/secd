use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use secd_core::Secret;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

const HOST: &str = "secd.imabee.com";
const IP: &str = "192.168.101.122";
const PORT: u16 = 443;
const ROOT_PEM: &str = include_str!("../keys/appsynergy-root.pem");
const INT_PEM: &str = include_str!("../keys/appsynergy-int.pem");
const AAD_DEK: &str = "dek";
const RESP_CAP: usize = 2 * 1024 * 1024;

/// Device `{id,name}` persisted under the client home.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceFile {
    pub id: String,
    pub name: String,
}

pub struct DeviceFlow {
    secret: StaticSecret,
    pub user_code: String,
    pub interval: u64,
    pub verification_uri: String,
    pub open_url: String,
}

pub enum Poll {
    Pending,
    Expired,
    Ready { token: String, sealed: Value },
}

pub struct Unlocked {
    pub token: String,
    pub dek: Secret,
}

pub fn run() -> anyhow::Result<()> {
    let _ = unlock()?;
    Ok(())
}

pub fn unlock() -> anyhow::Result<Unlocked> {
    let flow = start()?;
    open_browser(&flow.open_url);
    loop {
        match poll_once(&flow)? {
            Poll::Pending => {
                std::thread::sleep(Duration::from_secs(flow.interval.max(1)));
            }
            Poll::Expired => anyhow::bail!("device pending expired"),
            Poll::Ready { token, sealed } => return finish(flow, token, sealed),
        }
    }
}

pub fn home() -> PathBuf {
    if let Ok(p) = std::env::var("SECD_HOME") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(p) = std::env::var("XDG_DATA_HOME") {
        if !p.is_empty() {
            return PathBuf::from(p).join("secd");
        }
    }
    let base = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(base).join(".local/share/secd")
}

pub fn session_path() -> PathBuf {
    home().join("login.session")
}

pub fn device_path() -> PathBuf {
    home().join("login.device")
}

pub fn start() -> anyhow::Result<DeviceFlow> {
    let secret = StaticSecret::random();
    let public = PublicKey::from(&secret);
    let eph_pub = hex::encode(public.as_bytes());
    let device = load_or_create_device()?;
    let body = json!({
        "eph_pub": eph_pub,
        "device_id": device.id,
        "hostname": device.name,
    });
    let (status, v) = request("POST", "/api/v1/device/start", Some(&body), None)?;
    if status != 200 {
        anyhow::bail!("device start {status}: {}", err_of(&v));
    }
    let user_code = v
        .get("user_code")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("device start: no user_code"))?
        .to_string();
    let interval = v.get("interval").and_then(Value::as_u64).unwrap_or(5);
    let verification_uri = v
        .get("verification_uri")
        .and_then(Value::as_str)
        .unwrap_or("https://secd.imabee.com")
        .to_string();
    let open_url = complete_uri(&verification_uri, &user_code, &eph_pub);
    Ok(DeviceFlow {
        secret,
        user_code,
        interval,
        verification_uri,
        open_url,
    })
}

pub fn poll_once(flow: &DeviceFlow) -> anyhow::Result<Poll> {
    let body = json!({ "user_code": flow.user_code });
    let (status, v) = request("POST", "/api/v1/device/poll", Some(&body), None)?;
    if status == 404 {
        return Ok(Poll::Expired);
    }
    if status != 200 {
        anyhow::bail!("device poll {status}: {}", err_of(&v));
    }
    match v.get("status").and_then(Value::as_str) {
        Some("pending") => Ok(Poll::Pending),
        Some("expired") | Some("denied") => Ok(Poll::Expired),
        Some("ok") => {
            let token = v
                .get("token")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("device poll: no token"))?
                .to_string();
            let sealed = v
                .get("sealed_dek")
                .cloned()
                .ok_or_else(|| anyhow!("device poll: no sealed_dek"))?;
            Ok(Poll::Ready { token, sealed })
        }
        _ => Ok(Poll::Pending),
    }
}

/// `sealed_dek` is `{eph_pub: hex, blob: hex}` — X25519 then XChaCha AAD `dek`.
pub fn finish(flow: DeviceFlow, token: String, sealed: Value) -> anyhow::Result<Unlocked> {
    let dek = unseal(&flow.secret, &sealed)?;
    drop(flow);
    save_session(&token)?;
    Ok(Unlocked { token, dek })
}

pub fn load_snapshot(token: &str, dek: &Secret) -> Vec<(String, Secret, Value)> {
    let Ok((200, v)) = request("GET", "/api/v1/vault", None, Some(token)) else {
        return Vec::new();
    };
    let Some(entries) = v.get("entries").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries {
        let Some(name) = e.get("name").and_then(Value::as_str) else {
            continue;
        };
        if check_name_ok(name).is_err() {
            continue;
        }
        let Some(ct) = e.get("ciphertext").and_then(Value::as_str) else {
            continue;
        };
        let Ok(blob) = hex::decode(ct) else {
            continue;
        };
        let Ok(plain) = secd_core::open(dek.as_bytes(), name, &blob) else {
            continue;
        };
        let meta = e.get("meta").cloned().unwrap_or_else(|| json!({}));
        out.push((name.to_string(), plain, meta));
    }
    out
}

pub fn save_snapshot(
    token: &str,
    dek: &Secret,
    names: &[String],
    values: &HashMap<String, Secret>,
    meta: &HashMap<String, Value>,
) -> anyhow::Result<()> {
    let mut entries = Vec::new();
    for name in names {
        let Some(secret) = values.get(name) else {
            continue;
        };
        let blob = secd_core::seal(dek.as_bytes(), name, secret.as_bytes())
            .map_err(|e| anyhow!("seal {name}: {e}"))?;
        let m = meta.get(name).cloned().unwrap_or_else(|| json!({}));
        entries.push(json!({
            "name": name,
            "ciphertext": hex::encode(blob),
            "meta": m,
        }));
    }
    let body = json!({ "entries": entries });
    let (status, v) = request("PUT", "/api/v1/vault", Some(&body), Some(token))?;
    if status != 200 {
        anyhow::bail!("snapshot put {status}: {}", err_of(&v));
    }
    Ok(())
}

pub fn save_session(token: &str) -> anyhow::Result<()> {
    let dir = home();
    fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
    write_0600(&session_path(), token.as_bytes())
}

pub fn load_session() -> Option<String> {
    let raw = fs::read_to_string(session_path()).ok()?;
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn open_browser(url: &str) {
    for bin in ["xdg-open", "gio"] {
        let mut cmd = Command::new(bin);
        if bin == "gio" {
            cmd.arg("open");
        }
        let ok = cmd
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok();
        if ok {
            return;
        }
    }
}

pub fn clipboard_set(bytes: &[u8]) {
    if spawn_write("wl-copy", &[], bytes) {
        return;
    }
    if spawn_write("xclip", &["-selection", "clipboard"], bytes) {
        return;
    }
    let _ = spawn_write("xsel", &["-ib"], bytes);
}

pub fn clipboard_clear() {
    let clear = "wl-copy";
    let _ = Command::new(clear)
        .arg("--clear")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    clipboard_set(b"");
}

fn spawn_write(bin: &str, args: &[&str], bytes: &[u8]) -> bool {
    let mut child = match Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(bytes);
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

fn load_or_create_device() -> anyhow::Result<DeviceFile> {
    let path = device_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(d) = serde_json::from_str::<DeviceFile>(&raw) {
            if !d.id.is_empty() && !d.name.is_empty() {
                return Ok(d);
            }
        }
    }
    let id = random_hex(16)?;
    let name = hostname();
    let d = DeviceFile { id, name };
    fs::create_dir_all(home())?;
    fs::write(&path, serde_json::to_vec(&d)?)?;
    Ok(d)
}

fn hostname() -> String {
    if let Ok(s) = fs::read_to_string("/etc/hostname") {
        let t = s.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    std::env::var("HOSTNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "secd".into())
}

fn random_hex(n: usize) -> anyhow::Result<String> {
    let mut f = fs::File::open("/dev/urandom").context("urandom")?;
    let mut b = vec![0u8; n];
    f.read_exact(&mut b).context("urandom read")?;
    Ok(hex::encode(b))
}

fn complete_uri(base: &str, code: &str, eph: &str) -> String {
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}code={code}&eph={eph}")
}

fn unseal(sk: &StaticSecret, sealed: &Value) -> anyhow::Result<Secret> {
    let eph = sealed
        .get("eph_pub")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("sealed_dek missing eph_pub"))?;
    let mut eph_b = hex::decode(eph).context("sealed eph_pub")?;
    if eph_b.len() != 32 {
        eph_b.zeroize();
        anyhow::bail!("sealed eph_pub must be 32 bytes");
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&eph_b);
    eph_b.zeroize();
    let their = PublicKey::from(pk);
    let shared = sk.diffie_hellman(&their);
    let blob = blob_of(sealed)?;
    secd_core::open(shared.as_bytes(), AAD_DEK, &blob).map_err(|e| anyhow!("unseal dek: {e}"))
}

fn blob_of(sealed: &Value) -> anyhow::Result<Vec<u8>> {
    if let Some(b) = sealed.get("blob").and_then(Value::as_str) {
        return hex::decode(b).context("sealed blob");
    }
    let nonce = sealed
        .get("nonce")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("sealed_dek missing blob"))?;
    let ct = sealed
        .get("ct")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("sealed_dek missing blob"))?;
    let mut out = hex::decode(nonce).context("sealed nonce")?;
    out.extend(hex::decode(ct).context("sealed ct")?);
    Ok(out)
}

fn write_0600(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("write {}", path.display()))?;
    f.write_all(bytes)?;
    f.flush()?;
    let mut perms = f.metadata()?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

fn client_config() -> anyhow::Result<ClientConfig> {
    let mut roots = RootCertStore::empty();
    for pem in [ROOT_PEM, INT_PEM] {
        let mut cur = std::io::Cursor::new(pem.as_bytes());
        for cert in rustls_pemfile::certs(&mut cur) {
            let cert = cert.context("ca pem")?;
            let _ = roots.add(cert);
        }
    }
    if roots.is_empty() {
        anyhow::bail!("no AppSynergy CA");
    }
    let mut cfg =
        ClientConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .context("tls1.3")?
            .with_root_certificates(roots)
            .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(cfg)
}

fn connect() -> anyhow::Result<StreamOwned<ClientConnection, TcpStream>> {
    let cfg = Arc::new(client_config()?);
    let name = ServerName::try_from(HOST)
        .map_err(|_| anyhow!("server name"))?
        .to_owned();
    let conn = ClientConnection::new(cfg, name).context("tls client")?;
    let tcp = TcpStream::connect((HOST, PORT))
        .or_else(|_| TcpStream::connect((IP, PORT)))
        .with_context(|| format!("connect {HOST}:{PORT}"))?;
    tcp.set_nodelay(true)?;
    tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(30)))?;
    Ok(StreamOwned::new(conn, tcp))
}

fn request(
    method: &str,
    path: &str,
    body: Option<&Value>,
    bearer: Option<&str>,
) -> anyhow::Result<(u16, Value)> {
    let payload = match body {
        Some(v) => serde_json::to_vec(v)?,
        None => Vec::new(),
    };
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nAccept: application/json\r\nConnection: close\r\n");
    if let Some(t) = bearer {
        head.push_str("Authorization: Bearer ");
        head.push_str(t);
        head.push_str("\r\n");
    }
    if method == "POST" || method == "PUT" {
        head.push_str("Content-Type: application/json\r\n");
        head.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    head.push_str("\r\n");
    let mut tls = connect()?;
    tls.write_all(head.as_bytes())?;
    if !payload.is_empty() {
        tls.write_all(&payload)?;
    }
    tls.flush()?;
    let raw = read_limited(&mut tls, RESP_CAP)?;
    parse_http(&raw)
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

fn parse_http(raw: &[u8]) -> anyhow::Result<(u16, Value)> {
    let s = std::str::from_utf8(raw).context("response utf8")?;
    let (head, body) = s
        .split_once("\r\n\r\n")
        .or_else(|| s.split_once("\n\n"))
        .ok_or_else(|| anyhow!("bad http"))?;
    let status_line = head.lines().next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("status"))?
        .parse()
        .context("status parse")?;
    let val = if body.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(body).unwrap_or(Value::Null)
    };
    Ok((status, val))
}

fn err_of(v: &Value) -> String {
    v.get("error")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn check_name_ok(name: &str) -> Result<(), secd_core::NameError> {
    secd_core::check_name(name)
}
