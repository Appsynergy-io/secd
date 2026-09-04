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

impl Entry {
    /// This entry as a save borrows it: nothing is copied, least of all the value.
    pub fn row(&self) -> Row<'_> {
        (&self.name, &self.value, &self.meta)
    }
}

/// One entry of a save, borrowed from wherever the caller keeps it.
pub type Row<'a> = (&'a str, &'a Secret, &'a Value);

/// One `GET /api/v1/vault`: what decoded, how many entries the server sent,
/// the body that puts them all back, and the ciphertext they came as.
///
/// `entries` silently drops anything this DEK cannot open, and
/// `PUT /api/v1/vault` replaces the whole vault, so a caller that loads,
/// mutates and saves must refuse on `drop_refusal` first: every entry that did
/// not decode is an entry the save deletes.
pub struct VaultLoad {
    pub entries: Vec<Entry>,
    /// Entries in the response, decoded or not.
    pub raw: usize,
    /// Every entry the server sent, as a `PUT /api/v1/vault` body: the response
    /// carries a `version` per entry and the route rejects any key but `name`,
    /// `ciphertext` and `meta`, so this is the projection, not the response.
    /// Ciphertext, never plaintext.
    pub body: String,
    /// name -> ciphertext as loaded: what a save checks the vault against.
    pub before: BTreeMap<String, String>,
}

impl VaultLoad {
    /// Entries the server sent that this DEK could not open.
    pub fn dropped(&self) -> usize {
        self.raw.saturating_sub(self.entries.len())
    }

