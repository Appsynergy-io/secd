use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use serde_json::{json, Value};

use crate::db::{Db, RawConn, Step};
use crate::headers::{fail_auth, json_status, json_value};
use crate::state::AppState;

#[derive(Clone, Debug, Serialize)]
pub struct AuditEvent {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prev: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hash: String,
}

#[derive(Clone)]
pub struct AuditLog {
    db: Db,
    path: PathBuf,
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl AuditLog {
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        Self::from_db(Db::open(dir)?, dir)
    }

    pub(crate) fn from_db(db: Db, dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join("audit.jsonl");
        // A read that fails is not an empty chain: importing the jsonl on top
        // of rows we could not see would append the whole log twice.
        let mut events: Vec<AuditEvent> = list_audit(&db)
            .context("read the audit chain")?
            .iter()
            .map(row_to_event)
            .collect();
        if events.is_empty() && path.exists() {
            if let Ok(imported) = import_jsonl(&path, &db) {
                events = imported;
            }
        }
        Ok(Self {
            db,
            path,
            events: Arc::new(Mutex::new(events)),
        })
    }

    pub(crate) fn db(&self) -> &Db {
        &self.db
    }

    /// Sqlite is the chain: a failed INSERT or an unreadable head fails the
    /// record. jsonl is a replica written after COMMIT.
    pub fn record(&self, action: &str, session_id: Option<&str>) -> anyhow::Result<()> {
        self.record_names(action, session_id, &[])
    }

    pub fn record_names(
        &self,
        action: &str,
        session_id: Option<&str>,
        names: &[&str],
    ) -> anyhow::Result<()> {
        let event = self
            .db
            .with(|conn| conn.immediate(|| append_on(conn, action, session_id, names)))
            .context("append to the audit chain")?;
        self.db.tighten();
        self.journal(&event);
        Ok(())
    }

    /// Sqlite is the chain. A journal miss must not fail the request or
    /// roll back a committed row.
    pub(crate) fn journal(&self, event: &AuditEvent) {
        if let Ok(line) = serde_json::to_string(event) {
            if let Ok(mut f) = OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(&self.path)
            {
                if writeln!(f, "{line}").is_ok() {
                    let _ = f.sync_all();
                }
            }
        }
        lock(&self.events).push(event.clone());
    }

    pub fn events(&self) -> Vec<AuditEvent> {
        lock(&self.events).clone()
    }

    pub fn verify(&self) -> bool {
        let Ok(rows) = list_audit(&self.db) else {
            return false;
        };
        verify_rows(rows) && verify_jsonl(&self.path)
    }
}

fn row_to_event(row: &AuditRow) -> AuditEvent {
    let names: Vec<String> = serde_json::from_str(&row.names).unwrap_or_default();
    AuditEvent {
        action: row.action.clone(),
        session_id: row.session_id.clone(),
        names,
        prev: row.prev_hash.clone(),
        hash: row.hash.clone(),
    }
}

fn import_jsonl(path: &Path, db: &Db) -> anyhow::Result<Vec<AuditEvent>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut prev = zero_hash();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line)?;
        let action = v
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let session_id = v
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let names: Vec<String> = v
            .get("names")
            .and_then(|n| serde_json::from_value(n.clone()).ok())
            .unwrap_or_default();
        let names_json = serde_json::to_string(&names).unwrap_or_else(|_| "[]".into());
        let hash = event_hash(&prev, &action, session_id.as_deref(), &names_json);
        db.with(|conn| {
            insert_on(
                conn,
                &action,
                session_id.as_deref(),
                &names_json,
                &prev,
                &hash,
            )
        })?;
        events.push(AuditEvent {
            action,
            session_id,
            names,
            prev: prev.clone(),
            hash: hash.clone(),
        });
        prev = hash;
    }
    Ok(events)
}

fn verify_rows(rows: Vec<AuditRow>) -> bool {
    let mut prev = zero_hash();
    for row in rows {
        if row.prev_hash != prev {
            return false;
        }
        let expect = event_hash(&prev, &row.action, row.session_id.as_deref(), &row.names);
        if expect != row.hash {
            return false;
        }
        prev = row.hash;
    }
    true
}

