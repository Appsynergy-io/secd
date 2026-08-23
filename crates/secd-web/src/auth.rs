use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::IpAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant as StdInstant};

use anyhow::Context;
use rand::RngCore;
use secd_core::{unwrap_password, wrap_password, Factor, Wrap};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::Instant;
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::{
    DiscoverableAuthentication, Passkey, PasskeyAuthentication, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, Webauthn, WebauthnBuilder,
};

const HANDLE_TTL: Duration = Duration::from_secs(5 * 60);
const EMAIL_MAX: usize = 254;
const PW_MIN: usize = 12;
const PW_MAX: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredWrap {
    pub factor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cred_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,
    pub blob: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredPasskey {
    pub id: String,
    pub created: String,
    pub passkey: Passkey,
    pub wrap: StoredWrap,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(default)]
    pub password: Option<StoredWrap>,
    #[serde(default)]
    pub passkeys: Vec<StoredPasskey>,
}

impl User {
    pub fn has_passkey(&self) -> bool {
        !self.passkeys.is_empty()
    }

    pub fn has_password(&self) -> bool {
        self.password.is_some()
    }

    pub fn method(&self) -> &'static str {
        match (self.has_passkey(), self.has_password()) {
            (true, true) => "either",
            (true, false) => "passkey",
            (false, true) => "password",
            (false, false) => "register",
        }
    }

    pub fn wraps(&self) -> Vec<Wrap> {
        let mut out = Vec::new();
        if let Some(w) = &self.password {
            out.push(from_stored(w));
        }
        for pk in &self.passkeys {
            out.push(from_stored(&pk.wrap));
        }
        out
    }

    pub fn factor_count(&self) -> usize {
        usize::from(self.has_password()) + self.passkeys.len()
    }

    pub fn passkeys_json(&self) -> Value {
        let rows: Vec<Value> = self
            .passkeys
            .iter()
            .map(|p| json!({ "id": p.id, "created": p.created }))
            .collect();
        json!({ "passkeys": rows })
    }
}

#[derive(Serialize, Deserialize)]
struct UsersFile {
    users: Vec<User>,
}

#[derive(Clone)]
pub struct UserStore {
    path: PathBuf,
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    users: HashMap<String, User>,
    dummy: Wrap,
}

