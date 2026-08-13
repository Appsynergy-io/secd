#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::Router;
use secd_core::{unwrap_passkey, Factor, Wrap};
use secd_web::auth::{password_wrap, User};
use secd_web::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

const FAIL: &str = "That email and credential do not match.";
const FAIL_JSON: &str = r#"{"error":"That email and credential do not match."}"#;
const RATE_JSON: &str = r#"{"error":"Too many attempts. Wait a minute."}"#;
const PW: &str = "twelve-chars!";
const PW_LOG: &str = "pw-log-unique-Zx9q";

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
    let dir = std::env::temp_dir().join(format!("secd-t3-auth-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    let state = AppState::open(&dir).expect("AppState::open");
    let app = build_app(state.clone());
    H { app, state, dir }
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
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let (s, h, b) = exchange(
        app,
        Method::POST,
        path,
        Some(body.to_string().into_bytes()),
        Some("application/json"),
        cookie,
        None,
    )
    .await;
    let v = serde_json::from_slice(&b).unwrap_or(Value::Null);
    (s, h, v)
}

async fn get_json(
    app: &Router,
    path: &str,
    cookie: Option<&str>,
    bearer: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let (s, h, b) = exchange(app, Method::GET, path, None, None, cookie, bearer).await;
    let v = serde_json::from_slice(&b).unwrap_or(Value::Null);
    (s, h, v)
}

fn cookie_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(header::SET_COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("__Host-secd=") {
            if v.is_empty() {
                return None;
            }
            return Some(v.to_string());
        }
    }
    None
}

fn assert_fail_sentence(status: StatusCode, raw: &[u8]) {
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(raw, FAIL_JSON.as_bytes());
    let v: Value = serde_json::from_slice(raw).expect("json");
    assert_eq!(v, json!({"error": FAIL}));
}

fn dummy_passkey(cred: &[u8]) -> secd_web::auth::StoredPasskey {
    let id = hex::encode(cred);
    let cred_b64 = b64url(cred);
    let zeros = b64url(&[0u8; 32]);
    serde_json::from_value(json!({
        "id": id,
        "created": "2020-01-01T00:00:00Z",
        "passkey": {
            "cred": {
                "cred_id": cred_b64,
                "cred": {
                    "type_": "ES256",
                    "key": {
                        "EC_EC2": {
                            "curve": "SECP256R1",
                            "x": zeros,
                            "y": zeros
                        }
                    }
                },
                "counter": 0,
                "transports": null,
                "user_verified": true,
                "backup_eligible": false,
                "backup_state": false,
                "registration_policy": "required",
                "extensions": {},
                "attestation": { "data": "None", "metadata": "None" },
                "attestation_format": "none"
            }
        },
        "wrap": {
            "factor": "passkey",
            "cred_id": id,
            "blob": "00".repeat(72)
        }
    }))
    .expect("fixture StoredPasskey")
}

fn put_user(state: &AppState, email: &str, password: Option<&str>, creds: &[&[u8]]) {
    let password = password.map(|p| password_wrap(p).0);
    let passkeys = creds.iter().copied().map(dummy_passkey).collect();
    let user = User {
        id: serde_json::from_value(json!("00000000-0000-4000-8000-000000000001")).expect("uuid"),
        email: email.to_string(),
        password,
        passkeys,
    };
    state.users.put(user).expect("put user");
}

fn b64url(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < data.len() {
            out.push(T[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char);
        }
        if i + 2 < data.len() {
            out.push(T[(b2 & 63) as usize] as char);
        }
        i += 3;
    }
    out
}

fn cbor_hdr(major: u8, n: u64) -> Vec<u8> {
    if n < 24 {
        vec![(major << 5) | n as u8]
    } else if n < 256 {
        vec![(major << 5) | 24, n as u8]
    } else {
        vec![(major << 5) | 25, (n >> 8) as u8, n as u8]
    }
}

fn cbor_text(s: &str) -> Vec<u8> {
    let mut o = cbor_hdr(3, s.len() as u64);
    o.extend_from_slice(s.as_bytes());
    o
}

fn cbor_bytes(b: &[u8]) -> Vec<u8> {
    let mut o = cbor_hdr(2, b.len() as u64);
    o.extend_from_slice(b);
    o
}

