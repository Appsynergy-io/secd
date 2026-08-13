//! Unlock gate, bundle resolution, `--with` collision, vault I/O, file leases.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use secd_core::{infer, providers, Secret};
use serde_json::{json, Value};
use zeroize::Zeroize;

use crate::login::{self, Unlocked};

pub const LOCKED: &str = "secd: locked — run secd";

const HOST: &str = "secd.imabee.com";
const IP: &str = "192.168.101.122";
const PORT: u16 = 443;
const ROOT_PEM: &str = include_str!("../keys/appsynergy-root.pem");
const INT_PEM: &str = include_str!("../keys/appsynergy-int.pem");
const RESP_CAP: usize = 2 * 1024 * 1024;

pub struct Entry {
    pub name: String,
    pub value: Secret,
    pub meta: Value,
}

pub struct Bundle {
    pub name: String,
    pub provider: String,
    pub fields: BTreeMap<String, String>,
}

impl Drop for Bundle {
    fn drop(&mut self) {
        for v in self.fields.values_mut() {
            v.zeroize();
        }
    }
}

pub struct WithSpec {
    pub provider: String,
    pub bundle: String,
}

pub enum GiteaPick<'a> {
    One(&'a Bundle),
    Zero,
    Many(Vec<String>),
}

pub struct Lease {
    path: PathBuf,
}

impl Lease {
    /// 0600 marker under `$XDG_RUNTIME_DIR/secd/`. No plaintext.
    pub fn create() -> Option<Self> {
        let root = std::env::var("XDG_RUNTIME_DIR")
            .ok()
            .filter(|s| !s.is_empty())?;
        let dir = PathBuf::from(root).join("secd");
        fs::create_dir_all(&dir).ok()?;
        if let Ok(meta) = fs::metadata(&dir) {
            let mut perms = meta.permissions();
            perms.set_mode(0o700);
            let _ = fs::set_permissions(&dir, perms);
        }
        let path = dir.join(format!("lease-{}", std::process::id()));
        write_0600(&path, b"secd\n").ok()?;
        Some(Self { path })
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn locked_err() -> anyhow::Error {
    eprintln!("{LOCKED}");
    anyhow!(LOCKED)
}

pub fn require_unlocked() -> anyhow::Result<Unlocked> {
    let token = login::load_session().ok_or_else(locked_err)?;
    let dek = crate::keyring::load().ok_or_else(locked_err)?;
    Ok(Unlocked { token, dek })
}

pub fn parse_with(raw: &str) -> anyhow::Result<WithSpec> {
    let Some((p, b)) = raw.split_once('=') else {
        anyhow::bail!("expected --with P=B");
    };
    if p.is_empty() || b.is_empty() {
        anyhow::bail!("expected --with P=B");
    }
    Ok(WithSpec {
        provider: p.to_string(),
        bundle: b.to_string(),
    })
}

pub fn check_with_collision(specs: &[WithSpec]) -> anyhow::Result<()> {
    let mut claimed: BTreeMap<&str, &str> = BTreeMap::new();
    for spec in specs {
        let schema = providers()
            .iter()
            .find(|p| p.name == spec.provider)
            .ok_or_else(|| anyhow!("unknown provider {}", spec.provider))?;
        for field in &schema.fields {
            if claimed
                .insert(field.env.as_str(), spec.provider.as_str())
                .is_some()
            {
                anyhow::bail!("secd: env collision: {}", field.env);
            }
        }
    }
    Ok(())
}

pub fn apply_with(
    specs: &[WithSpec],
    bundles: &[Bundle],
) -> anyhow::Result<BTreeMap<String, String>> {
    check_with_collision(specs)?;
    let mut env = BTreeMap::new();
    for spec in specs {
        let bundle = bundles
            .iter()
            .find(|b| b.name == spec.bundle)
            .ok_or_else(|| anyhow!("unknown bundle {}", spec.bundle))?;
        let schema = providers()
            .iter()
            .find(|p| p.name == spec.provider)
            .ok_or_else(|| anyhow!("unknown provider {}", spec.provider))?;
        for field in &schema.fields {
            let Some(val) = field_get(bundle, &field.key, &field.env) else {
                continue;
            };
            env.insert(field.env.clone(), val.to_string());
        }
    }
    Ok(env)
}

pub fn discover_bundles(entries: &[Entry]) -> Vec<Bundle> {
    let mut out = Vec::new();
    let mut used = HashSet::new();
    for e in entries {
        if let Some(b) = json_bundle(e) {
            used.insert(e.name.clone());
            out.push(b);
        }
    }
    let mut by_parent: BTreeMap<String, Vec<(&str, &Entry)>> = BTreeMap::new();
    for e in entries {
        if used.contains(&e.name) {
            continue;
        }
        let Some((parent, field)) = e.name.rsplit_once('/') else {
            continue;
        };
        by_parent
            .entry(parent.to_string())
            .or_default()
            .push((field, e));
    }
    for (parent, fields) in by_parent {
        let mut map = BTreeMap::new();
        let mut keys = Vec::new();
        for (k, e) in fields {
            if let Ok(s) = std::str::from_utf8(e.value.as_bytes()) {
                map.insert(k.to_string(), s.to_string());
                keys.push(k);
            }
        }
        if let Some(provider) = resolve_provider(&keys, None, &map) {
            out.push(Bundle {
                name: parent,
                provider,
                fields: map,
            });
        }
    }
    out
}

fn gitea_ready(bundles: &[Bundle]) -> Vec<&Bundle> {
    bundles
        .iter()
        .filter(|b| b.provider == "gitea" && gitea_token(b).is_some() && gitea_url(b).is_some())
        .collect()
}

pub fn pick_gitea<'a>(bundles: &'a [Bundle], want: Option<&str>) -> GiteaPick<'a> {
    let ready = gitea_ready(bundles);
    if let Some(name) = want {
        if let Some(b) = ready.iter().copied().find(|b| b.name == name) {
            return GiteaPick::One(b);
        }
        return GiteaPick::Many(ready.iter().map(|b| b.name.clone()).collect());
    }
    match ready.len() {
        0 => GiteaPick::Zero,
        1 => GiteaPick::One(ready[0]),
        _ => GiteaPick::Many(ready.iter().map(|b| b.name.clone()).collect()),
    }
}

