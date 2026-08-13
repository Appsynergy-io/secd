#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use secd_core::{open, seal, unwrap_password, wrap_password, Factor, Wrap};
use secd_web::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

const PW: &str = "twelve-chars!";
const EPH: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const FIXTURE: &[u8] = b"t4-fixture-value-do-not-print";
const BUILTIN: &[&str] = &[
    "cloudflare",
    "aws",
    "s3",
    "github",
    "gitea",
    "gitlab",
    "slack",
    "digitalocean",
    "npm",
    "xai",
    "sendgrid",
    "pypi",
    "anthropic",
    "openai",
    "vault",
];

static SEQ: AtomicU64 = AtomicU64::new(1);

struct H {
    app: Router,
    state: AppState,
    dir: PathBuf,
}

impl Drop for H {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fresh() -> H {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("secd-t4-vault-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    let state = AppState::open(&dir).expect("open");
    H {
        app: secd_web::app(state.clone()),
        state,
        dir,
    }
}

fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt")
        .block_on(f)
}

async fn exchange(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Vec<u8>>,
    content_type: Option<&str>,
    cookie: Option<&str>,
    bearer: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(path);
    if let Some(ct) = content_type {
        b = b.header(header::CONTENT_TYPE, ct);
    }
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("__Host-secd={c}"));
    }
    if let Some(t) = bearer {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = b
        .body(Body::from(body.unwrap_or_default()))
        .expect("request");
    let res = app.clone().oneshot(req).await.expect("oneshot");
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = to_bytes(res.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    (status, headers, bytes.to_vec())
}

async fn post_json(
    app: &Router,
    path: &str,
    body: &Value,
    cookie: Option<&str>,
    bearer: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let (s, h, b) = exchange(
        app,
        Method::POST,
        path,
        Some(body.to_string().into_bytes()),
        Some("application/json"),
        cookie,
        bearer,
    )
    .await;
    (s, h, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn put_json(
    app: &Router,
    path: &str,
    body: &Value,
    cookie: Option<&str>,
    bearer: Option<&str>,
) -> (StatusCode, Value) {
    let (s, _, b) = exchange(
        app,
        Method::PUT,
        path,
        Some(body.to_string().into_bytes()),
        Some("application/json"),
        cookie,
        bearer,
    )
    .await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn get_json(
    app: &Router,
    path: &str,
    cookie: Option<&str>,
    bearer: Option<&str>,
) -> (StatusCode, Value) {
    let (s, _, b) = exchange(app, Method::GET, path, None, None, cookie, bearer).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

fn cookie_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(header::SET_COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("__Host-secd=") {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

async fn login(h: &H) -> String {
    let (s, hdrs, _) = post_json(
        &h.app,
        "/api/auth/password/register",
        &json!({"email": "op@secd.test", "password": PW}),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    cookie_token(&hdrs).expect("cookie")
}

async fn approve_device(h: &H, cookie: &str) -> String {
    let (s, _, start) = post_json(
        &h.app,
        "/api/v1/device/start",
        &json!({"eph_pub": EPH, "device_id": "dev-1", "hostname": "testhost"}),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let code = start["user_code"].as_str().expect("code");
    let (s, _, _) = post_json(
        &h.app,
        "/api/v1/device/approve",
        &json!({"user_code": code, "sealed_dek": {"x": 1}}),
        Some(cookie),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, _, poll) = post_json(
        &h.app,
        "/api/v1/device/poll",
        &json!({"user_code": code}),
        None,
        None,
    )
    .await;
    poll["token"].as_str().expect("token").to_string()
}

fn json_has_key(v: &Value, key: &str) -> bool {
    match v {
        Value::Object(m) => m.contains_key(key) || m.values().any(|x| json_has_key(x, key)),
        Value::Array(a) => a.iter().any(|x| json_has_key(x, key)),
        _ => false,
    }
}

fn assert_no_secret_keys(v: &Value) {
    assert!(!json_has_key(v, "value"));
    assert!(!json_has_key(v, "plaintext"));
    assert!(!json_has_key(v, "dek"));
}

fn contains_bytes(dir: &Path, needle: &[u8]) -> Option<PathBuf> {
    fn rec(p: &Path, needle: &[u8], hit: &mut Option<PathBuf>) {
        if hit.is_some() {
            return;
        }
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                for e in rd.flatten() {
                    rec(&e.path(), needle, hit);
                }
            }
            return;
        }
        if let Ok(b) = std::fs::read(p) {
            if !needle.is_empty() && b.windows(needle.len()).any(|w| w == needle) {
                *hit = Some(p.to_path_buf());
            }
        }
    }
    let mut hit = None;
    rec(dir, needle, &mut hit);
    hit
}

fn wrap_from_json(v: &Value) -> Wrap {
    Wrap {
        factor: if v["factor"].as_str() == Some("passkey") {
            Factor::Passkey
        } else {
            Factor::Password
        },
        cred_id: v["cred_id"].as_str().map(str::to_string),
        salt: v["salt"].as_str().map(str::to_string),
        blob: v["blob"].as_str().unwrap_or("").to_string(),
    }
}

#[test]
fn T_VAULT_GET_UNAUTH() {
    block_on(async {
        let h = fresh();
        let (s, _) = get_json(&h.app, "/api/v1/vault", None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_VAULT_GET_EMPTY() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"entries": []}));
    });
}

#[test]
fn T_VAULT_GET_SHAPE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let dek = [0x11u8; 32];
        let blob = seal(&dek, "kv/shape", FIXTURE).expect("seal");
        let ct = hex::encode(blob);
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"kv/shape","ciphertext":ct,"meta":{"k":1}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_no_secret_keys(&v);
        let entries = v["entries"].as_array().expect("entries");
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert!(e.get("name").and_then(Value::as_str).is_some());
        assert!(e.get("ciphertext").is_some());
        assert!(e.get("meta").map(Value::is_object).unwrap_or(false));
        assert_no_secret_keys(e);
    });
}

#[test]
fn T_VAULT_PUT_UNAUTH() {
    block_on(async {
        let h = fresh();
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"n","ciphertext":"aa","meta":{}}]}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let cookie = login(&h).await;
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"entries": []}));
    });
}

