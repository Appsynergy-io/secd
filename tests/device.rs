#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use secd_web::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

const FAIL: &str = "That email and credential do not match.";
const PW: &str = "twelve-chars!";
const EPH: &str = "1111111111111111111111111111111111111111111111111111111111111111";

static SEQ: AtomicU64 = AtomicU64::new(1);

struct H {
    app: Router,
    dir: PathBuf,
}

impl Drop for H {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fresh() -> H {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("secd-t3-dev-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    let state = AppState::open(&dir).expect("open");
    let app = secd_web::app(state);
    H { app, dir }
}

fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt")
        .block_on(f)
}

fn block_on_paused<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .expect("paused rt")
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
    let v = serde_json::from_slice(&b).unwrap_or(Value::Null);
    (s, h, v)
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

async fn start_ok(h: &H) -> Value {
    let (s, _, v) = post_json(
        &h.app,
        "/api/v1/device/start",
        &json!({
            "eph_pub": EPH,
            "device_id": "dev-1",
            "hostname": "testhost"
        }),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v
}

fn open_sealed(sealed: &Value, secret: &[u8; 32]) -> Option<[u8; 32]> {
    let ct = sealed.get("ct")?.as_str()?;
    let bytes = hex::decode(ct).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = bytes[i] ^ secret[i];
    }
    Some(out)
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
            if b.windows(needle.len()).any(|w| w == needle) {
                *hit = Some(p.to_path_buf());
            }
        }
    }
    let mut hit = None;
    rec(dir, needle, &mut hit);
    hit
}

#[test]
fn T_DEV_START_OK() {
    block_on(async {
        let h = fresh();
        let v = start_ok(&h).await;
        assert!(v.get("user_code").and_then(Value::as_str).is_some());
        assert!(v.get("interval").and_then(Value::as_u64).is_some());
        let uri = v["verification_uri"].as_str().expect("uri");
        assert!(uri.contains("secd.imabee.com"), "{uri}");
    });
}

#[test]
fn T_DEV_START_BAD_PUB() {
    block_on(async {
        let h = fresh();
        let missing = json!({"device_id": "dev-1", "hostname": "testhost"});
        let (s, _, _) = post_json(&h.app, "/api/v1/device/start", &missing, None, None).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let short = json!({
            "eph_pub": "11".repeat(16),
            "device_id": "dev-1",
            "hostname": "testhost"
        });
        let (s, _, _) = post_json(&h.app, "/api/v1/device/start", &short, None, None).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn T_DEV_POLL_UNKNOWN() {
    block_on(async {
        let h = fresh();
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"user_code": "XXXX-YYYY"}),
            None,
            None,
        )
        .await;
        assert!(v.get("token").is_none());
        assert!(s == StatusCode::NOT_FOUND || v.get("status") == Some(&json!("pending")));
    });
}

#[test]
fn T_DEV_POLL_PENDING() {
    block_on(async {
        let h = fresh();
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code");
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"user_code": code}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"status": "pending"}));
        assert!(v.get("token").is_none());
        assert!(v.get("sealed_dek").is_none());
    });
}

#[test]
fn T_DEV_POLL_OK() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code");
        let dek = [0x5eu8; 32];
        let sk_a = [0x11u8; 32];
        let sk_b = [0x22u8; 32];
        let mut ct = [0u8; 32];
        for i in 0..32 {
            ct[i] = dek[i] ^ sk_a[i];
        }
        let sealed = json!({"alg": "x25519-test", "ct": hex::encode(ct)});
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code, "sealed_dek": sealed}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"user_code": code}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["status"], "ok");
        assert!(v["token"].as_str().is_some_and(|t| !t.is_empty()));
        assert!(v.get("sealed_dek").is_some());
        assert!(
            open_sealed(&v["sealed_dek"], &sk_b).is_none() || {
                let opened = open_sealed(&v["sealed_dek"], &sk_b).expect("opened");
                opened != dek
            }
        );
        let opened_wrong = open_sealed(&v["sealed_dek"], &sk_b);
        match opened_wrong {
            None => {}
            Some(pt) => assert_ne!(pt, dek),
        }
    });
}

#[test]
fn T_DEV_POLL_EXPIRED() {
    block_on_paused(async {
        let h = fresh();
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code").to_string();
        tokio::time::advance(Duration::from_secs(10 * 60 + 1)).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"user_code": code}),
            None,
            None,
        )
        .await;
        assert!(v.get("token").is_none());
        let terminal = s == StatusCode::NOT_FOUND
            || v.get("status") == Some(&json!("expired"))
            || v.get("status") == Some(&json!("denied"));
        assert!(terminal, "{s} {v}");
    });
}

#[test]
fn T_DEV_APPROVE_NO_SESSION() {
    block_on(async {
        let h = fresh();
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code");
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code, "sealed_dek": {"x": 1}}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v, json!({"error": FAIL}));
    });
}

#[test]
fn T_DEV_APPROVE_BAD_CODE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": "NOPE-CODE", "sealed_dek": {"x": 1}}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v, json!({"error": FAIL}));
    });
}

#[test]
fn T_DEV_APPROVE_DOUBLE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code");
        let body = json!({"user_code": code, "sealed_dek": {"x": 1}});
        let (s, _, _) =
            post_json(&h.app, "/api/v1/device/approve", &body, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        let (s, _, _) =
            post_json(&h.app, "/api/v1/device/approve", &body, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn T_DEV_SERVER_NO_DEK() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code");
        let dek = [
            0xde, 0xad, 0xc0, 0xde, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa,
            0xbb, 0xcc, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
            0x0d, 0x0e, 0x0f, 0x10,
        ];
        let sealed = json!({"box": "opaque", "n": 1});
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code, "sealed_dek": sealed}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        if let Some(p) = contains_bytes(&h.dir, &dek) {
            panic!("DEK plaintext in {}", p.display());
        }
        let hexed = hex::encode(dek);
        if let Some(p) = contains_bytes(&h.dir, hexed.as_bytes()) {
            panic!("DEK hex in {}", p.display());
        }
    });
}

#[test]
fn T_DEV_REVOKE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code");
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code, "sealed_dek": {"x": 1}}),
            Some(&cookie),
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
        let token = poll["token"].as_str().expect("token").to_string();
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/revoke",
            &json!({}),
            None,
            Some(&token),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, _, _) = exchange(
            &h.app,
            Method::GET,
            "/api/v1/vault",
            None,
            None,
            None,
            Some(&token),
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_DEV_REVOKE_BAD() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = post_json(&h.app, "/api/v1/device/revoke", &json!({}), None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/revoke",
            &json!({}),
            None,
            Some("not-a-token"),
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}
