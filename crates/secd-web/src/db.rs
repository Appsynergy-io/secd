//! The one sqlite handle. Vault entries, the audit chain and sessions all
//! live in `secd.db`; AppState::open clones this Arc into each store.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context};

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

use libsqlite3_sys as _;

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
    fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, i: c_int, v: i64) -> c_int;
    fn sqlite3_bind_null(stmt: *mut sqlite3_stmt, i: c_int) -> c_int;
    fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_column_text(stmt: *mut sqlite3_stmt, i: c_int) -> *const u8;
    fn sqlite3_column_int64(stmt: *mut sqlite3_stmt, i: c_int) -> i64;
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

pub(crate) struct RawConn {
    ptr: *mut sqlite3,
}

// Connection is used only while the Mutex is held.
unsafe impl Send for RawConn {}

/// ROLLBACK on drop unless disarmed after COMMIT. A panic in `f` must not
/// leave the IMMEDIATE transaction open for the next `with()` caller.
struct RollbackOnDrop<'a> {
    conn: &'a RawConn,
    armed: bool,
}

impl Drop for RollbackOnDrop<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.conn.exec("ROLLBACK");
        }
    }
}

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

    /// One BEGIN IMMEDIATE. ROLLBACK if `f` fails or panics, or if COMMIT
    /// fails, so the next `with()` caller cannot see a half-done mutation.
    pub(crate) fn immediate<T>(&self, f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
        self.exec("BEGIN IMMEDIATE")?;
        let mut guard = RollbackOnDrop {
            conn: self,
            armed: true,
        };
        let v = f()?;
        match self.exec("COMMIT") {
            Ok(()) => {
                guard.armed = false;
                Ok(v)
            }
            Err(e) => {
                let _ = self.exec("ROLLBACK");
                guard.armed = false;
                Err(e)
            }
        }
    }

    pub(crate) fn exec(&self, sql: &str) -> anyhow::Result<()> {
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

    pub(crate) fn prepare(&self, sql: &str) -> anyhow::Result<Stmt> {
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

    /// Rows changed by the most recent statement on this connection.
    pub(crate) fn changes(&self) -> i64 {
        // SAFETY: ptr is the live handle that ran the statement.
        i64::from(unsafe { sqlite3_changes(self.ptr) })
    }
}

pub(crate) struct Stmt {
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
    pub(crate) fn bind_text(&self, i: c_int, v: &str) -> anyhow::Result<()> {
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

    pub(crate) fn bind_i64(&self, i: c_int, v: i64) -> anyhow::Result<()> {
        // SAFETY: i is a 1-based parameter index on this statement.
        let rc = unsafe { sqlite3_bind_int64(self.ptr, i, v) };
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

    pub(crate) fn bind_opt(&self, i: c_int, v: Option<&str>) -> anyhow::Result<()> {
        match v {
            Some(s) => self.bind_text(i, s),
            None => self.bind_null(i),
        }
    }

    pub(crate) fn step(&self) -> anyhow::Result<Step> {
        // SAFETY: ptr is a live prepared statement.
        let rc = unsafe { sqlite3_step(self.ptr) };
        match rc {
            SQLITE_ROW => Ok(Step::Row),
            SQLITE_DONE => Ok(Step::Done),
            _ => Err(anyhow!("sqlite step {rc}")),
        }
    }

    /// A statement that must return no row.
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        match self.step()? {
            Step::Done => Ok(()),
            Step::Row => Err(anyhow!("statement returned a row")),
        }
    }

    pub(crate) fn reset(&self) -> anyhow::Result<()> {
        // SAFETY: ptr is a live prepared statement.
        let rc = unsafe { sqlite3_reset(self.ptr) };
        if rc != SQLITE_OK {
            return Err(anyhow!("sqlite reset"));
        }
        Ok(())
    }

    pub(crate) fn text(&self, i: c_int) -> Option<String> {
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

    pub(crate) fn i64_at(&self, i: c_int) -> i64 {
        // SAFETY: only valid after SQLITE_ROW.
        unsafe { sqlite3_column_int64(self.ptr, i) }
    }
}

pub(crate) enum Step {
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

    pub(crate) fn with<T>(
        &self,
        f: impl FnOnce(&RawConn) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        f(&self.lock())
    }

    pub(crate) fn tighten(&self) {
        tighten(&self.dir);
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entries (
  name TEXT PRIMARY KEY NOT NULL,
  ciphertext TEXT NOT NULL,
  meta TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS versions (
  name TEXT NOT NULL,
  seq INTEGER NOT NULL,
  ciphertext TEXT NOT NULL,
  meta TEXT NOT NULL,
  created TEXT NOT NULL,
  PRIMARY KEY (name, seq)
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
CREATE TABLE IF NOT EXISTS sessions (
  token_hash TEXT PRIMARY KEY NOT NULL,
  id TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL,
  kind TEXT NOT NULL,
  label TEXT NOT NULL,
  created INTEGER NOT NULL,
  last_seen INTEGER NOT NULL,
  expires INTEGER NOT NULL
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use std::panic::AssertUnwindSafe;
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

    fn entry_count(conn: &RawConn) -> i64 {
        let stmt = conn.prepare("SELECT COUNT(*) FROM entries").expect("count");
        match stmt.step().expect("step") {
            Step::Row => stmt.i64_at(0),
            Step::Done => 0,
        }
    }

    fn open_raw(path: &Path) -> RawConn {
        let path_str = path.to_str().expect("utf-8");
        let c_path = CString::new(path_str).expect("nul");
        let mut ptr = ptr::null_mut();
        let flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX;
        let rc = unsafe { sqlite3_open_v2(c_path.as_ptr(), &mut ptr, flags, ptr::null()) };
        assert!(rc == SQLITE_OK && !ptr.is_null(), "open raw {rc}");
        RawConn { ptr }
    }

    fn insert_entry(conn: &RawConn, name: &str) -> anyhow::Result<()> {
        let stmt =
            conn.prepare("INSERT INTO entries (name, ciphertext, meta) VALUES (?, 'c', 'm')")?;
        stmt.bind_text(1, name)?;
        stmt.run()
    }

    /// A panic inside `f` must ROLLBACK: the next `with()` caller cannot
    /// start a nested transaction, and the insert is gone.
    #[test]
    fn T_TXN_PANIC_ROLLS_BACK() {
        let dir = fresh_dir("txn-panic");
        let db = Db::open(&dir.0).expect("open");
        let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = db.with(|conn| {
                conn.immediate(|| -> anyhow::Result<()> {
                    insert_entry(conn, "gone")?;
                    panic!("txn panic");
                })
            });
        }));
        assert!(caught.is_err(), "f must panic");
        db.with(|conn| {
            conn.immediate(|| Ok(()))?;
            assert_eq!(entry_count(conn), 0, "panicked insert must not persist");
            Ok(())
        })
        .expect("connection usable after panic");
    }

    /// A COMMIT that fails must ROLLBACK on this connection, not wait for
    /// sqlite3_close. DELETE journal + a held SHARED lock makes COMMIT
    /// return SQLITE_BUSY while the IMMEDIATE transaction is still open.
    #[test]
    fn T_TXN_COMMIT_FAIL_ROLLS_BACK() {
        let dir = fresh_dir("txn-commit");
        let db = Db::open(&dir.0).expect("open");
        db.with(|conn| {
            conn.exec("PRAGMA journal_mode=DELETE")?;
            conn.exec("PRAGMA busy_timeout=0")?;
            insert_entry(conn, "keep")
        })
        .expect("seed");

        let reader = open_raw(&dir.0.join(DB_NAME));
        reader
            .exec("PRAGMA busy_timeout=0")
            .expect("reader timeout");
        reader.exec("BEGIN").expect("reader begin");
        let hold = reader.prepare("SELECT name FROM entries").expect("hold");
        assert!(
            matches!(hold.step().expect("row"), Step::Row),
            "reader must hold SHARED"
        );

        let err = db.with(|conn| {
            conn.exec("PRAGMA busy_timeout=0")?;
            conn.immediate(|| insert_entry(conn, "gone"))
        });
        assert!(err.is_err(), "COMMIT must fail while SHARED is held");

        // Same connection, still inside the Mutex, with the reader holding
        // SHARED: a skipped ROLLBACK would show the insert here.
        db.with(|conn| {
            assert_eq!(
                entry_count(conn),
                1,
                "uncommitted insert must not be visible on this connection"
            );
            Ok::<_, anyhow::Error>(())
        })
        .expect("count");
        drop(hold);
        drop(reader);

        db.with(|conn| {
            conn.exec("PRAGMA busy_timeout=5000")?;
            conn.immediate(|| Ok(()))?;
            Ok(())
        })
        .expect("connection usable after COMMIT fail");
    }
}
