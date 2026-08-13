#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::Router;
use secd_web::auth::{password_wrap, User};
use secd_web::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

const PW: &str = "twelve-chars!";
const EPH: &str = "1111111111111111111111111111111111111111111111111111111111111111";

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
    let dir = std::env::temp_dir().join(format!("secd-t3-sess-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    let state = AppState::open(&dir).expect("open");
    H {
        app: build_app(state.clone()),
        state,
        dir,
    }
}

fn build_app(state: AppState) -> Router {
    Router::new()
        .without_v07_checks()
        .merge(secd_web::auth_routes::router())
        .merge(secd_web::sessions::router())
        .merge(secd_web::device::router())
        .merge(secd_web::vault::router())
        .merge(secd_web::providers_api::router())
        .merge(secd_web::audit::router())
        .merge(secd_web::static_ui::router())
        .fallback(fallback)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), vault_auth))
        .layer(middleware::from_fn_with_state(
            state,
            secd_web::headers::gate,
        ))
}

async fn vault_auth(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let path = req.uri().path();
    if (path == "/api/v1/vault" || path.starts_with("/api/v1/vault/") || path == "/api/v1/audit")
        && state.sessions.vault_from_headers(req.headers()).is_none()
    {
        return secd_web::headers::fail_auth();
    }
    next.run(req).await
}

async fn fallback(req: Request<Body>) -> Response {
    if req.uri().path().starts_with("/api/") {
        return secd_web::headers::json_status(StatusCode::NOT_FOUND, "not found");
    }
    if req.method() != Method::GET {
        return secd_web::headers::json_status(StatusCode::METHOD_NOT_ALLOWED, "method");
    }
    Html("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>secd</title></head><body></body></html>").into_response()
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

async fn login(h: &H, email: &str) -> String {
    let (s, hdrs, _) = post_json(
        &h.app,
        "/api/auth/password/register",
        &json!({"email": email, "password": PW}),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    cookie_token(&hdrs).expect("cookie")
}

async fn approve_device(h: &H, cookie: &str) -> (String, String) {
    let (s, _, start) = post_json(
        &h.app,
        "/api/v1/device/start",
        &json!({"eph_pub": EPH, "device_id": "dev-1", "hostname": "testhost"}),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let code = start["user_code"].as_str().expect("code").to_string();
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
    let token = poll["token"].as_str().expect("token").to_string();
    (code, token)
}

fn row_keys(row: &Value) -> Vec<String> {
    row.as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn T_SESS_UNAUTH() {
    block_on(async {
        let h = fresh();
        let (s, _) = get_json(&h.app, "/api/v1/sessions", None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_SESS_BEARER() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h, "op@secd.test").await;
        let (_, token) = approve_device(&h, &cookie).await;
        let (s, _) = get_json(&h.app, "/api/v1/sessions", None, Some(&token)).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_SESS_LIST() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h, "op@secd.test").await;
        let _ = approve_device(&h, &cookie).await;
        let (s, v) = get_json(&h.app, "/api/v1/sessions", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        let rows = v["sessions"].as_array().expect("sessions");
        assert_eq!(rows.len(), 2);
        let kinds: Vec<_> = rows
            .iter()
            .map(|r| r["kind"].as_str().unwrap_or(""))
            .collect();
        assert!(kinds.contains(&"console"));
        assert!(kinds.contains(&"device"));
        let currents: Vec<_> = rows.iter().filter(|r| r["current"] == true).collect();
        assert_eq!(currents.len(), 1);
        assert_eq!(currents[0]["kind"], "console");
    });
}

#[test]
fn T_SESS_SHAPE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h, "op@secd.test").await;
        let _ = approve_device(&h, &cookie).await;
        let (_, v) = get_json(&h.app, "/api/v1/sessions", Some(&cookie), None).await;
        let raw = v.to_string();
        for banned in ["token", "dek", "sealed_dek", "wraps"] {
            assert!(!raw.contains(banned), "leaked {banned}");
        }
        let allowed = ["id", "kind", "label", "created", "last_seen", "current"];
        for row in v["sessions"].as_array().expect("sessions") {
            let keys = row_keys(row);
            for k in &keys {
                assert!(allowed.contains(&k.as_str()), "extra key {k}");
            }
            for a in allowed {
                assert!(keys.iter().any(|k| k == a), "missing {a}");
            }
        }
    });
}

