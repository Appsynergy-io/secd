use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Json;
use axum::Router;
use secd_core::{check_name, CustomProvider, Field};
use serde_json::{json, Value};

use crate::headers::{fail_auth, json_status, json_value};
use crate::state::AppState;

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_NULL: c_int = 5;
const SQLITE_OPEN_READWRITE: c_int = 0x0000_0002;
const SQLITE_OPEN_CREATE: c_int = 0x0000_0004;
const SQLITE_OPEN_FULLMUTEX: c_int = 0x0001_0000;

const DB_NAME: &str = "secd.db";

#[repr(C)]
struct sqlite3 {
    _private: [u8; 0],
}

#[repr(C)]
struct sqlite3_stmt {
    _private: [u8; 0],
}

#[link(name = "sqlite3")]
extern "C" {
    fn sqlite3_open_v2(
        filename: *const c_char,
        pp_db: *mut *mut sqlite3,
        flags: c_int,
        z_vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut sqlite3) -> c_int;
    fn sqlite3_exec(
        db: *mut sqlite3,
        sql: *const c_char,
        cb: Option<
            unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
        >,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut sqlite3,
        z_sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut sqlite3_stmt,
        pz_tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut sqlite3_stmt,
        i: c_int,
        text: *const c_char,
        n: c_int,
        destructor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn sqlite3_bind_null(stmt: *mut sqlite3_stmt, i: c_int) -> c_int;
    fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_column_text(stmt: *mut sqlite3_stmt, i: c_int) -> *const u8;
    fn sqlite3_column_bytes(stmt: *mut sqlite3_stmt, i: c_int) -> c_int;
    fn sqlite3_column_type(stmt: *mut sqlite3_stmt, i: c_int) -> c_int;
    fn sqlite3_reset(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_errmsg(db: *mut sqlite3) -> *const c_char;
    fn sqlite3_free(p: *mut c_void);
    fn sqlite3_busy_timeout(db: *mut sqlite3, ms: c_int) -> c_int;
    fn sqlite3_changes(db: *mut sqlite3) -> c_int;
}

fn sqlite_transient() -> Option<unsafe extern "C" fn(*mut c_void)> {
    // SQLITE_TRANSIENT: sqlite copies the buffer.
    Some(unsafe { std::mem::transmute::<isize, unsafe extern "C" fn(*mut c_void)>(-1isize) })
}

struct RawConn {
    ptr: *mut sqlite3,
}

// Connection is used only while the Mutex is held.
unsafe impl Send for RawConn {}

impl Drop for RawConn {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: ptr is the handle from sqlite3_open_v2; we own it.
            unsafe {
                sqlite3_close(self.ptr);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

impl RawConn {
    fn errmsg(&self) -> String {
        // SAFETY: ptr is a live sqlite3 opened by us.
        unsafe {
            let p = sqlite3_errmsg(self.ptr);
            if p.is_null() {
                return "sqlite error".into();
            }
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }

    fn exec(&self, sql: &str) -> anyhow::Result<()> {
        let c = CString::new(sql).context("sql nul")?;
        let mut err = ptr::null_mut();
        // SAFETY: c lives for the call; errmsg is freed on the error path.
        let rc = unsafe { sqlite3_exec(self.ptr, c.as_ptr(), None, ptr::null_mut(), &mut err) };
        if rc != SQLITE_OK {
            let msg = if err.is_null() {
                self.errmsg()
            } else {
                // SAFETY: err is a sqlite-allocated string when non-null.
                let s = unsafe { CStr::from_ptr(err) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { sqlite3_free(err.cast()) };
                s
            };
            return Err(anyhow!("sqlite exec: {msg}"));
        }
        if !err.is_null() {
            // SAFETY: unused errmsg still owned by us.
            unsafe { sqlite3_free(err.cast()) };
        }
        Ok(())
    }

    fn prepare(&self, sql: &str) -> anyhow::Result<Stmt> {
        let c = CString::new(sql).context("sql nul")?;
        let mut stmt = ptr::null_mut();
        // SAFETY: c lives for the call; stmt is written on SQLITE_OK.
        let rc =
            unsafe { sqlite3_prepare_v2(self.ptr, c.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != SQLITE_OK || stmt.is_null() {
            return Err(anyhow!("sqlite prepare: {}", self.errmsg()));
        }
        Ok(Stmt { ptr: stmt })
    }
}

struct Stmt {
    ptr: *mut sqlite3_stmt,
}

impl Drop for Stmt {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: ptr is a statement from sqlite3_prepare_v2.
            unsafe {
                sqlite3_finalize(self.ptr);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

impl Stmt {
    fn bind_text(&self, i: c_int, v: &str) -> anyhow::Result<()> {
        // SAFETY: SQLITE_TRANSIENT copies v; i is a 1-based parameter index.
        let rc = unsafe {
            sqlite3_bind_text(
                self.ptr,
                i,
                v.as_ptr().cast(),
                v.len() as c_int,
                sqlite_transient(),
            )
        };
        if rc != SQLITE_OK {
            return Err(anyhow!("sqlite bind"));
        }
        Ok(())
    }

    fn bind_null(&self, i: c_int) -> anyhow::Result<()> {
        // SAFETY: i is a 1-based parameter index on this statement.
        let rc = unsafe { sqlite3_bind_null(self.ptr, i) };
        if rc != SQLITE_OK {
            return Err(anyhow!("sqlite bind"));
        }
        Ok(())
    }

    fn bind_opt(&self, i: c_int, v: Option<&str>) -> anyhow::Result<()> {
        match v {
            Some(s) => self.bind_text(i, s),
            None => self.bind_null(i),
        }
    }

    fn step(&self) -> anyhow::Result<Step> {
        // SAFETY: ptr is a live prepared statement.
        let rc = unsafe { sqlite3_step(self.ptr) };
        match rc {
            SQLITE_ROW => Ok(Step::Row),
            SQLITE_DONE => Ok(Step::Done),
            _ => Err(anyhow!("sqlite step {rc}")),
        }
    }

    fn reset(&self) -> anyhow::Result<()> {
        // SAFETY: ptr is a live prepared statement.
        let rc = unsafe { sqlite3_reset(self.ptr) };
        if rc != SQLITE_OK {
            return Err(anyhow!("sqlite reset"));
        }
        Ok(())
    }

    fn text(&self, i: c_int) -> Option<String> {
        // SAFETY: only valid after SQLITE_ROW; bytes are copied immediately.
        unsafe {
            if sqlite3_column_type(self.ptr, i) == SQLITE_NULL {
                return None;
            }
            let p = sqlite3_column_text(self.ptr, i);
            if p.is_null() {
                return None;
            }
            let n = sqlite3_column_bytes(self.ptr, i) as usize;
            let sl = std::slice::from_raw_parts(p, n);
            Some(String::from_utf8_lossy(sl).into_owned())
        }
    }
}

enum Step {
    Row,
    Done,
}

#[derive(Clone)]
pub(crate) struct Db {
    dir: PathBuf,
    inner: Arc<Mutex<RawConn>>,
}

impl Db {
    pub(crate) fn open(dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(DB_NAME);
        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow!("db path is not utf-8"))?;
        let c_path = CString::new(path_str).context("db path nul")?;
        let mut ptr = ptr::null_mut();
        let flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX;
        // SAFETY: c_path lives for the call; ptr is written on SQLITE_OK.
        let rc = unsafe { sqlite3_open_v2(c_path.as_ptr(), &mut ptr, flags, ptr::null()) };
        if rc != SQLITE_OK || ptr.is_null() {
            if !ptr.is_null() {
                unsafe { sqlite3_close(ptr) };
            }
            return Err(anyhow!("sqlite open {rc}"));
        }
        // SAFETY: ptr is the handle we just opened.
        unsafe {
            sqlite3_busy_timeout(ptr, 5000);
        }
        tighten(dir);
        let raw = RawConn { ptr };
        raw.exec("PRAGMA journal_mode=WAL;")?;
        raw.exec("PRAGMA synchronous=NORMAL;")?;
        raw.exec(SCHEMA)?;
        tighten(dir);
        Ok(Self {
            dir: dir.to_path_buf(),
            inner: Arc::new(Mutex::new(raw)),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RawConn> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn with<T>(&self, f: impl FnOnce(&RawConn) -> anyhow::Result<T>) -> anyhow::Result<T> {
        f(&self.lock())
    }

    fn tighten(&self) {
        tighten(&self.dir);
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entries (
  name TEXT PRIMARY KEY NOT NULL,
  ciphertext TEXT NOT NULL,
  meta TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS wraps (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  factor TEXT NOT NULL,
  cred_id TEXT,
  salt TEXT,
  blob TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS custom_providers (
  name TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL,
  fields TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS audit (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  action TEXT NOT NULL,
  session_id TEXT,
  names TEXT NOT NULL,
  prev_hash TEXT NOT NULL,
  hash TEXT NOT NULL
);
";

fn tighten(dir: &Path) {
    for name in [DB_NAME, "secd.db-wal", "secd.db-shm"] {
        let p = dir.join(name);
        if p.exists() {
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
    }
}

#[derive(Clone)]
pub struct VaultEntry {
    pub name: String,
    pub ciphertext: Value,
    pub meta: Value,
}

#[derive(Clone)]
pub struct VaultStore {
    db: Db,
    dir: PathBuf,
}

impl VaultStore {
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        let db = Db::open(dir)?;
        let store = Self {
            db,
            dir: dir.to_path_buf(),
        };
        store.sync_wraps();
        Ok(store)
    }

    pub(crate) fn db(&self) -> &Db {
        &self.db
    }

    pub fn entries(&self) -> anyhow::Result<Vec<VaultEntry>> {
        self.db.with(|conn| {
            let stmt = conn.prepare("SELECT name, ciphertext, meta FROM entries ORDER BY name")?;
            let mut out = Vec::new();
            loop {
                match stmt.step()? {
                    Step::Done => break,
                    Step::Row => {
                        let name = stmt.text(0).unwrap_or_default();
                        let ct_raw = stmt.text(1).unwrap_or_else(|| "null".into());
                        let meta_raw = stmt.text(2).unwrap_or_else(|| "{}".into());
                        let ciphertext = serde_json::from_str(&ct_raw).unwrap_or(Value::Null);
                        let meta = serde_json::from_str(&meta_raw).unwrap_or_else(|_| json!({}));
                        out.push(VaultEntry {
                            name,
                            ciphertext,
                            meta,
                        });
                    }
                }
            }
            Ok(out)
        })
    }

    pub fn replace_entries(&self, entries: &[VaultEntry]) -> anyhow::Result<()> {
        self.db.with(|conn| {
            conn.exec("BEGIN IMMEDIATE")?;
            let tx = (|| {
                conn.exec("DELETE FROM entries")?;
                let stmt =
                    conn.prepare("INSERT INTO entries (name, ciphertext, meta) VALUES (?, ?, ?)")?;
                for e in entries {
                    stmt.reset()?;
                    stmt.bind_text(1, &e.name)?;
                    stmt.bind_text(2, &e.ciphertext.to_string())?;
                    stmt.bind_text(3, &e.meta.to_string())?;
                    match stmt.step()? {
                        Step::Done => {}
                        Step::Row => return Err(anyhow!("insert returned a row")),
                    }
                }
                Ok(())
            })();
            match tx {
                Ok(()) => conn.exec("COMMIT"),
                Err(e) => {
                    let _ = conn.exec("ROLLBACK");
                    Err(e)
                }
            }
        })?;
        self.db.tighten();
        self.sync_wraps();
        Ok(())
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

    pub fn put_custom_provider(&self, p: &CustomProvider) -> anyhow::Result<()> {
        let fields = serde_json::to_string(&fields_json(&p.fields)).context("fields json")?;
        self.db.with(|conn| {
            let stmt = conn.prepare(
                "INSERT INTO custom_providers (name, title, fields) VALUES (?, ?, ?)
                 ON CONFLICT(name) DO UPDATE SET title=excluded.title, fields=excluded.fields",
            )?;
            stmt.bind_text(1, &p.name)?;
            stmt.bind_text(2, &p.title)?;
            stmt.bind_text(3, &fields)?;
            match stmt.step()? {
                Step::Done => Ok(()),
                Step::Row => Err(anyhow!("upsert returned a row")),
            }
        })?;
        self.db.tighten();
        Ok(())
    }

    pub fn delete_custom_provider(&self, name: &str) -> anyhow::Result<bool> {
        let n = self.db.with(|conn| {
            let stmt = conn.prepare("DELETE FROM custom_providers WHERE name = ?")?;
            stmt.bind_text(1, name)?;
            match stmt.step()? {
                // SAFETY: conn.ptr is the live handle that just ran DELETE.
                Step::Done => Ok(unsafe { sqlite3_changes(conn.ptr) }),
                Step::Row => Err(anyhow!("delete returned a row")),
            }
        })?;
        self.db.tighten();
        Ok(n > 0)
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
            conn.exec("BEGIN IMMEDIATE")?;
            let tx = (|| {
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
            })();
            match tx {
                Ok(()) => conn.exec("COMMIT"),
                Err(e) => {
                    let _ = conn.exec("ROLLBACK");
                    Err(e)
                }
            }
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

pub(crate) fn last_audit_hash(db: &Db) -> anyhow::Result<String> {
    db.with(|conn| {
        let stmt = conn.prepare("SELECT hash FROM audit ORDER BY seq DESC LIMIT 1")?;
        match stmt.step()? {
            Step::Row => Ok(stmt.text(0).unwrap_or_else(zero_hash)),
            Step::Done => Ok(zero_hash()),
        }
    })
}

pub(crate) fn insert_audit(
    db: &Db,
    action: &str,
    session_id: Option<&str>,
    names_json: &str,
    prev_hash: &str,
    hash: &str,
) -> anyhow::Result<()> {
    db.with(|conn| {
        let stmt = conn.prepare(
            "INSERT INTO audit (action, session_id, names, prev_hash, hash) VALUES (?, ?, ?, ?, ?)",
        )?;
        stmt.bind_text(1, action)?;
        stmt.bind_opt(2, session_id)?;
        stmt.bind_text(3, names_json)?;
        stmt.bind_text(4, prev_hash)?;
        stmt.bind_text(5, hash)?;
        match stmt.step()? {
            Step::Done => Ok(()),
            Step::Row => Err(anyhow!("insert returned a row")),
        }
    })?;
    db.tighten();
    Ok(())
}

pub(crate) fn list_audit(db: &Db) -> anyhow::Result<Vec<AuditRow>> {
    db.with(|conn| {
        let stmt = conn
            .prepare("SELECT action, session_id, names, prev_hash, hash FROM audit ORDER BY seq")?;
        let mut out = Vec::new();
        loop {
            match stmt.step()? {
                Step::Done => break,
                Step::Row => {
                    out.push(AuditRow {
                        action: stmt.text(0).unwrap_or_default(),
                        session_id: stmt.text(1),
                        names: stmt.text(2).unwrap_or_else(|| "[]".into()),
                        prev_hash: stmt.text(3).unwrap_or_default(),
                        hash: stmt.text(4).unwrap_or_default(),
                    });
                }
            }
        }
        Ok(out)
    })
}

pub(crate) struct AuditRow {
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
    Router::new().route("/api/v1/vault", get(get_vault).put(put_vault))
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
        });
    }
    if state.vault.replace_entries(&entries).is_err() {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    }
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    state
        .audit
        .record_names("vault.put", Some(&session.id), &name_refs);
    json_value(StatusCode::OK, json!({ "ok": true }))
}
