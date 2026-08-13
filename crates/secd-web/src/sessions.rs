use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get};
use axum::Router;
use rand::RngCore;
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::headers::{fail_auth, json_status, json_value};
use crate::state::AppState;

pub const COOKIE_NAME: &str = "__Host-secd";
const CONSOLE_TTL: Duration = Duration::from_secs(12 * 60 * 60);
const DEVICE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const COOKIE_MAX_AGE: i64 = 12 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionKind {
    Console,
    Device,
}

impl SessionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Device => "device",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub email: String,
    pub kind: SessionKind,
    pub label: String,
    pub created: SystemTime,
    pub last_seen: SystemTime,
}

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Mutex<HashMap<String, Session>>>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_console(&self, email: &str) -> (String, String) {
        self.create(email, SessionKind::Console, "This browser")
    }

    pub fn create_device(&self, email: &str, hostname: &str) -> (String, String) {
        let label = if hostname.is_empty() {
            "device"
        } else {
            hostname
        };
        self.create(email, SessionKind::Device, label)
    }

    fn create(&self, email: &str, kind: SessionKind, label: &str) -> (String, String) {
        let now = SystemTime::now();
        let id = Uuid::new_v4().to_string();
        let token = random_token();
        let session = Session {
            id: id.clone(),
            email: email.to_string(),
            kind,
            label: label.to_string(),
            created: now,
            last_seen: now,
        };
        lock(&self.inner).insert(token.clone(), session);
        (id, token)
    }

    pub fn by_token(&self, token: &str) -> Option<Session> {
        let now = SystemTime::now();
        let mut map = lock(&self.inner);
        let s = map.get(token)?;
        if expired(s, now) {
            map.remove(token);
            return None;
        }
        if let Some(s) = map.get_mut(token) {
            s.last_seen = now;
        }
        map.get(token).cloned()
    }

    pub fn console_from_headers(&self, headers: &HeaderMap) -> Option<Session> {
        let token = cookie_token(headers)?;
        let s = self.by_token(&token)?;
        if s.kind != SessionKind::Console {
            return None;
        }
        Some(s)
    }

    pub fn vault_from_headers(&self, headers: &HeaderMap) -> Option<Session> {
        if let Some(token) = cookie_token(headers) {
            if let Some(s) = self.by_token(&token) {
                return Some(s);
            }
        }
        let token = bearer_token(headers)?;
        self.by_token(&token)
    }

    pub fn device_from_headers(&self, headers: &HeaderMap) -> Option<Session> {
        let token = bearer_token(headers)?;
        let s = self.by_token(&token)?;
        if s.kind != SessionKind::Device {
            return None;
        }
        Some(s)
    }

    pub fn list_for(&self, email: &str) -> Vec<Session> {
        let now = SystemTime::now();
        let mut map = lock(&self.inner);
        map.retain(|_, s| !expired(s, now));
        map.values().filter(|s| s.email == email).cloned().collect()
    }

    pub fn revoke_id(&self, email: &str, id: &str) -> Revoke {
        let mut map = lock(&self.inner);
        let Some(found) = map.values().find(|s| s.id == id).cloned() else {
            return Revoke::Unknown;
        };
        if found.email != email {
            return Revoke::Unknown;
        }
        map.retain(|_, s| s.id != id);
        if found.kind == SessionKind::Console {
            Revoke::Console
        } else {
            Revoke::Other
        }
    }

    pub fn revoke_token(&self, token: &str) -> bool {
        lock(&self.inner).remove(token).is_some()
    }
}

pub enum Revoke {
    Unknown,
    Console,
    Other,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .without_v07_checks()
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions/:id", delete(revoke_session))
}

async fn list_sessions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(current) = state.sessions.console_from_headers(&headers) else {
        return fail_auth();
    };
    let rows: Vec<_> = state
        .sessions
        .list_for(&current.email)
        .into_iter()
        .map(|s| {
            json!({
                "id": s.id,
                "kind": s.kind.as_str(),
                "label": s.label,
                "created": rfc3339(s.created),
                "last_seen": rfc3339(s.last_seen),
                "current": s.id == current.id,
            })
        })
        .collect();
    json_value(StatusCode::OK, json!({ "sessions": rows }))
}

async fn revoke_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(current) = state.sessions.console_from_headers(&headers) else {
        return fail_auth();
    };
    match state.sessions.revoke_id(&current.email, &id) {
        Revoke::Unknown => json_status(StatusCode::NOT_FOUND, "not found"),
        Revoke::Console => {
            state.audit.record("session.revoke", Some(&id));
            let mut res = json_value(StatusCode::OK, json!({"ok": true}));
            res.headers_mut().insert(header::SET_COOKIE, clear_cookie());
            res
        }
        Revoke::Other => {
            state.audit.record("session.revoke", Some(&id));
            json_value(StatusCode::OK, json!({"ok": true}))
        }
    }
}

pub fn set_cookie(token: &str) -> HeaderValue {
    let v = format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={COOKIE_MAX_AGE}"
    );
    HeaderValue::from_str(&v).expect("invariant: cookie token is ascii")
}

pub fn clear_cookie() -> HeaderValue {
    HeaderValue::from_static("__Host-secd=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0")
}

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        if name == COOKIE_NAME {
            if value.is_empty() {
                return None;
            }
            return Some(value.to_string());
        }
    }
    None
}

pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let rest = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest.to_string())
}

fn expired(s: &Session, now: SystemTime) -> bool {
    let ttl = match s.kind {
        SessionKind::Console => CONSOLE_TTL,
        SessionKind::Device => DEVICE_TTL,
    };
    now.duration_since(s.created)
        .map(|d| d > ttl)
        .unwrap_or(true)
}

fn rfc3339(t: SystemTime) -> String {
    let d = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let odt = OffsetDateTime::from_unix_timestamp(d.as_secs() as i64)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    odt.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

fn random_token() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn with_cookie(mut res: Response, token: &str) -> Response {
    res.headers_mut()
        .insert(header::SET_COOKIE, set_cookie(token));
    res
}
