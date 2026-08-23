use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

use crate::audit::AuditLog;
use crate::db::{Db, RawConn, Step};
use crate::headers::{fail_auth, json_status, json_value};
use crate::state::AppState;

pub const COOKIE_NAME: &str = "__Host-secd";
const CONSOLE_TTL: i64 = 12 * 60 * 60;
const DEVICE_TTL: i64 = 30 * 24 * 60 * 60;
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

    /// An unknown kind is no kind: a row we cannot classify is not a session.
    fn parse(s: &str) -> Option<Self> {
        match s {
            "console" => Some(Self::Console),
            "device" => Some(Self::Device),
            _ => None,
        }
    }

    fn ttl(self) -> i64 {
        match self {
            Self::Console => CONSOLE_TTL,
            Self::Device => DEVICE_TTL,
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

/// Sessions live in the sqlite store beside the vault and audit tables, so a
/// restart signs nobody out and a revocation stays revoked.
#[derive(Clone)]
pub struct SessionStore {
    db: Db,
}

impl SessionStore {
    pub fn open(dir: &std::path::Path) -> anyhow::Result<Self> {
        Ok(Self::from_db(Db::open(dir)?))
    }

    pub(crate) fn from_db(db: Db) -> Self {
        Self { db }
    }

    pub fn create_console(&self, email: &str) -> anyhow::Result<(String, String)> {
        self.create(email, SessionKind::Console, "This browser")
    }

    pub fn create_device(&self, email: &str, hostname: &str) -> anyhow::Result<(String, String)> {
        let label = if hostname.is_empty() {
            "device"
        } else {
            hostname
        };
        self.create(email, SessionKind::Device, label)
    }

    fn create(
        &self,
        email: &str,
        kind: SessionKind,
        label: &str,
    ) -> anyhow::Result<(String, String)> {
        let now = unix_now();
        let id = Uuid::new_v4().to_string();
        let token = random_token();
        let hash = token_hash(&token);
        let expires = now.saturating_add(kind.ttl());
        self.db.with(|conn| {
            let stmt = conn.prepare(
                "INSERT INTO sessions \
                 (token_hash, id, email, kind, label, created, last_seen, expires) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            stmt.bind_text(1, &hash)?;
            stmt.bind_text(2, &id)?;
            stmt.bind_text(3, email)?;
            stmt.bind_text(4, kind.as_str())?;
            stmt.bind_text(5, label)?;
            stmt.bind_i64(6, now)?;
            stmt.bind_i64(7, now)?;
            stmt.bind_i64(8, expires)?;
            stmt.run()
        })?;
        self.db.tighten();
        Ok((id, token))
    }

    /// A store that cannot answer denies: every caller treats `None` as
    /// unauthenticated.
    pub fn by_token(&self, token: &str) -> Option<Session> {
        self.lookup(token).ok().flatten()
    }

    fn lookup(&self, token: &str) -> anyhow::Result<Option<Session>> {
        let hash = token_hash(token);
        let now = unix_now();
        self.db.with(|conn| {
            sweep(conn, now)?;
            let sel = conn.prepare(
                "SELECT id, email, kind, label, created FROM sessions WHERE token_hash = ?",
            )?;
            sel.bind_text(1, &hash)?;
            let row = match sel.step()? {
                Step::Done => None,
                Step::Row => Some((
                    sel.text(0).unwrap_or_default(),
                    sel.text(1).unwrap_or_default(),
                    sel.text(2).unwrap_or_default(),
                    sel.text(3).unwrap_or_default(),
                    sel.i64_at(4),
                )),
            };
            drop(sel);
            let Some((id, email, kind, label, created)) = row else {
                return Ok(None);
            };
            let Some(kind) = SessionKind::parse(&kind) else {
                return Ok(None);
            };
            let touch = conn.prepare("UPDATE sessions SET last_seen = ? WHERE token_hash = ?")?;
            touch.bind_i64(1, now)?;
            touch.bind_text(2, &hash)?;
            touch.run()?;
            Ok(Some(Session {
                id,
                email,
                kind,
                label,
                created: from_unix(created),
                last_seen: from_unix(now),
            }))
        })
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

    pub fn list_for(&self, email: &str) -> anyhow::Result<Vec<Session>> {
        let now = unix_now();
        self.db.with(|conn| {
            sweep(conn, now)?;
            let stmt = conn.prepare(
                "SELECT id, kind, label, created, last_seen FROM sessions \
                 WHERE email = ? ORDER BY created",
            )?;
            stmt.bind_text(1, email)?;
            let mut out = Vec::new();
            loop {
                match stmt.step()? {
                    Step::Done => break,
                    Step::Row => {
                        let Some(kind) = SessionKind::parse(&stmt.text(1).unwrap_or_default())
                        else {
                            continue;
                        };
                        out.push(Session {
                            id: stmt.text(0).unwrap_or_default(),
                            email: email.to_string(),
                            kind,
                            label: stmt.text(2).unwrap_or_default(),
                            created: from_unix(stmt.i64_at(3)),
                            last_seen: from_unix(stmt.i64_at(4)),
                        });
                    }
                }
            }
            Ok(out)
        })
    }

    pub fn revoke_id(&self, email: &str, id: &str, audit: &AuditLog) -> anyhow::Result<Revoke> {
        let (revoked, event) = self.db.with(|conn| {
            conn.immediate(|| {
                let found = {
                    let sel = conn.prepare("SELECT email, kind FROM sessions WHERE id = ?")?;
                    sel.bind_text(1, id)?;
                    match sel.step()? {
                        Step::Done => None,
                        Step::Row => Some((
                            sel.text(0).unwrap_or_default(),
                            sel.text(1).unwrap_or_default(),
                        )),
                    }
                };
                let Some((owner, kind)) = found else {
                    return Ok((Revoke::Unknown, None));
                };
                if owner != email {
                    return Ok((Revoke::Unknown, None));
                }
                {
                    let del = conn.prepare("DELETE FROM sessions WHERE id = ?")?;
                    del.bind_text(1, id)?;
                    del.run()?;
                }
                let event = crate::audit::append_on(conn, "session.revoke", Some(id), &[])?;
                let revoked = if SessionKind::parse(&kind) == Some(SessionKind::Console) {
                    Revoke::Console
                } else {
                    Revoke::Other
                };
                Ok((revoked, Some(event)))
            })
        })?;
        self.db.tighten();
        if let Some(event) = event {
            audit.journal(&event);
        }
        Ok(revoked)
    }

    pub fn revoke_token(&self, token: &str) -> anyhow::Result<bool> {
        let hash = token_hash(token);
        self.db.with(|conn| {
            let del = conn.prepare("DELETE FROM sessions WHERE token_hash = ?")?;
            del.bind_text(1, &hash)?;
            del.run()?;
            Ok(conn.changes() > 0)
        })
    }

    pub fn revoke_token_with_audit(
        &self,
        token: &str,
        session_id: &str,
        audit: &AuditLog,
    ) -> anyhow::Result<()> {
        let hash = token_hash(token);
        let event = self.db.with(|conn| {
            conn.immediate(|| {
                {
                    let del = conn.prepare("DELETE FROM sessions WHERE token_hash = ?")?;
                    del.bind_text(1, &hash)?;
                    del.run()?;
                }
                crate::audit::append_on(conn, "session.revoke", Some(session_id), &[])
            })
        })?;
        self.db.tighten();
        audit.journal(&event);
        Ok(())
    }
}

pub enum Revoke {
    Unknown,
    Console,
    Other,
}

/// Expiry is a stored deadline, so it outlives the process that set it.
fn sweep(conn: &RawConn, now: i64) -> anyhow::Result<()> {
    let del = conn.prepare("DELETE FROM sessions WHERE expires <= ?")?;
    del.bind_i64(1, now)?;
    del.run()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sessions", get(list_sessions))
        .route(concat!("/api/v1/sessions/", "{id}"), delete(revoke_session))
}

async fn list_sessions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(current) = state.sessions.console_from_headers(&headers) else {
        return fail_auth();
    };
    let Ok(sessions) = state.sessions.list_for(&current.email) else {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    };
    let rows: Vec<_> = sessions
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
    match state.sessions.revoke_id(&current.email, &id, &state.audit) {
        Ok(Revoke::Unknown) => json_status(StatusCode::NOT_FOUND, "not found"),
        Ok(Revoke::Console) => {
            let mut res = json_value(StatusCode::OK, json!({"ok": true}));
            res.headers_mut().insert(header::SET_COOKIE, clear_cookie());
            res
        }
        Ok(Revoke::Other) => json_value(StatusCode::OK, json!({"ok": true})),
        Err(_) => json_status(StatusCode::INTERNAL_SERVER_ERROR, "store"),
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

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn from_unix(secs: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs.max(0) as u64)
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

/// The store holds the hash, never the bearer token: reading the database
/// yields nothing that can be presented as a session.
fn token_hash(token: &str) -> String {
    hex::encode(crate::audit::sha256(token.as_bytes()))
}

pub fn with_cookie(mut res: Response, token: &str) -> Response {
    res.headers_mut()
        .insert(header::SET_COOKIE, set_cookie(token));
    res
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(1);

    struct Dir(PathBuf);

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fresh_dir(tag: &str) -> Dir {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("secd-u-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("test dir");
        Dir(p)
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
            .block_on(f)
    }

    fn cookie(token: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{COOKIE_NAME}={token}")).expect("cookie"),
        );
        h
    }

    /// A deadline written before the restart is still a deadline after it.
    #[test]
    fn T_SESS_EXPIRY_PERSIST() {
        let dir = fresh_dir("expiry");
        let store = SessionStore::open(&dir.0).expect("open");
        let (id, token) = store.create_console("op@secd.test").expect("create");
        assert!(store.by_token(&token).is_some(), "fresh session is live");

        let db = Db::open(&dir.0).expect("db");
        db.with(|conn| {
            let stmt = conn.prepare("UPDATE sessions SET expires = 1 WHERE id = ?")?;
            stmt.bind_text(1, &id)?;
            stmt.run()
        })
        .expect("age the session");
        drop(store);

        let reopened = SessionStore::open(&dir.0).expect("reopen");
        assert!(
            reopened.by_token(&token).is_none(),
            "an expired session must not survive a restart"
        );
    }

    /// An audit insert that fails must fail the request and leave the
    /// session in place.
    #[test]
    fn T_AUDIT_INSERT_FAILS_REQUEST() {
        let dir = fresh_dir("audit-insert");
        let state = AppState::open(&dir.0).expect("state");
        let (_console_id, console_token) = state
            .sessions
            .create_console("op@secd.test")
            .expect("console");
        let (device_id, device_token) = state
            .sessions
            .create_device("op@secd.test", "testhost")
            .expect("device");

        // Reads still answer; the insert cannot, because `blocked` has no
        // default and the insert does not name it.
        let db = Db::open(&dir.0).expect("db");
        db.with(|conn| {
            conn.exec(
                "DROP TABLE audit;
                 CREATE TABLE audit (
                   seq INTEGER PRIMARY KEY AUTOINCREMENT,
                   action TEXT NOT NULL,
                   session_id TEXT,
                   names TEXT NOT NULL,
                   prev_hash TEXT NOT NULL,
                   hash TEXT NOT NULL,
                   blocked TEXT NOT NULL
                 );",
            )
        })
        .expect("break the audit insert");

        let res = block_on(revoke_session(
            State(state.clone()),
            Path(device_id),
            cookie(&console_token),
        ));
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            state.sessions.by_token(&device_token).is_some(),
            "a failed audit must not delete the session"
        );
    }
}