fn verify_jsonl(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let reader = BufReader::new(file);
    let mut prev = zero_hash();
    for line in reader.lines() {
        let Ok(line) = line else {
            return false;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        let action = v.get("action").and_then(Value::as_str).unwrap_or("");
        let session_id = v.get("session_id").and_then(Value::as_str);
        let names_val = v.get("names").cloned().unwrap_or_else(|| json!([]));
        let names_json = if names_val.is_array() {
            names_val.to_string()
        } else {
            "[]".into()
        };
        let stored_prev = v.get("prev").and_then(Value::as_str);
        let stored_hash = v.get("hash").and_then(Value::as_str);
        let expect = event_hash(&prev, action, session_id, &names_json);
        if let Some(sp) = stored_prev {
            if sp != prev {
                return false;
            }
        }
        if let Some(sh) = stored_hash {
            if sh != expect {
                return false;
            }
        } else {
            return false;
        }
        prev = expect;
    }
    true
}

pub(crate) fn event_hash(
    prev: &str,
    action: &str,
    session_id: Option<&str>,
    names_json: &str,
) -> String {
    let mut buf = Vec::with_capacity(
        prev.len() + action.len() + session_id.map_or(0, str::len) + names_json.len() + 4,
    );
    buf.extend_from_slice(prev.as_bytes());
    buf.push(0x1f);
    buf.extend_from_slice(action.as_bytes());
    buf.push(0x1f);
    if let Some(s) = session_id {
        buf.extend_from_slice(s.as_bytes());
    }
    buf.push(0x1f);
    buf.extend_from_slice(names_json.as_bytes());
    hex::encode(sha256(&buf))
}

/// SELECT the head and INSERT the next row. Caller holds BEGIN IMMEDIATE.
/// The hash is computed from the head read here, not from a previous with().
pub(crate) fn append_on(
    conn: &RawConn,
    action: &str,
    session_id: Option<&str>,
    names: &[&str],
) -> anyhow::Result<AuditEvent> {
    let names_owned: Vec<String> = names.iter().map(|s| (*s).to_string()).collect();
    let names_json = serde_json::to_string(&names_owned).context("audit names json")?;
    let prev = head_hash(conn).context("read the audit head")?;
    let hash = event_hash(&prev, action, session_id, &names_json);
    insert_on(conn, action, session_id, &names_json, &prev, &hash)
        .context("append to the audit chain")?;
    Ok(AuditEvent {
        action: action.to_string(),
        session_id: session_id.map(str::to_string),
        names: names_owned,
        prev,
        hash,
    })
}

fn head_hash(conn: &RawConn) -> anyhow::Result<String> {
    let stmt = conn.prepare("SELECT hash FROM audit ORDER BY seq DESC LIMIT 1")?;
    match stmt.step()? {
        Step::Row => Ok(stmt.text(0).unwrap_or_else(zero_hash)),
        Step::Done => Ok(zero_hash()),
    }
}

fn insert_on(
    conn: &RawConn,
    action: &str,
    session_id: Option<&str>,
    names_json: &str,
    prev_hash: &str,
    hash: &str,
) -> anyhow::Result<()> {
    let stmt = conn.prepare(
        "INSERT INTO audit (action, session_id, names, prev_hash, hash) VALUES (?, ?, ?, ?, ?)",
    )?;
    stmt.bind_text(1, action)?;
    stmt.bind_opt(2, session_id)?;
    stmt.bind_text(3, names_json)?;
    stmt.bind_text(4, prev_hash)?;
    stmt.bind_text(5, hash)?;
    stmt.run()
}

#[cfg(test)]
pub(crate) fn last_audit_hash(db: &Db) -> anyhow::Result<String> {
    db.with(head_hash)
}

pub(crate) fn list_audit(db: &Db) -> anyhow::Result<Vec<AuditRow>> {
    db.with(|conn| {
        let stmt = conn.prepare(
            "SELECT seq, action, session_id, names, prev_hash, hash FROM audit ORDER BY seq",
        )?;
        let mut out = Vec::new();
        loop {
            match stmt.step()? {
                Step::Done => break,
                Step::Row => {
                    out.push(AuditRow {
                        seq: stmt.i64_at(0),
                        action: stmt.text(1).unwrap_or_default(),
                        session_id: stmt.text(2),
                        names: stmt.text(3).unwrap_or_else(|| "[]".into()),
                        prev_hash: stmt.text(4).unwrap_or_default(),
                        hash: stmt.text(5).unwrap_or_default(),
                    });
                }
            }
        }
        Ok(out)
    })
}

pub(crate) struct AuditRow {
    pub seq: i64,
    pub action: String,
    pub session_id: Option<String>,
    pub names: String,
    pub prev_hash: String,
    pub hash: String,
}

pub(crate) fn zero_hash() -> String {
    "0".repeat(64)
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/audit", get(get_audit))
}

async fn get_audit(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.sessions.vault_from_headers(&headers).is_none() {
        return fail_auth();
    }
    let rows = match list_audit(state.audit.db()) {
        Ok(r) => r,
        Err(_) => return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store"),
    };
    let head = rows.last().map_or_else(zero_hash, |row| row.hash.clone());
    let events: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            let names: Value = serde_json::from_str(&row.names).unwrap_or_else(|_| json!([]));
            let mut m = serde_json::Map::new();
            m.insert("seq".into(), json!(row.seq));
            m.insert("action".into(), json!(row.action));
            m.insert("names".into(), names);
            if let Some(id) = row.session_id {
                m.insert("session_id".into(), json!(id));
            }
            m.insert("prev".into(), json!(row.prev_hash));
            m.insert("hash".into(), json!(row.hash));
            Value::Object(m)
        })
        .collect();
    json_value(
        StatusCode::OK,
        json!({ "events": events, "head": head, "verified": state.audit.verify() }),
    )
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (input.len() as u64).saturating_mul(8);
    let mut msg = input.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::db::Step;
    use crate::state::AppState;
    use axum::http::{header, HeaderValue};

    fn fresh_dir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "secd-u-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("test dir");
        p
    }

    fn audit_rows(db: &Db) -> i64 {
        db.with(|conn| {
            let stmt = conn.prepare("SELECT COUNT(*) FROM audit")?;
            match stmt.step()? {
                Step::Row => Ok(stmt.i64_at(0)),
                Step::Done => Ok(0),
            }
        })
        .expect("count")
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
            .block_on(f)
    }

    fn bearer(token: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("bearer"),
        );
        h
    }

    async fn get_audit_json(state: &AppState, token: &str) -> Value {
        let res = get_audit(State(state.clone()), bearer(token)).await;
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .expect("body");
        serde_json::from_slice(&bytes).expect("json")
    }

    /// The route is the chain: every row in order with its seq and hashes,
    /// the head the next append will link to, and the verify verdict.
    #[test]
    fn T_AUDIT_GET_CHAIN() {
        let dir = fresh_dir("audit-get");
        let state = AppState::open(&dir).expect("state");
        let (_id, token) = state
            .sessions
            .create_console("op@secd.test")
            .expect("console");

        let empty = block_on(get_audit_json(&state, &token));
        assert_eq!(empty["events"], json!([]));
        assert_eq!(empty["head"], json!(zero_hash()));
        assert_eq!(empty["verified"], json!(true));

        state
            .audit
            .record_names("vault.put", Some("s-1"), &["kv/a"])
            .expect("first");
        state.audit.record("session.revoke", None).expect("second");
        let appended = state.audit.events();
        assert_eq!(appended.len(), 2);

        let body = block_on(get_audit_json(&state, &token));
        let events = body["events"].as_array().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["seq"], json!(1));
        assert_eq!(events[0]["action"], json!("vault.put"));
        assert_eq!(events[0]["names"], json!(["kv/a"]));
        assert_eq!(events[0]["session_id"], json!("s-1"));
        assert_eq!(events[0]["prev"], json!(zero_hash()));
        assert_eq!(events[0]["hash"], json!(appended[0].hash));
        assert_eq!(events[1]["seq"], json!(2));
        assert_eq!(events[1]["action"], json!("session.revoke"));
        assert_eq!(events[1]["names"], json!([]));
        assert!(events[1].get("session_id").is_none());
        assert_eq!(events[1]["prev"], json!(appended[0].hash));
        assert_eq!(events[1]["hash"], json!(appended[1].hash));
        assert_eq!(body["head"], json!(appended[1].hash));
        assert_eq!(body["verified"], json!(true));

        let denied = block_on(get_audit(State(state.clone()), HeaderMap::new()));
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A head the chain cannot read is an error, never a chain that starts
    /// again from zero.
    #[test]
    fn T_AUDIT_READ_FAILS_CLOSED() {
        let dir = fresh_dir("audit-read");
        let log = AuditLog::open(&dir).expect("open");
        log.record("session.revoke", Some("first")).expect("record");

        // Inserts still land; the head cannot be read, because the ordering
        // column the read names is gone.
        let db = Db::open(&dir).expect("db");
        db.with(|conn| {
            conn.exec(
                "DROP TABLE audit;
                 CREATE TABLE audit (
                   action TEXT NOT NULL,
                   session_id TEXT,
                   names TEXT NOT NULL,
                   prev_hash TEXT NOT NULL,
                   hash TEXT NOT NULL
                 );",
            )
        })
        .expect("break the audit read");

        assert!(
            last_audit_hash(&db).is_err(),
            "an unreadable chain must not report a head"
        );
        assert!(
            log.record("session.revoke", Some("second")).is_err(),
            "a failed head read must fail the record"
        );
        assert_eq!(
            audit_rows(&db),
            0,
            "a failed head read must not restart the chain from zero"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