struct Soft {
    key: openssl::ec::EcKey<openssl::pkey::Private>,
    cred_id: Vec<u8>,
    counter: u32,
}

impl Soft {
    fn new() -> Self {
        let group = openssl::ec::EcGroup::from_curve_name(openssl::nid::Nid::X9_62_PRIME256V1)
            .expect("p256");
        let key = openssl::ec::EcKey::generate(&group).expect("ec key");
        Self {
            key,
            cred_id: (0u8..16).collect(),
            counter: 1,
        }
    }

    fn xy(&self) -> ([u8; 32], [u8; 32]) {
        let group = self.key.group();
        let mut ctx = openssl::bn::BigNumContext::new().expect("ctx");
        let mut x = openssl::bn::BigNum::new().expect("x");
        let mut y = openssl::bn::BigNum::new().expect("y");
        self.key
            .public_key()
            .affine_coordinates_gfp(group, &mut x, &mut y, &mut ctx)
            .expect("xy");
        let mut xb = [0u8; 32];
        let mut yb = [0u8; 32];
        let xv = x.to_vec();
        let yv = y.to_vec();
        xb[32 - xv.len()..].copy_from_slice(&xv);
        yb[32 - yv.len()..].copy_from_slice(&yv);
        (xb, yb)
    }

    fn sign(&self, data: &[u8]) -> Vec<u8> {
        let pkey = openssl::pkey::PKey::from_ec_key(self.key.clone()).expect("pkey");
        let mut signer =
            openssl::sign::Signer::new(openssl::hash::MessageDigest::sha256(), &pkey).expect("s");
        signer.update(data).expect("upd");
        signer.sign_to_vec().expect("sig")
    }

    fn register(&self, challenge_b64: &str) -> Value {
        let (x, y) = self.xy();
        let mut cose = cbor_hdr(5, 5);
        cose.push(0x01);
        cose.push(0x02);
        cose.push(0x03);
        cose.push(0x26);
        cose.push(0x20);
        cose.push(0x01);
        cose.push(0x21);
        cose.extend(cbor_bytes(&x));
        cose.push(0x22);
        cose.extend(cbor_bytes(&y));
        let rp = openssl::sha::sha256(b"secd.imabee.com");
        let mut auth = Vec::new();
        auth.extend_from_slice(&rp);
        auth.push(0x45);
        auth.extend_from_slice(&self.counter.to_be_bytes());
        auth.extend_from_slice(&[0u8; 16]);
        auth.extend_from_slice(&(self.cred_id.len() as u16).to_be_bytes());
        auth.extend_from_slice(&self.cred_id);
        auth.extend_from_slice(&cose);
        let mut ao = cbor_hdr(5, 3);
        ao.extend(cbor_text("fmt"));
        ao.extend(cbor_text("none"));
        ao.extend(cbor_text("attStmt"));
        ao.push(0xa0);
        ao.extend(cbor_text("authData"));
        ao.extend(cbor_bytes(&auth));
        let cdata = format!(
            r#"{{"type":"webauthn.create","challenge":"{challenge_b64}","origin":"https://secd.imabee.com","tokenBinding":null}}"#
        );
        let id = b64url(&self.cred_id);
        json!({
            "id": id,
            "rawId": id,
            "type": "public-key",
            "response": {
                "attestationObject": b64url(&ao),
                "clientDataJSON": b64url(cdata.as_bytes()),
            }
        })
    }

    fn assert(&mut self, challenge_b64: &str) -> Value {
        self.counter += 1;
        let rp = openssl::sha::sha256(b"secd.imabee.com");
        let mut auth = Vec::new();
        auth.extend_from_slice(&rp);
        auth.push(0x05);
        auth.extend_from_slice(&self.counter.to_be_bytes());
        let cdata = format!(
            r#"{{"type":"webauthn.get","challenge":"{challenge_b64}","origin":"https://secd.imabee.com","tokenBinding":null}}"#
        );
        let mut msg = auth.clone();
        msg.extend_from_slice(&openssl::sha::sha256(cdata.as_bytes()));
        let sig = self.sign(&msg);
        let id = b64url(&self.cred_id);
        json!({
            "id": id,
            "rawId": id,
            "type": "public-key",
            "response": {
                "authenticatorData": b64url(&auth),
                "clientDataJSON": b64url(cdata.as_bytes()),
                "signature": b64url(&sig),
            }
        })
    }
}