impl UserStore {
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join("users.json");
        let users = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            let file: UsersFile = serde_json::from_str(&raw).unwrap_or(UsersFile { users: vec![] });
            file.users
                .into_iter()
                .map(|u| (u.email.clone(), u))
                .collect()
        } else {
            HashMap::new()
        };
        let dummy = wrap_password(&[0x5c; 32], b"secd-dummy-argon2-timing")
            .expect("invariant: dummy argon2 wrap");
        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(Inner { users, dummy })),
        })
    }

    pub fn is_empty(&self) -> bool {
        lock(&self.inner).users.is_empty()
    }

    pub fn get(&self, email: &str) -> Option<User> {
        lock(&self.inner).users.get(email).cloned()
    }

    pub fn put(&self, user: User) -> anyhow::Result<()> {
        let mut g = lock(&self.inner);
        g.users.insert(user.email.clone(), user);
        persist(&self.path, &g.users)
    }

    pub fn dummy_argon2(&self, password: &[u8]) {
        let dummy = lock(&self.inner).dummy.clone();
        let _ = unwrap_password(&dummy, password);
    }

    pub fn verify_password(&self, user: &User, password: &[u8]) -> bool {
        let Some(w) = &user.password else {
            self.dummy_argon2(password);
            return false;
        };
        unwrap_password(&from_stored(w), password).is_ok()
    }

    pub fn passkeys(&self) -> Vec<(String, StoredPasskey)> {
        lock(&self.inner)
            .users
            .values()
            .flat_map(|u| {
                u.passkeys
                    .iter()
                    .map(|p| (u.email.clone(), p.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn by_passkey_id(&self, id: &str) -> Option<(User, usize)> {
        let g = lock(&self.inner);
        for u in g.users.values() {
            if let Some(i) = u.passkeys.iter().position(|p| p.id == id) {
                return Some((u.clone(), i));
            }
        }
        None
    }

    pub fn by_cred_bytes(&self, cred: &[u8]) -> Option<(User, StoredPasskey)> {
        let want = hex::encode(cred);
        let g = lock(&self.inner);
        for u in g.users.values() {
            for p in &u.passkeys {
                if p.id == want || p.passkey.cred_id().as_slice() == cred {
                    return Some((u.clone(), p.clone()));
                }
            }
        }
        None
    }
}

fn persist(path: &Path, users: &HashMap<String, User>) -> anyhow::Result<()> {
    let file = UsersFile {
        users: users.values().cloned().collect(),
    };
    let bytes = serde_json::to_vec_pretty(&file)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

pub enum PendingEntry {
    Register {
        email: String,
        user_id: Uuid,
        state: PasskeyRegistration,
        add: bool,
        created: Instant,
    },
    LoginSpecific {
        email: String,
        state: PasskeyAuthentication,
        created: Instant,
    },
    LoginDiscoverable {
        state: DiscoverableAuthentication,
        created: Instant,
    },
}

#[derive(Clone)]
pub struct Pending {
    inner: Arc<Mutex<HashMap<String, PendingEntry>>>,
    rate: Arc<Mutex<HashMap<IpAddr, Vec<StdInstant>>>>,
}

impl Default for Pending {
    fn default() -> Self {
        Self::new()
    }
}

impl Pending {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            rate: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn allow_rate(&self, ip: IpAddr) -> bool {
        let now = StdInstant::now();
        let mut hits = self.rate.lock().unwrap_or_else(|e| e.into_inner());
        let w = hits.entry(ip).or_default();
        w.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
        if w.len() >= 10 {
            return false;
        }
        w.push(now);
        true
    }

    pub fn insert(&self, entry: PendingEntry) -> String {
        let handle = random_token();
        let mut g = lock(&self.inner);
        sweep(&mut g);
        g.insert(handle.clone(), entry);
        handle
    }

    pub fn take(&self, handle: &str) -> Result<PendingEntry, PendingErr> {
        let mut g = lock(&self.inner);
        sweep(&mut g);
        match g.remove(handle) {
            Some(e) if expired(&e) => Err(PendingErr::Expired),
            Some(e) => Ok(e),
            None => Err(PendingErr::Missing),
        }
    }
}

pub enum PendingErr {
    Missing,
    Expired,
}

fn expired(e: &PendingEntry) -> bool {
    let created = match e {
        PendingEntry::Register { created, .. }
        | PendingEntry::LoginSpecific { created, .. }
        | PendingEntry::LoginDiscoverable { created, .. } => *created,
    };
    Instant::now().saturating_duration_since(created) > HANDLE_TTL
}

fn sweep(map: &mut HashMap<String, PendingEntry>) {
    map.retain(|_, e| !expired(e));
}

/// Builds the WebAuthn config for `rp_id`/`origin`. Both come from the
/// operator-supplied `--hostname` flag (or the compiled-in default), so
/// every failure here is returned rather than panicked: called once, at
/// startup, from `AppState::open_with_hostname`, so a bad `--hostname`
/// fails the process before it binds a socket instead of panicking on the
/// first WebAuthn request.
pub(crate) fn webauthn(rp_id: &str, origin: &str) -> anyhow::Result<Webauthn> {
    let url = Url::parse(origin)
        .with_context(|| format!("--hostname {rp_id}: {origin} is not a valid origin"))?;
    WebauthnBuilder::new(rp_id, &url)
        .map_err(|_| {
            anyhow::anyhow!(
                "--hostname {rp_id} is not a registrable domain of {origin} \
                 (no port, path, scheme, or IP literal)"
            )
        })?
        .rp_name("secd")
        .build()
        .context("webauthn config")
}

pub fn normalize_email(raw: &str) -> Option<String> {
    let s = raw.trim().to_lowercase();
    if s.is_empty() || s.len() > EMAIL_MAX {
        return None;
    }
    let (local, domain) = s.split_once('@')?;
    if local.is_empty() || domain.is_empty() {
        return None;
    }
    if domain.contains('@') {
        return None;
    }
    if !domain.contains('.') {
        return None;
    }
    Some(s)
}

pub fn password_ok(password: &str) -> bool {
    let n = password.chars().count();
    (PW_MIN..=PW_MAX).contains(&n)
}

/// The browser mints the DEK and wraps it; the server only stores the
/// ciphertext. blob = 24-byte nonce + 32-byte DEK + 16-byte tag, hex.
pub fn parse_client_wrap(v: &Option<Value>, expect: &str) -> Result<StoredWrap, WrapErr> {
    let obj = match v {
        None | Some(Value::Null) => return Err(WrapErr::Missing),
        Some(Value::Object(m)) => m,
        Some(_) => return Err(WrapErr::Bad),
    };
    let factor = obj.get("factor").and_then(Value::as_str).unwrap_or("");
    if factor != expect {
        return Err(WrapErr::Bad);
    }
    let blob = obj.get("blob").and_then(Value::as_str).unwrap_or("");
    if blob.len() != (24 + 32 + 16) * 2 || !lower_hex(blob) {
        return Err(WrapErr::Bad);
    }
    let salt = obj.get("salt").and_then(Value::as_str);
    let cred_id = obj.get("cred_id").and_then(Value::as_str);
    match expect {
        "password" => {
            let Some(salt) = salt else {
                return Err(WrapErr::Bad);
            };
            if salt.len() != 32 || !lower_hex(salt) || cred_id.is_some() {
                return Err(WrapErr::Bad);
            }
            Ok(StoredWrap {
                factor: "password".into(),
                cred_id: None,
                salt: Some(salt.to_string()),
                blob: blob.to_string(),
            })
        }
        "passkey" => {
            let Some(cred_id) = cred_id else {
                return Err(WrapErr::Bad);
            };
            if cred_id.is_empty()
                || !cred_id.len().is_multiple_of(2)
                || !lower_hex(cred_id)
                || salt.is_some()
            {
                return Err(WrapErr::Bad);
            }
            Ok(StoredWrap {
                factor: "passkey".into(),
                cred_id: Some(cred_id.to_string()),
                salt: None,
                blob: blob.to_string(),
            })
        }
        _ => Err(WrapErr::Bad),
    }
}

fn lower_hex(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub enum WrapErr {
    Missing,
    Bad,
}

pub fn decode_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.len().is_multiple_of(2) && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        if let Ok(v) = hex::decode(s) {
            return Some(v);
        }
    }
    use base64::Engine;
    if let Ok(v) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s) {
        return Some(v);
    }
    if let Ok(v) = base64::engine::general_purpose::URL_SAFE.decode(s) {
        return Some(v);
    }
    if let Ok(v) = base64::engine::general_purpose::STANDARD.decode(s) {
        return Some(v);
    }
    None
}

pub fn wrap_json_list(wraps: &[Wrap]) -> Value {
    Value::Array(wraps.iter().map(wrap_json).collect())
}

pub fn wrap_json(w: &Wrap) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("factor".into(), json!(w.factor.as_str()));
    if let Some(c) = &w.cred_id {
        m.insert("cred_id".into(), json!(c));
    }
    if let Some(s) = &w.salt {
        m.insert("salt".into(), json!(s));
    }
    m.insert("blob".into(), json!(w.blob));
    Value::Object(m)
}

pub fn to_stored(w: &Wrap) -> StoredWrap {
    StoredWrap {
        factor: w.factor.as_str().to_string(),
        cred_id: w.cred_id.clone(),
        salt: w.salt.clone(),
        blob: w.blob.clone(),
    }
}

pub fn from_stored(w: &StoredWrap) -> Wrap {
    Wrap {
        factor: if w.factor == "passkey" {
            Factor::Passkey
        } else {
            Factor::Password
        },
        cred_id: w.cred_id.clone(),
        salt: w.salt.clone(),
        blob: w.blob.clone(),
    }
}

pub fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

pub fn random_token() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

pub fn parse_register_cred(v: &Value) -> Option<RegisterPublicKeyCredential> {
    serde_json::from_value(v.clone()).ok()
}

pub fn parse_login_cred(v: &Value) -> Option<PublicKeyCredential> {
    serde_json::from_value(v.clone()).ok()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