#[test]
fn T_VAULT_PUT_GET() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let dek = [0x22u8; 32];
        let blob = seal(&dek, "kv/round", FIXTURE).expect("seal");
        let ct = hex::encode(&blob);
        let body = json!({"entries":[{"name":"kv/round","ciphertext":ct,"meta":{"n":2}}]});
        let (s, _) = put_json(&h.app, "/api/v1/vault", &body, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        let got = &v["entries"][0]["ciphertext"];
        assert_eq!(got, &json!(ct));
        let got_hex = got.as_str().expect("ct string");
        assert_eq!(got_hex.as_bytes(), ct.as_bytes());
        let opened = open(&dek, "kv/round", &hex::decode(got_hex).expect("hex")).expect("open");
        assert!(opened.as_bytes() == FIXTURE);
    });
}

#[test]
fn T_VAULT_PUT_PLAINTEXT_FIELD() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"value":"x","entries":[]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"n","ciphertext":"aa","meta":{},"value":"x"}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"entries": []}));
    });
}

#[test]
fn T_VAULT_PUT_BAD_NAME() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"../x","ciphertext":"aa","meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"entries": []}));
    });
}

#[test]
fn T_VAULT_PUT_SQL() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let sql = "'; drop table entries;--";
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":sql,"ciphertext":"aa","meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert!(
            s == StatusCode::BAD_REQUEST || s == StatusCode::OK,
            "sql name status"
        );
        let (s2, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"kv/still-here","ciphertext":"bb","meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s2, StatusCode::OK);
        let (s3, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s3, StatusCode::OK);
        let names: Vec<&str> = v["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert!(names.contains(&"kv/still-here"), "entries table survived");
    });
}