const PRF: &str = "2222222222222222222222222222222222222222222222222222222222222222";

async fn register_password(h: &H, email: &str, password: &str) -> String {
    let (s, hdrs, _) = post_json(
        &h.app,
        "/api/auth/password/register",
        &json!({"email": email, "password": password}),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    cookie_token(&hdrs).expect("cookie")
}

#[test]
fn T_AUTH_START_EMPTY() {
    block_on(async {
        let h = fresh();
        for body in [json!({}), json!({"email": ""})] {
            let (s, _, v) = post_json(&h.app, "/api/auth/start", &body, None).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        }
    });
}

#[test]
fn T_AUTH_START_BAD_EMAIL() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "not-an-email"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn T_AUTH_START_EMPTY_SERVER() {
    block_on(async {
        let h = fresh();
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"method": "register"}));
    });
}

#[test]
fn T_AUTH_START_PASSKEY_ONLY() {
    block_on(async {
        let h = fresh();
        put_user(&h.state, "op@secd.test", None, &[b"aa"]);
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["method"], "passkey");
        let raw = v.to_string();
        assert!(!raw.contains("password"), "{raw}");
        assert!(!raw.contains("either"), "{raw}");
    });
}

#[test]
fn T_AUTH_START_PASSWORD_ONLY() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"method": "password"}));
    });
}

#[test]
fn T_AUTH_START_BOTH() {
    block_on(async {
        let h = fresh();
        put_user(&h.state, "op@secd.test", Some(PW), &[b"aa"]);
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"method": "either"}));
    });
}

#[test]
fn T_AUTH_START_UNKNOWN() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "nobody@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["method"], "passkey");
        let raw = v.to_string();
        assert!(!raw.contains("password"));
        assert!(!raw.contains("either"));
    });
}

#[test]
fn T_AUTH_START_CASE() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "A@B.C", PW).await;
        assert!(h.state.users.get("a@b.c").is_some());
        let (s, _, v) =
            post_json(&h.app, "/api/auth/start", &json!({"email": "A@B.C"}), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v, json!({"method": "password"}));
    });
}

#[test]
fn T_AUTH_START_RATE() {
    block_on(async {
        let h = fresh();
        let body = json!({"email": "op@secd.test"});
        for i in 0..10 {
            let (s, _, _) = post_json(&h.app, "/api/auth/start", &body, None).await;
            assert_eq!(s, StatusCode::OK, "req {i}");
        }
        let (s, _, v) = post_json(&h.app, "/api/auth/start", &body, None).await;
        assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(v, serde_json::from_str::<Value>(RATE_JSON).expect("rate"));
    });
}

#[test]
fn T_AUTH_START_CT() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/start",
            Some(br#"{"email":"op@secd.test"}"#.to_vec()),
            Some("text/plain"),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn T_AUTH_PK_REG_NONEMPTY_NEW() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let (s, _, raw_v) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "new@secd.test"}),
            None,
        )
        .await;
        let raw = serde_json::to_vec(&raw_v).unwrap_or_default();
        if s == StatusCode::UNAUTHORIZED {
            assert_eq!(raw_v, json!({"error": FAIL}));
        } else {
            let (_, _, b) = exchange(
                &h.app,
                Method::POST,
                "/api/auth/passkey/register/start",
                Some(json!({"email": "new@secd.test"}).to_string().into_bytes()),
                Some("application/json"),
                None,
                None,
            )
            .await;
            assert_fail_sentence(s, &b);
        }
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(raw_v, json!({"error": FAIL}));
        let _ = raw;
    });
}