pub fn gitea_env(bundle: &Bundle) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    if let Some(t) = gitea_token(bundle) {
        env.insert("GITEA_TOKEN".into(), t.to_string());
    }
    if let Some(u) = gitea_url(bundle) {
        env.insert("GITEA_URL".into(), u.to_string());
    }
    if let Some(user) = field_get(bundle, "user", "GITEA_USER") {
        env.insert("GITEA_USER".into(), user.to_string());
    }
    env
}

pub fn gitea_token(bundle: &Bundle) -> Option<&str> {
    field_get(bundle, "token", "GITEA_TOKEN")
}

pub fn gitea_url(bundle: &Bundle) -> Option<&str> {
    field_get(bundle, "url", "GITEA_URL")
}

pub fn origin_url(raw: &str) -> String {
    let raw = raw.trim().trim_end_matches('/');
    let (scheme, rest) = if let Some(r) = raw.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = raw.strip_prefix("http://") {
        ("http", r)
    } else {
        return format!("https://{raw}");
    };
    let host = rest.split('/').next().unwrap_or(rest);
    format!("{scheme}://{host}")
}

pub fn host_of_url(raw: &str) -> Option<String> {
    let origin = origin_url(raw);
    let host = origin.split("://").nth(1)?;
    let host = host.split('/').next().unwrap_or(host);
    Some(strip_default_port(host).to_string())
}

pub fn hosts_match(bundle_url: &str, requested: &str) -> bool {
    let Some(want) = host_of_url(bundle_url) else {
        return false;
    };
    let got = strip_default_port(requested.trim());
    want.eq_ignore_ascii_case(got)
}