#[test]
fn T_VAULT_AAD() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let dek = [0x33u8; 32];
        let blob = seal(&dek, "alpha", FIXTURE).expect("seal");
        let ct = hex::encode(blob);
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"alpha","ciphertext":ct,"meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (_, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        let got = v["entries"][0]["ciphertext"].as_str().expect("ct");
        let bytes = hex::decode(got).expect("hex");
        assert!(open(&dek, "beta", &bytes).is_err());
        let opened = open(&dek, "alpha", &bytes).expect("open A");
        assert!(opened.as_bytes() == FIXTURE);
    });
}

#[test]
fn T_VAULT_BEARER_AND_COOKIE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let token = approve_device(&h, &cookie).await;
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(v.get("entries").is_some());
        let (s, v) = get_json(&h.app, "/api/v1/vault", None, Some(&token)).await;
        assert_eq!(s, StatusCode::OK);
        assert!(v.get("entries").is_some());
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/revoke",
            &json!({}),
            None,
            Some(&token),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, _) = get_json(&h.app, "/api/v1/vault", None, Some(&token)).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, _) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
    });
}

#[test]
fn T_VAULT_NO_DECRYPT_WITHOUT_KEK() {
    block_on(async {
        let h = fresh();
        let dek = [0x44u8; 32];
        let local_wrap = wrap_password(&dek, PW.as_bytes()).expect("wrap");
        let blob = seal(&dek, "kv/fixture", FIXTURE).expect("seal");
        let ct = hex::encode(&blob);
        let (s, hdrs, body) = post_json(
            &h.app,
            "/api/auth/password/register",
            &json!({"email": "op@secd.test", "password": PW}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let cookie = cookie_token(&hdrs).expect("cookie");
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"kv/fixture","ciphertext":ct,"meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, v) = get_json(&h.app, "/api/v1/vault", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_no_secret_keys(&v);
        let raw = v.to_string();
        assert!(
            !raw.as_bytes().windows(FIXTURE.len()).any(|w| w == FIXTURE),
            "GET leaked fixture"
        );
        if let Some(p) = contains_bytes(&h.dir, FIXTURE) {
            panic!("fixture plaintext in {}", p.display());
        }
        assert!(open(&[0u8; 32], "kv/fixture", &blob).is_err());
        assert!(unwrap_password(&local_wrap, b"not-the-password!").is_err());
        if let Some(arr) = body.get("wraps").and_then(Value::as_array) {
            for w in arr {
                let wrap = wrap_from_json(w);
                assert!(unwrap_password(&wrap, b"not-the-password!").is_err());
                let blob_hex = w["blob"].as_str().unwrap_or("");
                assert_ne!(blob_hex.len(), 64, "wrap blob is not a raw DEK");
            }
        }
    });
}

#[test]
fn T_PROV_GET_BUILTIN() {
    block_on(async {
        let h = fresh();
        let (s, v) = get_json(&h.app, "/api/v1/providers", None, None).await;
        assert_eq!(s, StatusCode::OK);
        assert_no_secret_keys(&v);
        let rows = v["providers"].as_array().expect("providers");
        let builtins: Vec<&Value> = rows
            .iter()
            .filter(|p| p["builtin"] == true || p.get("builtin").is_none())
            .collect();
        assert_eq!(builtins.len(), 15);
        let names: Vec<&str> = builtins.iter().filter_map(|p| p["name"].as_str()).collect();
        for n in BUILTIN {
            assert!(names.contains(n), "missing builtin");
        }
        for p in &builtins {
            assert_no_secret_keys(p);
            let fields = p["fields"].as_array().expect("fields");
            for f in fields {
                assert!(f.get("key").is_some());
                assert!(f.get("env").is_some());
                assert!(f.get("value").is_none());
            }
        }
    });
}

#[test]
fn T_PROV_PUT_CUSTOM() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let schema = json!({
            "name": "acme",
            "title": "Acme",
            "fields": [{"key": "token", "secret": true, "env": "ACME_TOKEN"}]
        });
        let (s, _) = put_json(&h.app, "/api/v1/providers", &schema, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        let (s, v) = get_json(&h.app, "/api/v1/providers", None, None).await;
        assert_eq!(s, StatusCode::OK);
        let rows = v["providers"].as_array().expect("providers");
        let acme = rows.iter().find(|p| p["name"] == "acme").expect("acme");
        assert_eq!(acme["title"], "Acme");
        assert_eq!(acme["builtin"], false);
        assert_eq!(acme["fields"][0]["key"], "token");
        assert_eq!(acme["fields"][0]["env"], "ACME_TOKEN");
        assert!(acme["fields"][0].get("value").is_none());
    });
}