#[test]
fn T_AUTH_PK_REG_NO_PRF() {
    block_on(async {
        let h = fresh();
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let (s, _, _) = post_json(
            &h.app,
            "/api/auth/passkey/register/finish",
            &json!({"handle": handle, "credential": {}}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert!(h.state.users.is_empty());
    });
}

#[test]
fn T_AUTH_PK_REG_BAD_HANDLE() {
    block_on(async {
        let h = fresh();
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/passkey/register/finish",
            &json!({"handle": "deadbeef", "credential": {}, "prf": PRF}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v, json!({"error": FAIL}));
    });
}

#[test]
fn T_AUTH_PK_REG_EXPIRED() {
    block_on_paused(async {
        let h = fresh();
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle").to_string();
        tokio::time::advance(Duration::from_secs(5 * 60 + 1)).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/passkey/register/finish",
            &json!({"handle": handle, "credential": {}, "prf": PRF}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v, json!({"error": FAIL}));
    });
}

#[test]
fn T_AUTH_PK_REG_REPLAY() {
    block_on(async {
        let h = fresh();
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let body = json!({"handle": handle, "credential": {}, "prf": PRF});
        let _ = post_json(&h.app, "/api/auth/passkey/register/finish", &body, None).await;
        let (s, _, v) = post_json(&h.app, "/api/auth/passkey/register/finish", &body, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v, json!({"error": FAIL}));
    });
}

#[test]
fn T_AUTH_PK_REG_CROSS() {
    block_on(async {
        let h = fresh();
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "a@b.c"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/passkey/register/finish",
            &json!({
                "handle": handle,
                "email": "other@x.y",
                "credential": {},
                "prf": PRF
            }),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v, json!({"error": FAIL}));
    });
}

#[test]
fn T_AUTH_PK_LOGIN_BAD() {
    block_on(async {
        let h = fresh();
        put_user(&h.state, "op@secd.test", None, &[b"aa"]);
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/login/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let (s, hdrs, raw) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/passkey/login/finish",
            Some(
                json!({"handle": handle, "credential": {}, "prf": PRF})
                    .to_string()
                    .into_bytes(),
            ),
            Some("application/json"),
            None,
            None,
        )
        .await;
        assert_fail_sentence(s, &raw);
        assert!(cookie_token(&hdrs).is_none());
        let unknown = json!({"email": "nobody@secd.test", "password": PW});
        let (s2, _, raw2) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/password/login",
            Some(unknown.to_string().into_bytes()),
            Some("application/json"),
            None,
            None,
        )
        .await;
        assert_eq!(raw, raw2);
        assert_eq!(s2, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_AUTH_PK_LOGIN_NO_PRF() {
    block_on(async {
        let h = fresh();
        let (s, hdrs, raw) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/passkey/login/finish",
            Some(
                json!({"handle": "x", "credential": {}})
                    .to_string()
                    .into_bytes(),
            ),
            Some("application/json"),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert!(cookie_token(&hdrs).is_none());
        let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
        assert!(v.get("wraps").is_none());
    });
}

#[test]
fn T_AUTH_PK_LOGIN_WRAPS_ARE_CIPHER() {
    block_on(async {
        let h = fresh();
        let mut tok = Soft::new();
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let chal = start["publicKey"]["challenge"].as_str().expect("chal");
        let cred = tok.register(chal);
        let (s, hdrs, _) = post_json(
            &h.app,
            "/api/auth/passkey/register/finish",
            &json!({"handle": handle, "credential": cred, "prf": PRF}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "register finish");
        let cookie = cookie_token(&hdrs).expect("cookie");
        let _ = post_json(&h.app, "/api/auth/logout", &json!({}), Some(&cookie)).await;
        let (s, _, login) = post_json(
            &h.app,
            "/api/auth/passkey/login/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = login["handle"].as_str().expect("handle");
        let chal = login["publicKey"]["challenge"].as_str().expect("chal");
        let cred = tok.assert(chal);
        let (s, hdrs, body) = post_json(
            &h.app,
            "/api/auth/passkey/login/finish",
            &json!({"handle": handle, "credential": cred, "prf": PRF}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert!(cookie_token(&hdrs).is_some());
        let wraps = body["wraps"].as_array().expect("wraps");
        assert!(!wraps.is_empty());
        assert!(body.get("dek").is_none());
        for w in wraps {
            let blob = w["blob"].as_str().expect("blob");
            let bytes = hex::decode(blob).expect("hex blob");
            assert_ne!(bytes.len(), 32, "blob is not a raw DEK");
            let wrap = Wrap {
                factor: Factor::Passkey,
                cred_id: w["cred_id"].as_str().map(str::to_string),
                salt: w["salt"].as_str().map(str::to_string),
                blob: blob.to_string(),
            };
            assert!(unwrap_passkey(&wrap, &[0u8; 32]).is_err());
        }
    });
}

#[test]
fn T_AUTH_PW_SHORT() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = post_json(
            &h.app,
            "/api/auth/password/register",
            &json!({"email": "op@secd.test", "password": "12345678901"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn T_AUTH_PW_LONG() {
    block_on(async {
        let h = fresh();
        let long = "a".repeat(257);
        let (s, _, _) = post_json(
            &h.app,
            "/api/auth/password/register",
            &json!({"email": "op@secd.test", "password": long}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn T_AUTH_PW_REG_NONEMPTY() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/password/register",
            &json!({"email": "new@secd.test", "password": PW}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v, json!({"error": FAIL}));
    });
}

#[test]
fn T_AUTH_PW_WRONG() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let (s, _, raw) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/password/login",
            Some(
                json!({"email": "op@secd.test", "password": "wrong-password!"})
                    .to_string()
                    .into_bytes(),
            ),
            Some("application/json"),
            None,
            None,
        )
        .await;
        assert_fail_sentence(s, &raw);
    });
}

#[test]
fn T_AUTH_PW_UNKNOWN() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let (s, _, raw) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/password/login",
            Some(
                json!({"email": "nobody@secd.test", "password": PW})
                    .to_string()
                    .into_bytes(),
            ),
            Some("application/json"),
            None,
            None,
        )
        .await;
        assert_fail_sentence(s, &raw);
        let (s2, _, raw2) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/password/login",
            Some(
                json!({"email": "op@secd.test", "password": "wrong-password!"})
                    .to_string()
                    .into_bytes(),
            ),
            Some("application/json"),
            None,
            None,
        )
        .await;
        assert_eq!(s2, StatusCode::UNAUTHORIZED);
        assert_eq!(raw, raw2);
    });
}

#[test]
fn T_AUTH_PW_TIMING() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let t0 = Instant::now();
        let _ = post_json(
            &h.app,
            "/api/auth/password/login",
            &json!({"email": "nobody@secd.test", "password": PW}),
            None,
        )
        .await;
        let dummy = t0.elapsed();
        let floor = dummy / 4;
        let t1 = Instant::now();
        let (s_u, _, _) = post_json(
            &h.app,
            "/api/auth/password/login",
            &json!({"email": "ghost@secd.test", "password": PW}),
            None,
        )
        .await;
        let unknown = t1.elapsed();
        let t2 = Instant::now();
        let (s_w, _, _) = post_json(
            &h.app,
            "/api/auth/password/login",
            &json!({"email": "op@secd.test", "password": "wrong-password!"}),
            None,
        )
        .await;
        let wrong = t2.elapsed();
        assert_eq!(s_u, StatusCode::UNAUTHORIZED);
        assert_eq!(s_w, StatusCode::UNAUTHORIZED);
        assert!(unknown >= floor, "unknown returned too fast");
        assert!(wrong >= floor, "wrong returned too fast");
    });
}