    /// What a caller that saves must refuse with, when anything did not decode.
    pub fn drop_refusal(&self) -> Option<String> {
        let dropped = self.dropped();
        (dropped > 0).then(|| {
            format!(
                "vault: {dropped} of {} entries did not decode; a save would delete them",
                self.raw
            )
        })
    }
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

/// Whether the server still honours this session, without the refusal
/// message: the caller offers sign-in itself. A server it cannot reach is not
/// a signed-out session, so that answers `true` and the caller's own load
/// reports the failure.
pub fn session_live(token: &str) -> bool {
    !matches!(
        request("GET", "/api/v1/vault", None, Some(token)),
        Ok((401, _))
    )
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
    let rows: Vec<Row<'_>> = entries.iter().map(Entry::row).collect();
    bundles_of(&rows)
}

/// `discover_bundles` over borrowed pieces. The register keeps names, values
/// and meta in three maps, and a `Secret` does not clone, so grouping what it
/// holds has to go through the borrow rather than through an `Entry`.
pub fn bundles_of(rows: &[Row<'_>]) -> Vec<Bundle> {
    let mut out = Vec::new();
    let mut used = HashSet::new();
    for (name, value, meta) in rows {
        if let Some(b) = json_bundle(name, value, meta) {
            used.insert((*name).to_string());
            out.push(b);
        }
    }
    let mut by_parent: BTreeMap<String, Vec<(&str, &Secret)>> = BTreeMap::new();
    for (name, value, _) in rows {
        if used.contains(*name) {
            continue;
        }
        let Some((parent, field)) = name.rsplit_once('/') else {
            continue;
        };
        by_parent
            .entry(parent.to_string())
            .or_default()
            .push((field, value));
    }
    for (parent, fields) in by_parent {
        let mut map = BTreeMap::new();
        let mut keys = Vec::new();
        for (k, v) in fields {
            if let Ok(s) = std::str::from_utf8(v.as_bytes()) {
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

/// What `discover_bundles` found, without the values: which entries stand for
/// one credential, and under what name. A JSON bundle is its own single
/// member; siblings under a shared parent are the parent's members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleShape {
    pub name: String,
    /// `None` when the keys are a credential shape that names no single
    /// provider -- `{token, user}` is both `github` and `gitea`.
    pub provider: Option<String>,
    pub members: Vec<String>,
}

/// The grouping `secd run` already uses, in the shape a list can draw. A
/// register that shows `prod/github/token` and `prod/github/user` as two
/// unrelated rows is showing storage where the human sees one credential.
pub fn bundle_shapes(entries: &[Entry]) -> Vec<BundleShape> {
    let rows: Vec<Row<'_>> = entries.iter().map(Entry::row).collect();
    shapes_of(&rows)
}

/// The same walk as `bundles_of`, with one difference: siblings group when
/// their keys are *a* credential shape, not only when they name one provider.
/// `secd run` has to name a provider to map env vars, so `bundles_of` refuses
/// what it cannot name; a list only has to know that six keys are one thing.
pub fn shapes_of(rows: &[Row<'_>]) -> Vec<BundleShape> {
    let mut out = Vec::new();
    let mut used = HashSet::new();
    for (name, value, meta) in rows {
        if let Some(b) = json_bundle(name, value, meta) {
            used.insert((*name).to_string());
            out.push(BundleShape {
                name: (*name).to_string(),
                provider: Some(b.provider.clone()),
                members: vec![(*name).to_string()],
            });
        }
    }
    let mut by_parent: BTreeMap<String, Vec<(&str, &Secret)>> = BTreeMap::new();
    for (name, value, _) in rows {
        if used.contains(*name) {
            continue;
        }
        let Some((parent, field)) = name.rsplit_once('/') else {
            continue;
        };
        by_parent
            .entry(parent.to_string())
            .or_default()
            .push((field, value));
    }
    for (parent, fields) in by_parent {
        let mut map = BTreeMap::new();
        let mut keys = Vec::new();
        for (k, v) in fields {
            if let Ok(s) = std::str::from_utf8(v.as_bytes()) {
                map.insert(k.to_string(), s.to_string());
                keys.push(k);
            }
        }
        // One key is its own row already, and a group of one is not a group.
        if keys.len() < 2 {
            continue;
        }
        let provider = resolve_provider(&keys, None, &map);
        if provider.is_none() && secd_core::candidates(&keys).is_empty() {
            continue;
        }
        out.push(BundleShape {
            members: keys.iter().map(|k| format!("{parent}/{k}")).collect(),
            name: parent,
            provider,
        });
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

/// A provider that can authenticate git over HTTPS: a token, a username, and a
/// host that the bundle either states or inherits from the service.
struct Forge {
    provider: &'static str,
    token: (&'static str, &'static str),
    url: (&'static str, &'static str),
    user: (&'static str, &'static str),
    /// The host when the bundle names no url. A self-hosted Gitea has no such
    /// default, so a Gitea bundle without a url serves no host at all.
    host: Option<&'static str>,
}

const FORGES: [Forge; 3] = [
    Forge {
        provider: "gitea",
        token: ("token", "GITEA_TOKEN"),
        url: ("url", "GITEA_URL"),
        user: ("user", "GITEA_USER"),
        host: None,
    },
    Forge {
        provider: "github",
        token: ("token", "GITHUB_TOKEN"),
        url: ("url", "GITHUB_URL"),
        user: ("user", "GITHUB_USER"),
        host: Some("github.com"),
    },
    Forge {
        provider: "gitlab",
        token: ("token", "GITLAB_TOKEN"),
        url: ("url", "GITLAB_URL"),
        user: ("user", "GITLAB_USER"),
        host: Some("gitlab.com"),
    },
];

fn forge(provider: &str) -> Option<&'static Forge> {
    FORGES.iter().find(|f| f.provider == provider)
}

/// The host this bundle can serve credentials for, if any.
pub fn forge_host(bundle: &Bundle) -> Option<String> {
    let f = forge(&bundle.provider)?;
    match field_get(bundle, f.url.0, f.url.1) {
        Some(url) => host_of_url(url),
        None => f.host.map(str::to_string),
    }
}

/// The remote origin this bundle serves, as git scopes a config key. Built
/// from the bundle's own url where it has one, so a self-hosted forge on plain
/// http keeps its scheme.
pub fn forge_origin(bundle: &Bundle) -> Option<String> {
    let f = forge(&bundle.provider)?;
    match field_get(bundle, f.url.0, f.url.1) {
        Some(url) => Some(origin_url(url)),
        None => f.host.map(|h| format!("https://{h}")),
    }
}

pub fn forge_token(bundle: &Bundle) -> Option<&str> {
    let f = forge(&bundle.provider)?;
    field_get(bundle, f.token.0, f.token.1)
}

/// The username git should be handed. `git` is what every forge accepts when
/// the token is the password and the bundle named nobody.
pub fn forge_user(bundle: &Bundle) -> &str {
    forge(&bundle.provider)
        .and_then(|f| field_get(bundle, f.user.0, f.user.1))
        .unwrap_or("git")
}

/// The bundle that serves `host`, or none. Unlike `pick_gitea`, which answers
/// "which bundle did the human mean", this is answering a host git already
/// named, so an ambiguous answer is a refusal rather than a menu: two bundles
/// for one host cannot be disambiguated by a helper git invokes silently.
pub fn pick_forge<'a>(bundles: &'a [Bundle], want: Option<&str>, host: &str) -> Option<&'a Bundle> {
    let host = strip_default_port(host.trim());
    let mut ready = bundles.iter().filter(|b| {
        forge_token(b).is_some() && forge_host(b).is_some_and(|h| h.eq_ignore_ascii_case(host))
    });
    match want {
        Some(name) => ready.find(|b| b.name == name),
        None => {
            let first = ready.next()?;
            ready.next().is_none().then_some(first)
        }
    }
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
    Ok(load_vault(token, dek)?.entries)
}

/// `load_entries` plus what it had to leave behind, for a caller that saves.
pub fn load_vault(token: &str, dek: &Secret) -> anyhow::Result<VaultLoad> {
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
    let raw = arr.len();
    let mut before = BTreeMap::new();
    let mut put_entries = Vec::with_capacity(raw);
    let mut out = Vec::new();
    for e in arr {
        let Some(name) = e.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(ct) = e.get("ciphertext").and_then(Value::as_str) else {
            continue;
        };
        let meta = e.get("meta").cloned().unwrap_or_else(|| json!({}));
        // The pre-image carries every entry the server sent, decoded or not:
        // it is what restores the vault a save is about to replace.
        put_entries.push(json!({ "name": name, "ciphertext": ct, "meta": meta }));
        before.insert(name.to_string(), ct.to_string());
        if secd_core::check_name(name).is_err() {
            continue;
        }
        let Ok(blob) = hex::decode(ct) else {
            continue;
        };
        let Ok(plain) = secd_core::open(dek.as_bytes(), name, &blob) else {
            continue;
        };
        out.push(Entry {
            name: name.to_string(),
            value: plain,
            meta,
        });
    }
    Ok(VaultLoad {
        entries: out,
        raw,
        body: json!({ "entries": put_entries }).to_string(),
        before,
    })
}

/// Check the vault still holds `before`, save, then read it back and compare
/// what the server returns against what was sent. Returns the vault as written,
/// which is `before` for this caller's next save.
///
/// `PUT /api/v1/vault` replaces the whole vault and carries no version, so a
/// save landing between the check and the write, or between the write and the
/// read, wins and cannot be stopped here. The check narrows that window to one
/// round trip, and the read-back turns a clobber from silent into an error.
pub fn save_entries_read_back(
    token: &str,
    dek: &Secret,
    rows: &[Row<'_>],
    before: &BTreeMap<String, String>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let now = load_ciphertexts(token)?;
    if &now != before {
        return Err(changed_under_write("loaded", before, &now));
    }
    let sent = put_vault(token, dek, rows)?;
    let back = load_ciphertexts(token)
        .context("vault written; read-back failed, the vault now holds what was sent")?;
    if back != sent {
        return Err(changed_under_write("sent", &sent, &back));
    }
    Ok(sent)
}

/// One sentence for a vault that is not what this write assumed it was.
fn changed_under_write(
    verb: &str,
    mine: &BTreeMap<String, String>,
    theirs: &BTreeMap<String, String>,
) -> anyhow::Error {
    let mut diff: Vec<&str> = mine
        .iter()
        .filter(|(name, ct)| theirs.get(name.as_str()) != Some(ct))
        .map(|(name, _)| name.as_str())
        .collect();
    diff.extend(
        theirs
            .keys()
            .filter(|name| !mine.contains_key(name.as_str()))
            .map(String::as_str),
    );
    diff.sort_unstable();
    diff.dedup();
    diff.truncate(8);
    anyhow!(
        "vault changed under the write: {verb} {}, found {} ({})",
        mine.len(),
        theirs.len(),
        diff.join(", ")
    )
}

/// PUT the whole vault. Returns name -> ciphertext, exactly as sent.
fn put_vault(
    token: &str,
    dek: &Secret,
    rows: &[Row<'_>],
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut sent: BTreeMap<String, String> = BTreeMap::new();
    let mut body_entries = Vec::with_capacity(rows.len());
    for (name, value, meta) in rows {
        let blob = secd_core::seal(dek.as_bytes(), name, value.as_bytes())
            .map_err(|err| anyhow!("seal {name}: {err}"))?;
        let ct = hex::encode(blob);
        body_entries.push(json!({
            "name": name,
            "ciphertext": ct,
            "meta": meta,
        }));
        if sent.insert((*name).to_string(), ct).is_some() {
            anyhow::bail!("duplicate name {name}");
        }
    }
    let body = json!({ "entries": body_entries });
    let (status, v) = request("PUT", "/api/v1/vault", Some(&body), Some(token))?;
    if status == 401 {
        return Err(locked_err());
    }
    if status != 200 {
        anyhow::bail!("vault put {status}: {}", err_of(&v));
    }
    Ok(sent)
}

/// The vault as name -> ciphertext, without opening anything.
fn load_ciphertexts(token: &str) -> anyhow::Result<BTreeMap<String, String>> {
    let (status, v) = request("GET", "/api/v1/vault", None, Some(token))?;
    if status != 200 {
        anyhow::bail!("vault {status}");
    }
    let Some(arr) = v.get("entries").and_then(Value::as_array) else {
        anyhow::bail!("vault: no entries");
    };
    let mut out = BTreeMap::new();
    for e in arr {
        let Some(name) = e.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(ct) = e.get("ciphertext").and_then(Value::as_str) else {
            continue;
        };
        out.insert(name.to_string(), ct.to_string());
    }
    if out.len() != arr.len() {
        anyhow::bail!(
            "vault read-back: {} entries, {} usable",
            arr.len(),
            out.len()
        );
    }
    Ok(out)
}

/// A provider schema as the register offers it: built-in and custom, one type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Schema {
    pub name: String,
    pub title: String,
    pub builtin: bool,
    pub fields: Vec<secd_core::Field>,
}

/// The locked built-ins, with no server. What the register starts from.
pub fn builtin_schemas() -> Vec<Schema> {
    providers()
        .iter()
        .map(|p| Schema {
            name: p.name.clone(),
            title: p.title.clone(),
            builtin: true,
            fields: p.fields.clone(),
        })
        .collect()
}

/// Every schema the register offers, and how many rows did not parse. The
/// count rides along so a caller says so rather than showing a short list as
/// though it were the whole one.
pub fn fetch_schemas(token: &str) -> anyhow::Result<(Vec<Schema>, usize)> {
    let (status, v) = request("GET", "/api/v1/providers", None, Some(token))?;
    if status != 200 {
        anyhow::bail!("providers {status}");
    }
    let Some(arr) = v.get("providers").and_then(Value::as_array) else {
        anyhow::bail!("providers: no list");
    };
    let mut out = Vec::with_capacity(arr.len());
    for p in arr {
        if let Some(schema) = parse_schema(p) {
            out.push(schema);
        }
    }
    let dropped = arr.len().saturating_sub(out.len());
    Ok((out, dropped))
}

/// A row of `GET /api/v1/providers`. An unreadable flag masks rather than
/// exposes, and blocks nothing: absent `secret` reads as secret, absent
/// `optional` as optional.
fn parse_schema(v: &Value) -> Option<Schema> {
    let name = v.get("name").and_then(Value::as_str)?;
    let title = v.get("title").and_then(Value::as_str)?;
    let builtin = v.get("builtin").and_then(Value::as_bool).unwrap_or(false);
    let raw = v.get("fields").and_then(Value::as_array)?;
    let mut fields = Vec::with_capacity(raw.len());
    for f in raw {
        fields.push(secd_core::Field {
            key: f.get("key").and_then(Value::as_str)?.to_string(),
            secret: f.get("secret").and_then(Value::as_bool).unwrap_or(true),
            optional: f.get("optional").and_then(Value::as_bool).unwrap_or(true),
            env: f.get("env").and_then(Value::as_str)?.to_string(),
        });
    }
    Some(Schema {
        name: name.to_string(),
        title: title.to_string(),
        builtin,
        fields,
    })
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

/// `{"k":"v",...}` in the order given: the shape the console seals, which
/// `json_bundle` reads back. Hand-written because `serde_json::Map` sorts,
/// and the order is what the console and `secd info` print.
pub fn payload_json(pairs: &[(String, String)]) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, k);
        out.push(':');
        push_json_str(&mut out, v);
    }
    out.push('}');
    out
}

/// The meta the console writes, and `secd info` and `resolve_provider` read.
pub fn provider_meta(provider: &str, keys: &[&str]) -> Value {
    json!({ "provider": provider, "fields": keys })
}

fn push_json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_bundle(name: &str, value: &Secret, meta: &Value) -> Option<Bundle> {
    let v: Value = serde_json::from_slice(value.as_bytes()).ok()?;
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
    let meta_p = meta.get("provider").and_then(Value::as_str);
    let provider = resolve_provider(&keys, meta_p, &fields)?;
    Some(Bundle {
        name: name.to_string(),
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
