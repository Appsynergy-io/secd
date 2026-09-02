use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use secd_core::{check_name, CustomProvider, Field};
use serde_json::{json, Value};

use crate::audit::AuditLog;
use crate::db::{Db, Step};
use crate::headers::{fail_auth, json_status, json_value};
use crate::state::AppState;

#[derive(Clone)]
pub struct VaultEntry {
    pub name: String,
    pub ciphertext: Value,
    pub meta: Value,
    /// Current version number; 1 for entries that predate any change.
    pub version: i64,
    /// `created` of the newest version row; None for an entry without one.
    pub updated: Option<String>,
}

pub struct VersionRow {
    pub seq: i64,
    pub created: String,
    pub meta: Value,
}

#[derive(Clone)]
pub struct VaultStore {
    db: Db,
    dir: PathBuf,
}

impl VaultStore {
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        Ok(Self::from_db(Db::open(dir)?, dir))
    }

    pub(crate) fn from_db(db: Db, dir: &Path) -> Self {
        let store = Self {
            db,
            dir: dir.to_path_buf(),
        };
        store.sync_wraps();
        store
    }

    pub fn entries(&self) -> anyhow::Result<Vec<VaultEntry>> {
        self.db.with(|conn| {
            let stmt = conn.prepare(
                "SELECT e.name, e.ciphertext, e.meta, \
                 COALESCE((SELECT MAX(v.seq) FROM versions v WHERE v.name = e.name), 1), \
                 (SELECT v.created FROM versions v WHERE v.name = e.name \
                  ORDER BY v.seq DESC LIMIT 1) \
                 FROM entries e ORDER BY e.name",
            )?;
            let mut out = Vec::new();
            loop {
                match stmt.step()? {
                    Step::Done => break,
                    Step::Row => {
                        let name = stmt.text(0).unwrap_or_default();
                        let ct_raw = stmt.text(1).unwrap_or_else(|| "null".into());
                        let meta_raw = stmt.text(2).unwrap_or_else(|| "{}".into());
                        let version = stmt
                            .text(3)
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(1);
                        let updated = stmt.text(4);
                        let ciphertext = serde_json::from_str(&ct_raw).unwrap_or(Value::Null);
                        let meta = serde_json::from_str(&meta_raw).unwrap_or_else(|_| json!({}));
                        out.push(VaultEntry {
                            name,
                            ciphertext,
                            meta,
                            version,
                            updated,
                        });
                    }
                }
            }
            Ok(out)
        })
    }

    pub fn replace_entries(
        &self,
        entries: &[VaultEntry],
        audit: &AuditLog,
        session_id: Option<&str>,
        names: &[&str],
    ) -> anyhow::Result<()> {
        let created = crate::auth::now_rfc3339();
        let event = self.db.with(|conn| {
            conn.immediate(|| {
                {
                    // Names whose ciphertext changes (or appears) in this snapshot
                    // get a new version row; unchanged names keep their history.
                    let mut before: std::collections::HashMap<String, String> =
                        std::collections::HashMap::new();
                    let sel = conn.prepare("SELECT name, ciphertext FROM entries")?;
                    loop {
                        match sel.step()? {
                            Step::Done => break,
                            Step::Row => {
                                before.insert(
                                    sel.text(0).unwrap_or_default(),
                                    sel.text(1).unwrap_or_default(),
                                );
                            }
                        }
                    }
                    conn.exec("DELETE FROM entries")?;
                    let stmt = conn
                        .prepare("INSERT INTO entries (name, ciphertext, meta) VALUES (?, ?, ?)")?;
                    let ver = conn.prepare(
                        "INSERT INTO versions (name, seq, ciphertext, meta, created) VALUES (?, \
                         (SELECT COALESCE(MAX(seq), 0) + 1 FROM versions WHERE name = ?), ?, ?, ?)",
                    )?;
                    for e in entries {
                        let ct = e.ciphertext.to_string();
                        let meta = e.meta.to_string();
                        stmt.reset()?;
                        stmt.bind_text(1, &e.name)?;
                        stmt.bind_text(2, &ct)?;
                        stmt.bind_text(3, &meta)?;
                        match stmt.step()? {
                            Step::Done => {}
                            Step::Row => return Err(anyhow!("insert returned a row")),
                        }
                        if before.get(&e.name).map(String::as_str) != Some(ct.as_str()) {
                            ver.reset()?;
                            ver.bind_text(1, &e.name)?;
                            ver.bind_text(2, &e.name)?;
                            ver.bind_text(3, &ct)?;
                            ver.bind_text(4, &meta)?;
                            ver.bind_text(5, &created)?;
                            match ver.step()? {
                                Step::Done => {}
                                Step::Row => return Err(anyhow!("insert returned a row")),
                            }
                        }
                    }
                }
                crate::audit::append_on(conn, "vault.put", session_id, names)
            })
        })?;
        self.db.tighten();
        self.sync_wraps();
        audit.journal(&event);
        Ok(())
    }

    pub fn versions_of(&self, name: &str) -> anyhow::Result<Vec<VersionRow>> {
        self.db.with(|conn| {
            let stmt = conn
                .prepare("SELECT seq, created, meta FROM versions WHERE name = ? ORDER BY seq")?;
            stmt.bind_text(1, name)?;
            let mut out = Vec::new();
            loop {
                match stmt.step()? {
                    Step::Done => break,
                    Step::Row => {
                        let seq = stmt
                            .text(0)
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(0);
                        let created = stmt.text(1).unwrap_or_default();
                        let meta_raw = stmt.text(2).unwrap_or_else(|| "{}".into());
                        let meta = serde_json::from_str(&meta_raw).unwrap_or_else(|_| json!({}));
                        out.push(VersionRow { seq, created, meta });
                    }
                }
            }
            Ok(out)
        })
    }

    /// Restores version `seq` of `name` as the current entry and appends the
    /// restored ciphertext as a new version. Returns false when no such
    /// version exists.
    pub fn rollback(
        &self,
        name: &str,
        seq: i64,
        audit: &AuditLog,
        session_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        let created = crate::auth::now_rfc3339();
        let (hit, event) = self.db.with(|conn| {
            conn.immediate(|| {
                let sel = conn
                    .prepare("SELECT ciphertext, meta FROM versions WHERE name = ? AND seq = ?")?;
                sel.bind_text(1, name)?;
                sel.bind_text(2, &seq.to_string())?;
                let (ct, meta) = match sel.step()? {
                    Step::Done => return Ok((false, None)),
                    Step::Row => (
                        sel.text(0).unwrap_or_default(),
                        sel.text(1).unwrap_or_else(|| "{}".into()),
                    ),
                };
                drop(sel);
                let up = conn.prepare(
                    "INSERT OR REPLACE INTO entries (name, ciphertext, meta) VALUES (?, ?, ?)",
                )?;
                up.bind_text(1, name)?;
                up.bind_text(2, &ct)?;
                up.bind_text(3, &meta)?;
                match up.step()? {
                    Step::Done => {}
                    Step::Row => return Err(anyhow!("insert returned a row")),
                }
                drop(up);
                let ver = conn.prepare(
                    "INSERT INTO versions (name, seq, ciphertext, meta, created) VALUES (?, \
                     (SELECT COALESCE(MAX(seq), 0) + 1 FROM versions WHERE name = ?), ?, ?, ?)",
                )?;
                ver.bind_text(1, name)?;
                ver.bind_text(2, name)?;
                ver.bind_text(3, &ct)?;
                ver.bind_text(4, &meta)?;
                ver.bind_text(5, &created)?;
                match ver.step()? {
                    Step::Done => {}
                    Step::Row => return Err(anyhow!("insert returned a row")),
                }
                drop(ver);
                let event = crate::audit::append_on(conn, "vault.rollback", session_id, &[name])?;
                Ok((true, Some(event)))
            })
        })?;
        self.db.tighten();
        if let Some(event) = event {
            audit.journal(&event);
        }
        Ok(hit)
    }

    pub fn list_custom_providers(&self) -> anyhow::Result<Vec<CustomProvider>> {
        self.db.with(|conn| {
            let stmt =
                conn.prepare("SELECT name, title, fields FROM custom_providers ORDER BY name")?;
            let mut out = Vec::new();
            loop {
                match stmt.step()? {
                    Step::Done => break,
                    Step::Row => {
                        let name = stmt.text(0).unwrap_or_default();
                        let title = stmt.text(1).unwrap_or_default();
                        let fields_raw = stmt.text(2).unwrap_or_else(|| "[]".into());
                        let fields = parse_stored_fields(&fields_raw);
                        out.push(CustomProvider {
                            name,
                            title,
                            fields,
                        });
                    }
                }
            }
            Ok(out)
        })
    }

    pub fn put_custom_provider(&self, p: &CustomProvider, audit: &AuditLog) -> anyhow::Result<()> {
        let fields = serde_json::to_string(&fields_json(&p.fields)).context("fields json")?;
        let event = self.db.with(|conn| {
            conn.immediate(|| {
                {
                    let stmt = conn.prepare(
                        "INSERT INTO custom_providers (name, title, fields) VALUES (?, ?, ?)
                         ON CONFLICT(name) DO UPDATE SET title=excluded.title, fields=excluded.fields",
                    )?;
                    stmt.bind_text(1, &p.name)?;
                    stmt.bind_text(2, &p.title)?;
                    stmt.bind_text(3, &fields)?;
                    stmt.run()?;
                }
                crate::audit::append_on(conn, "provider.put", None, &[p.name.as_str()])
            })
        })?;
        self.db.tighten();
        audit.journal(&event);
        Ok(())
    }

    pub fn delete_custom_provider(&self, name: &str, audit: &AuditLog) -> anyhow::Result<bool> {
        let event = self.db.with(|conn| {
            conn.immediate(|| {
                {
                    let stmt = conn.prepare("DELETE FROM custom_providers WHERE name = ?")?;
                    stmt.bind_text(1, name)?;
                    stmt.run()?;
                }
                if conn.changes() == 0 {
                    return Ok(None);
                }
                Ok(Some(crate::audit::append_on(
                    conn,
                    "provider.delete",
                    None,
                    &[name],
                )?))
            })
        })?;
        self.db.tighten();
        if let Some(event) = event {
            audit.journal(&event);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn sync_wraps(&self) {
        let path = self.dir.join("users.json");
        if !path.exists() {
            return;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(v) = serde_json::from_str::<Value>(&raw) else {
            return;
        };
        let rows = collect_wraps(&v);
        let _ = self.replace_wraps(&rows);
    }

    fn replace_wraps(&self, rows: &[WrapRow]) -> anyhow::Result<()> {
        self.db.with(|conn| {
            conn.immediate(|| {
                conn.exec("DELETE FROM wraps")?;
                let stmt = conn.prepare(
                    "INSERT INTO wraps (factor, cred_id, salt, blob) VALUES (?, ?, ?, ?)",
                )?;
                for w in rows {
                    stmt.reset()?;
                    stmt.bind_text(1, &w.factor)?;
                    stmt.bind_opt(2, w.cred_id.as_deref())?;
                    stmt.bind_opt(3, w.salt.as_deref())?;
                    stmt.bind_text(4, &w.blob)?;
                    match stmt.step()? {
                        Step::Done => {}
                        Step::Row => return Err(anyhow!("insert returned a row")),
                    }
                }
                Ok(())
            })
        })
    }
}

struct WrapRow {
    factor: String,
    cred_id: Option<String>,
    salt: Option<String>,
    blob: String,
}

fn collect_wraps(v: &Value) -> Vec<WrapRow> {
    let mut out = Vec::new();
    let Some(users) = v.get("users").and_then(|u| u.as_array()) else {
        return out;
    };
    for u in users {
        if let Some(w) = u.get("password").and_then(wrap_row) {
            out.push(w);
        }
        if let Some(pks) = u.get("passkeys").and_then(|x| x.as_array()) {
            for pk in pks {
                if let Some(w) = pk.get("wrap").and_then(wrap_row) {
                    out.push(w);
                }
            }
        }
    }
    out
}

fn wrap_row(v: &Value) -> Option<WrapRow> {
    let factor = v.get("factor")?.as_str()?.to_string();
    let blob = v.get("blob")?.as_str()?.to_string();
    Some(WrapRow {
        factor,
        cred_id: v
            .get("cred_id")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        salt: v.get("salt").and_then(|x| x.as_str()).map(str::to_string),
        blob,
    })
}

fn parse_stored_fields(raw: &str) -> Vec<Field> {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    arr.iter().filter_map(field_from_value).collect()
}

fn field_from_value(v: &Value) -> Option<Field> {
    let obj = v.as_object()?;
    let key = obj.get("key")?.as_str()?.to_string();
    let env = obj.get("env")?.as_str()?.to_string();
    if key.is_empty() || env.is_empty() {
        return None;
    }
    Some(Field {
        key,
        secret: obj.get("secret").and_then(Value::as_bool).unwrap_or(false),
        optional: obj
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        env,
    })
}

pub(crate) fn fields_json(fields: &[Field]) -> Value {
    Value::Array(
        fields
            .iter()
            .map(|f| {
                json!({
                    "key": f.key,
                    "secret": f.secret,
                    "optional": f.optional,
                    "env": f.env,
                })
            })
            .collect(),
    )
}

pub(crate) fn has_forbidden(v: &Value) -> bool {
    match v {
        Value::Object(m) => {
            m.keys()
                .any(|k| matches!(k.as_str(), "value" | "plaintext" | "dek"))
                || m.values().any(has_forbidden)
        }
        Value::Array(a) => a.iter().any(has_forbidden),
        _ => false,
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/vault", get(get_vault).put(put_vault))
        .route("/api/v1/vault/versions", get(get_versions))
        .route("/api/v1/vault/rollback", post(post_rollback))
}

async fn get_vault(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.sessions.vault_from_headers(&headers).is_none() {
        return fail_auth();
    }
    state.vault.sync_wraps();
    match state.vault.entries() {
        Ok(rows) => {
            let entries: Vec<Value> = rows
                .iter()
                .map(|e| {
                    json!({
                        "name": e.name,
                        "ciphertext": e.ciphertext,
                        "meta": e.meta,
                        "version": e.version,
                        "updated": e.updated,
                    })
                })
                .collect();
            json_value(StatusCode::OK, json!({ "entries": entries }))
        }
        Err(_) => json_status(StatusCode::INTERNAL_SERVER_ERROR, "store"),
    }
}

async fn put_vault(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let Some(session) = state.sessions.vault_from_headers(&headers) else {
        return fail_auth();
    };
    if has_forbidden(&body) {
        return json_status(StatusCode::BAD_REQUEST, "plaintext");
    }
    let Some(obj) = body.as_object() else {
        return json_status(StatusCode::BAD_REQUEST, "body");
    };
    if obj.keys().any(|k| k != "entries") {
        return json_status(StatusCode::BAD_REQUEST, "plaintext");
    }
    let Some(arr) = obj.get("entries").and_then(Value::as_array) else {
        return json_status(StatusCode::BAD_REQUEST, "entries");
    };
    let mut entries = Vec::with_capacity(arr.len());
    let mut names = Vec::with_capacity(arr.len());
    for item in arr {
        if has_forbidden(item) {
            return json_status(StatusCode::BAD_REQUEST, "plaintext");
        }
        let Some(o) = item.as_object() else {
            return json_status(StatusCode::BAD_REQUEST, "entries");
        };
        for k in o.keys() {
            if !matches!(k.as_str(), "name" | "ciphertext" | "meta") {
                return json_status(StatusCode::BAD_REQUEST, "plaintext");
            }
        }
        let Some(name) = o.get("name").and_then(Value::as_str) else {
            return json_status(StatusCode::BAD_REQUEST, "name");
        };
        if check_name(name).is_err() {
            return json_status(StatusCode::BAD_REQUEST, "name");
        }
        let Some(ciphertext) = o.get("ciphertext").cloned() else {
            return json_status(StatusCode::BAD_REQUEST, "ciphertext");
        };
        if ciphertext.is_null() {
            return json_status(StatusCode::BAD_REQUEST, "ciphertext");
        }
        let meta = match o.get("meta") {
            None => json!({}),
            Some(m) if m.is_object() => m.clone(),
            Some(_) => return json_status(StatusCode::BAD_REQUEST, "meta"),
        };
        names.push(name.to_string());
        entries.push(VaultEntry {
            name: name.to_string(),
            ciphertext,
            meta,
            version: 0,
            updated: None,
        });
    }
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    if state
        .vault
        .replace_entries(&entries, &state.audit, Some(&session.id), &name_refs)
        .is_err()
    {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    }
    json_value(StatusCode::OK, json!({ "ok": true }))
}

async fn get_versions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.sessions.vault_from_headers(&headers).is_none() {
        return fail_auth();
    }
    let name = headers
        .get("x-secd-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if check_name(name).is_err() {
        return json_status(StatusCode::BAD_REQUEST, "name");
    }
    match state.vault.versions_of(name) {
        Ok(rows) => {
            let versions: Vec<Value> = rows
                .iter()
                .map(|v| {
                    json!({
                        "version": v.seq,
                        "created": v.created,
                        "meta": v.meta,
                    })
                })
                .collect();
            json_value(StatusCode::OK, json!({ "versions": versions }))
        }
        Err(_) => json_status(StatusCode::INTERNAL_SERVER_ERROR, "store"),
    }
}

async fn post_rollback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let Some(session) = state.sessions.vault_from_headers(&headers) else {
        return fail_auth();
    };
    let Some(obj) = body.as_object() else {
        return json_status(StatusCode::BAD_REQUEST, "body");
    };
    if obj
        .keys()
        .any(|k| !matches!(k.as_str(), "name" | "version"))
    {
        return json_status(StatusCode::BAD_REQUEST, "body");
    }
    let Some(name) = obj.get("name").and_then(Value::as_str) else {
        return json_status(StatusCode::BAD_REQUEST, "name");
    };
    if check_name(name).is_err() {
        return json_status(StatusCode::BAD_REQUEST, "name");
    }
    let Some(version) = obj.get("version").and_then(Value::as_i64) else {
        return json_status(StatusCode::BAD_REQUEST, "version");
    };
    if version < 1 {
        return json_status(StatusCode::BAD_REQUEST, "version");
    }
    match state
        .vault
        .rollback(name, version, &state.audit, Some(&session.id))
    {
        Ok(true) => json_value(StatusCode::OK, json!({ "ok": true })),
        Ok(false) => json_status(StatusCode::NOT_FOUND, "version"),
        Err(_) => json_status(StatusCode::INTERNAL_SERVER_ERROR, "store"),
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
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

    async fn body_json(res: Response) -> Value {
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .expect("body");
        serde_json::from_slice(&bytes).expect("json")
    }

    /// `updated` is the newest version's `created`, and null for an entry
    /// that has no version row.
    #[test]
    fn T_VAULT_ENTRY_UPDATED() {
        let dir = fresh_dir("vault-updated");
        let state = AppState::open(&dir).expect("state");
        let (_id, token) = state
            .sessions
            .create_console("op@secd.test")
            .expect("console");

        for ct in ["aa", "bb"] {
            let res = block_on(put_vault(
                State(state.clone()),
                bearer(&token),
                Json(json!({"entries": [{"name": "kv/a", "ciphertext": ct, "meta": {}}]})),
            ));
            assert_eq!(res.status(), StatusCode::OK);
        }
        Db::open(&dir)
            .expect("db")
            .with(|conn| {
                conn.exec(
                    "INSERT INTO entries (name, ciphertext, meta) \
                     VALUES ('kv/legacy', '\"cc\"', '{}')",
                )
            })
            .expect("entry without a version row");

        let mut versions = bearer(&token);
        versions.insert("x-secd-name", HeaderValue::from_static("kv/a"));
        let versions =
            block_on(async { body_json(get_versions(State(state.clone()), versions).await).await });
        let versions = versions["versions"].as_array().expect("versions");
        assert_eq!(versions.len(), 2);
        let newest = versions[1]["created"].as_str().expect("created");
        assert_eq!(
            newest,
            state.vault.versions_of("kv/a").expect("store")[1].created
        );

        let vault = block_on(async {
            body_json(get_vault(State(state.clone()), bearer(&token)).await).await
        });
        let entries = vault["entries"].as_array().expect("entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["name"], json!("kv/a"));
        assert_eq!(entries[0]["version"], json!(2));
        assert_eq!(entries[0]["updated"], json!(newest));
        assert_eq!(entries[1]["name"], json!("kv/legacy"));
        assert_eq!(entries[1]["version"], json!(1));
        assert_eq!(entries[1]["updated"], Value::Null);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