#[test]
fn T_AUTH_PW_RATE() {
    block_on(async {
        let h = fresh();
        let body = json!({"email": "op@secd.test", "password": PW});
        for _ in 0..10 {
            let _ = post_json(&h.app, "/api/auth/password/login", &body, None).await;
        }
        let (s, _, v) = post_json(&h.app, "/api/auth/password/login", &body, None).await;
        assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(v, serde_json::from_str::<Value>(RATE_JSON).expect("rate"));
    });
}

#[test]
fn T_AUTH_PW_LOG() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW_LOG).await;
        let (s, _, _) = post_json(
            &h.app,
            "/api/auth/password/login",
            &json!({"email": "op@secd.test", "password": PW_LOG}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        fn walk(p: &std::path::Path, needle: &str) {
            if p.is_dir() {
                if let Ok(rd) = std::fs::read_dir(p) {
                    for e in rd.flatten() {
                        walk(&e.path(), needle);
                    }
                }
                return;
            }
            let Ok(bytes) = std::fs::read(p) else {
                return;
            };
            if bytes.windows(needle.len()).any(|w| w == needle.as_bytes()) {
                panic!("password substring present in {}", p.display());
            }
        }
        walk(&h.dir, PW_LOG);
    });
}

#[test]
fn T_AUTH_SESSION_NONE() {
    block_on(async {
        let h = fresh();
        let (s, _, raw) =
            exchange(&h.app, Method::GET, "/api/session", None, None, None, None).await;
        assert_fail_sentence(s, &raw);
    });
}