pub fn load_entries(token: &str, dek: &Secret) -> anyhow::Result<Vec<Entry>> {
    let (status, v) = request("GET", "/api/v1/vault", None, Some(token))?;
    if status == 401 {
        return Err(locked_err());
    }
    if status != 200 {
        anyhow::bail!("vault {status}");
    }
    let Some(arr) = v.get("entries").and_then(Value::as_array) else {
        anyhow::bail!("vault: no entries");
    };
    let mut out = Vec::new();
    for e in arr {
        let Some(name) = e.get("name").and_then(Value::as_str) else {
            continue;
        };
        if secd_core::check_name(name).is_err() {
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
        out.push(Entry {
            name: name.to_string(),
            value: plain,
            meta,
        });
    }
    Ok(out)
}

pub fn save_entries(token: &str, dek: &Secret, entries: &[Entry]) -> anyhow::Result<()> {
    let mut body_entries = Vec::new();
    for e in entries {
        let blob = secd_core::seal(dek.as_bytes(), &e.name, e.value.as_bytes())
            .map_err(|err| anyhow!("seal {}: {err}", e.name))?;
        body_entries.push(json!({
            "name": e.name,
            "ciphertext": hex::encode(blob),
            "meta": e.meta,
        }));
    }
    let body = json!({ "entries": body_entries });
    let (status, v) = request("PUT", "/api/v1/vault", Some(&body), Some(token))?;
    if status == 401 {
        return Err(locked_err());
    }
    if status != 200 {
        anyhow::bail!("vault put {status}: {}", err_of(&v));
    }
    Ok(())
}

pub fn revoke_session(token: &str) {
    let _ = request("POST", "/api/v1/device/revoke", None, Some(token));
}

pub fn redact_values(entries: &[Entry]) -> Vec<String> {
    let mut out = Vec::new();
    for e in entries {
        if let Ok(s) = std::str::from_utf8(e.value.as_bytes()) {
            if let Ok(v) = serde_json::from_str::<Value>(s) {
                if let Some(obj) = v.as_object() {
                    for val in obj.values() {
                        if let Some(t) = val.as_str() {
                            if !t.is_empty() {
                                out.push(t.to_string());
                            }
                        }
                    }
                    continue;
                }
            }
            if !s.is_empty() {
                out.push(s.to_string());
            }
        }
    }
    out
}

pub fn env_values(env: &BTreeMap<String, String>) -> Vec<String> {
    env.values().filter(|v| !v.is_empty()).cloned().collect()
}

pub fn write_0600(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
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

fn field_get<'a>(bundle: &'a Bundle, key: &str, env: &str) -> Option<&'a str> {
    bundle
        .fields
        .get(key)
        .or_else(|| bundle.fields.get(env))
        .map(String::as_str)
}

fn json_bundle(e: &Entry) -> Option<Bundle> {
    let v: Value = serde_json::from_slice(e.value.as_bytes()).ok()?;
    let obj = v.as_object()?;
    if obj.is_empty() {
        return None;
    }
    let mut fields = BTreeMap::new();
    let mut keys = Vec::new();
    for (k, val) in obj {
        let Some(s) = val.as_str() else {
            continue;
        };
        keys.push(k.as_str());
        fields.insert(k.clone(), s.to_string());
    }
    if fields.is_empty() {
        return None;
    }
    let meta_p = e.meta.get("provider").and_then(Value::as_str);
    let provider = resolve_provider(&keys, meta_p, &fields)?;
    Some(Bundle {
        name: e.name.clone(),
        provider,
        fields,
    })
}

fn resolve_provider(
    keys: &[&str],
    meta: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(p) = meta {
        if providers().iter().any(|x| x.name == p) {
            return Some(p.to_string());
        }
    }
    if let Some(p) = infer(keys) {
        return Some(p.to_string());
    }
    let env_keys: Vec<&str> = keys
        .iter()
        .copied()
        .map(|k| k.strip_prefix("GITEA_").unwrap_or(k))
        .collect();
    let lower: Vec<String> = env_keys.iter().map(|k| k.to_ascii_lowercase()).collect();
    let lower_ref: Vec<&str> = lower.iter().map(String::as_str).collect();
    if let Some(p) = infer(&lower_ref) {
        return Some(p.to_string());
    }
    if looks_gitea(fields) {
        return Some("gitea".into());
    }
    None
}

fn looks_gitea(fields: &BTreeMap<String, String>) -> bool {
    let token = fields.contains_key("token") || fields.contains_key("GITEA_TOKEN");
    let url = fields.contains_key("url") || fields.contains_key("GITEA_URL");
    token && url
}

fn strip_default_port(host: &str) -> &str {
    host.strip_suffix(":443")
        .or_else(|| host.strip_suffix(":80"))
        .unwrap_or(host)
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
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nAccept: application/json\r\nConnection: close\r\n"
    );
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