#[test]
fn T_SESS_REVOKE_OTHER() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h, "op@secd.test").await;
        let (_, token) = approve_device(&h, &cookie).await;
        let (_, list) = get_json(&h.app, "/api/v1/sessions", Some(&cookie), None).await;
        let device_id = list["sessions"]
            .as_array()
            .expect("sessions")
            .iter()
            .find(|r| r["kind"] == "device")
            .and_then(|r| r["id"].as_str())
            .expect("device id")
            .to_string();
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/v1/sessions/{device_id}"),
            None,
            None,
            Some(&cookie),
            None,
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
        let (s, _) = get_json(&h.app, "/api/session", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
    });
}

#[test]
fn T_SESS_REVOKE_SELF() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h, "op@secd.test").await;
        let (_, sess) = get_json(&h.app, "/api/session", Some(&cookie), None).await;
        let id = sess["session_id"].as_str().expect("session_id");
        let (s, hdrs, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/v1/sessions/{id}"),
            None,
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let set = hdrs
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(set.contains("__Host-secd="));
        assert!(set.contains("Max-Age=0"));
        let (s, _) = get_json(&h.app, "/api/session", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_SESS_REVOKE_UNKNOWN() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h, "op@secd.test").await;
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            "/api/v1/sessions/00000000-0000-4000-8000-000000000099",
            None,
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    });
}

#[test]
fn T_SESS_REVOKE_UNAUTH() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            "/api/v1/sessions/00000000-0000-4000-8000-000000000099",
            None,
            None,
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_SESS_REVOKE_FOREIGN() {
    block_on(async {
        let h = fresh();
        let cookie_a = login(&h, "a@secd.test").await;
        let (stored, _) = password_wrap(PW);
        let user_b = User {
            id: serde_json::from_value(json!("00000000-0000-4000-8000-00000000000b"))
                .expect("uuid"),
            email: "b@secd.test".into(),
            password: Some(stored),
            passkeys: vec![],
        };
        h.state.users.put(user_b).expect("put b");
        let (_id_b, token_b) = h.state.sessions.create_console("b@secd.test");
        let (_, list_b) = get_json(&h.app, "/api/v1/sessions", Some(&token_b), None).await;
        let id_b = list_b["sessions"]
            .as_array()
            .expect("b sessions")
            .iter()
            .find(|r| r["current"] == true)
            .and_then(|r| r["id"].as_str())
            .expect("b id")
            .to_string();
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/v1/sessions/{id_b}"),
            None,
            None,
            Some(&cookie_a),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    });
}

#[test]
fn T_SESS_AUDIT() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h, "op@secd.test").await;
        let (_, token) = approve_device(&h, &cookie).await;
        let (_, list) = get_json(&h.app, "/api/v1/sessions", Some(&cookie), None).await;
        let device_id = list["sessions"]
            .as_array()
            .expect("sessions")
            .iter()
            .find(|r| r["kind"] == "device")
            .and_then(|r| r["id"].as_str())
            .expect("device id")
            .to_string();
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/v1/sessions/{device_id}"),
            None,
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let audit = std::fs::read_to_string(h.dir.join("audit.jsonl")).expect("audit.jsonl");
        assert!(audit.contains(&device_id), "audit missing session id");
        assert!(audit.contains("session.revoke"));
        assert!(!audit.contains(&token), "audit leaked token");
        for line in audit.lines() {
            let v: Value = serde_json::from_str(line).expect("audit line");
            assert!(v.get("token").is_none());
        }
    });
}