#[test]
fn T_PROV_DEL_CUSTOM() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let schema = json!({
            "name": "acme",
            "title": "Acme",
            "fields": [{"key": "token", "env": "ACME_TOKEN"}]
        });
        let (s, _) = put_json(&h.app, "/api/v1/providers", &schema, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            "/api/v1/providers/acme",
            None,
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (_, v) = get_json(&h.app, "/api/v1/providers", None, None).await;
        let rows = v["providers"].as_array().expect("providers");
        assert!(rows.iter().all(|p| p["name"] != "acme"));
    });
}

#[test]
fn T_PROV_DEL_BUILTIN() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            "/api/v1/providers/gitea",
            None,
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (_, v) = get_json(&h.app, "/api/v1/providers", None, None).await;
        let names: Vec<&str> = v["providers"]
            .as_array()
            .expect("providers")
            .iter()
            .filter_map(|p| p["name"].as_str())
            .collect();
        assert!(names.contains(&"gitea"));
    });
}

#[test]
fn T_PROV_PUT_MALFORMED() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        for body in [
            json!({"name":"acme","title":"Acme","fields":[{}]}),
            json!({"name":"acme","title":"Acme","fields":[{"key":"token"}]}),
            json!({"name":"acme","title":"Acme","fields":[{"env":"ACME_TOKEN"}]}),
        ] {
            let (s, _) = put_json(&h.app, "/api/v1/providers", &body, Some(&cookie), None).await;
            assert_eq!(s, StatusCode::BAD_REQUEST);
        }
    });
}

#[test]
fn T_AUDIT_NO_VALUE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let dek = [0x55u8; 32];
        let blob = seal(&dek, "kv/audited", FIXTURE).expect("seal");
        let ct = hex::encode(blob);
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"kv/audited","ciphertext":ct,"meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, v) = get_json(&h.app, "/api/v1/audit", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_no_secret_keys(&v);
        let events = v["events"].as_array().expect("events");
        assert!(!events.is_empty());
        let raw = v.to_string();
        assert!(
            !raw.as_bytes().windows(FIXTURE.len()).any(|w| w == FIXTURE),
            "audit leaked fixture"
        );
        for ev in events {
            assert!(ev.get("action").and_then(Value::as_str).is_some());
            assert!(ev.get("names").map(Value::is_array).unwrap_or(false));
            assert!(ev.get("value").is_none());
            assert!(ev.get("ciphertext").is_none());
        }
        if let Some(p) = contains_bytes(&h.dir.join("audit.jsonl"), FIXTURE) {
            panic!("fixture in {}", p.display());
        }
    });
}

#[test]
fn T_AUDIT_CHAIN() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"kv/a","ciphertext":"aa","meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, _) = put_json(
            &h.app,
            "/api/v1/vault",
            &json!({"entries":[{"name":"kv/b","ciphertext":"bb","meta":{}}]}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert!(h.state.audit.verify(), "chain must verify before tamper");
        let path = h.dir.join("audit.jsonl");
        let raw = std::fs::read_to_string(&path).expect("audit.jsonl");
        assert!(!raw.is_empty());
        let tampered = raw.replacen("vault.put", "vault.tamper", 1);
        assert_ne!(tampered, raw);
        std::fs::write(&path, tampered).expect("write tamper");
        assert!(!h.state.audit.verify(), "tamper one row must fail verify");
    });
}

#[test]
fn T_AUDIT_UNAUTH() {
    block_on(async {
        let h = fresh();
        let (s, _) = get_json(&h.app, "/api/v1/audit", None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}