#[test]
fn T_AUTH_SESSION_OK() {
    block_on(async {
        let h = fresh();
        let cookie = register_password(&h, "op@secd.test", PW).await;
        let (s, _, v) = get_json(&h.app, "/api/session", Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["email"], "op@secd.test");
        assert_eq!(v["has_passkey"], false);
        assert_eq!(v["has_password"], true);
        assert!(v.get("wraps").is_none());
        assert!(v.get("dek").is_none());
        let raw = v.to_string();
        assert!(!raw.contains("dek"));
    });
}

#[test]
fn T_AUTH_LOGOUT() {
    block_on(async {
        let h = fresh();
        let cookie = register_password(&h, "op@secd.test", PW).await;
        let (s, hdrs, _) = post_json(&h.app, "/api/auth/logout", &json!({}), Some(&cookie)).await;
        assert_eq!(s, StatusCode::OK);
        let set = hdrs
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(set.contains("__Host-secd="));
        assert!(set.contains("Max-Age=0"));
        let (s, _, raw) = exchange(
            &h.app,
            Method::GET,
            "/api/session",
            None,
            None,
            Some(&cookie),
            None,
        )
        .await;
        assert_fail_sentence(s, &raw);
    });
}

#[test]
fn T_AUTH_COOKIE_FLAGS() {
    block_on(async {
        let h = fresh();
        let (s, hdrs, _) = post_json(
            &h.app,
            "/api/auth/password/register",
            &json!({"email": "op@secd.test", "password": PW}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let set = hdrs
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .expect("Set-Cookie");
        assert!(set.starts_with("__Host-secd="), "{set}");
        assert!(set.contains("Secure"), "{set}");
        assert!(set.contains("HttpOnly"), "{set}");
        assert!(set.contains("SameSite=Lax"), "{set}");
        assert!(set.contains("Path=/"), "{set}");
        let lower = set.to_ascii_lowercase();
        assert!(
            !lower.split(';').any(|p| p.trim().starts_with("domain=")),
            "{set}"
        );
    });
}

#[test]
fn T_AUTH_NO_CORS() {
    block_on(async {
        let h = fresh();
        let (_, hdrs, _) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert!(hdrs.get("access-control-allow-origin").is_none());
        for name in hdrs.keys() {
            assert!(
                !name.as_str().starts_with("access-control-"),
                "cors header {}",
                name
            );
        }
    });
}

#[test]
fn T_AUTH_FAIL_SENTENCE() {
    block_on(async {
        let h = fresh();
        let _ = register_password(&h, "op@secd.test", PW).await;
        let cases: Vec<(Method, &str, Option<Value>)> = vec![
            (Method::GET, "/api/session", None),
            (
                Method::POST,
                "/api/auth/password/login",
                Some(json!({"email": "op@secd.test", "password": "wrong-password!"})),
            ),
            (
                Method::POST,
                "/api/auth/password/login",
                Some(json!({"email": "ghost@secd.test", "password": PW})),
            ),
            (
                Method::POST,
                "/api/auth/passkey/register/start",
                Some(json!({"email": "new@secd.test"})),
            ),
            (
                Method::POST,
                "/api/auth/passkey/register/finish",
                Some(json!({"handle": "nope", "credential": {}, "prf": PRF})),
            ),
        ];
        for (method, path, body) in cases {
            let raw_body = body.map(|v| v.to_string().into_bytes());
            let ct = raw_body.as_ref().map(|_| "application/json");
            let (s, _, raw) = exchange(&h.app, method, path, raw_body, ct, None, None).await;
            assert_fail_sentence(s, &raw);
        }
    });
}
