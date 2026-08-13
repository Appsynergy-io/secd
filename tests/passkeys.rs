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
const PRF: &str = "2222222222222222222222222222222222222222222222222222222222222222";

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
    let dir = std::env::temp_dir().join(format!("secd-t3-pk-{}-{n}", std::process::id()));
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
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(path);
    if let Some(ct) = content_type {
        b = b.header(header::CONTENT_TYPE, ct);
    }
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, format!("__Host-secd={c}"));
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
    )
    .await;
    (s, h, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn get_json(app: &Router, path: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let (s, _, b) = exchange(app, Method::GET, path, None, None, cookie).await;
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

fn put_user(state: &AppState, email: &str, password: Option<&str>, creds: &[&[u8]]) -> String {
    let password = password.map(|p| password_wrap(p).0);
    let passkeys = creds.iter().copied().map(dummy_passkey).collect();
    let user = User {
        id: serde_json::from_value(json!("00000000-0000-4000-8000-000000000001")).expect("uuid"),
        email: email.to_string(),
        password,
        passkeys,
    };
    state.users.put(user).expect("put");
    let (_id, token) = state.sessions.create_console(email);
    token
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
        Self {
            key: openssl::ec::EcKey::generate(&group).expect("ec"),
            cred_id: (1u8..=16).collect(),
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
}

async fn list_len(app: &Router, cookie: &str) -> usize {
    let (s, v) = get_json(app, "/api/auth/passkeys", Some(cookie)).await;
    assert_eq!(s, StatusCode::OK);
    v["passkeys"].as_array().map(Vec::len).unwrap_or(0)
}

#[test]
fn T_PK_LIST_UNAUTH() {
    block_on(async {
        let h = fresh();
        let (s, _) = get_json(&h.app, "/api/auth/passkeys", None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_PK_LIST_SHAPE() {
    block_on(async {
        let h = fresh();
        let cookie = put_user(&h.state, "op@secd.test", Some(PW), &[b"aa"]);
        let (s, v) = get_json(&h.app, "/api/auth/passkeys", Some(&cookie)).await;
        assert_eq!(s, StatusCode::OK);
        let rows = v["passkeys"].as_array().expect("passkeys");
        assert_eq!(rows.len(), 1);
        let obj = rows[0].as_object().expect("row");
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("created"));
        let raw = v.to_string();
        assert!(!raw.contains("prf") && !raw.contains("PRF"));
        assert!(!raw.contains("wraps"));
        assert!(!raw.contains("attestationObject"));
        assert!(!obj.contains_key("wrap"));
        assert!(!obj.contains_key("prf"));
        assert!(!obj.contains_key("passkey"));
    });
}

#[test]
fn T_PK_ADD_WHILE_IN() {
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
        let cookie = cookie_token(&hdrs).expect("cookie");
        let before = list_len(&h.app, &cookie).await;
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "op@secd.test"}),
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let chal = start["publicKey"]["challenge"].as_str().expect("chal");
        let tok = Soft::new();
        let cred = tok.register(chal);
        let (s, _, v) = post_json(
            &h.app,
            "/api/auth/passkey/register/finish",
            &json!({"handle": handle, "credential": cred, "prf": PRF}),
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let after = list_len(&h.app, &cookie).await;
        assert_eq!(after, before + 1);
    });
}

#[test]
fn T_PK_ADD_NO_PRF() {
    block_on(async {
        let h = fresh();
        let cookie = put_user(&h.state, "op@secd.test", Some(PW), &[b"aa"]);
        let before = list_len(&h.app, &cookie).await;
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "op@secd.test"}),
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let (s, _, _) = post_json(
            &h.app,
            "/api/auth/passkey/register/finish",
            &json!({"handle": handle, "credential": {}}),
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(list_len(&h.app, &cookie).await, before);
    });
}

#[test]
fn T_PK_DEL() {
    block_on(async {
        let h = fresh();
        let cookie = put_user(&h.state, "op@secd.test", Some(PW), &[b"aa", b"bb"]);
        let (s, list) = get_json(&h.app, "/api/auth/passkeys", Some(&cookie)).await;
        assert_eq!(s, StatusCode::OK);
        let id = list["passkeys"][0]["id"].as_str().expect("id").to_string();
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/auth/passkeys/{id}"),
            None,
            None,
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(list_len(&h.app, &cookie).await, 1);
        let (s, _, start) = post_json(
            &h.app,
            "/api/auth/passkey/login/start",
            &json!({"email": "op@secd.test"}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let handle = start["handle"].as_str().expect("handle");
        let (s, _, _) = post_json(
            &h.app,
            "/api/auth/passkey/login/finish",
            &json!({
                "handle": handle,
                "prf": PRF,
                "credential": {
                    "id": b64url(b"aa"),
                    "rawId": b64url(b"aa"),
                    "type": "public-key",
                    "response": {
                        "authenticatorData": b64url(&[0u8; 37]),
                        "clientDataJSON": b64url(b"{}"),
                        "signature": b64url(&[0u8; 8])
                    }
                }
            }),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_PK_DEL_LAST_NO_PW() {
    block_on(async {
        let h = fresh();
        let cookie = put_user(&h.state, "op@secd.test", None, &[b"aa"]);
        let (s, list) = get_json(&h.app, "/api/auth/passkeys", Some(&cookie)).await;
        let id = list["passkeys"][0]["id"].as_str().expect("id");
        let (s2, _, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/auth/passkeys/{id}"),
            None,
            None,
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(s2, StatusCode::BAD_REQUEST);
        assert_eq!(list_len(&h.app, &cookie).await, 1);
    });
}

#[test]
fn T_PK_DEL_LAST_WITH_PW() {
    block_on(async {
        let h = fresh();
        let cookie = put_user(&h.state, "op@secd.test", Some(PW), &[b"aa"]);
        let (_, list) = get_json(&h.app, "/api/auth/passkeys", Some(&cookie)).await;
        let id = list["passkeys"][0]["id"].as_str().expect("id");
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/auth/passkeys/{id}"),
            None,
            None,
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
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
fn T_PK_DEL_UNAUTH() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            "/api/auth/passkeys/aa",
            None,
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_PK_DEL_UNKNOWN() {
    block_on(async {
        let h = fresh();
        let cookie = put_user(&h.state, "op@secd.test", Some(PW), &[b"aa"]);
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            "/api/auth/passkeys/ffffffffffffffff",
            None,
            None,
            Some(&cookie),
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    });
}

#[test]
fn T_PK_DEL_FOREIGN() {
    block_on(async {
        let h = fresh();
        let cookie_a = put_user(&h.state, "a@secd.test", Some(PW), &[b"aa"]);
        let stored = password_wrap(PW).0;
        let user_b = User {
            id: serde_json::from_value(json!("00000000-0000-4000-8000-00000000000b"))
                .expect("uuid"),
            email: "b@secd.test".into(),
            password: Some(stored),
            passkeys: vec![dummy_passkey(b"bb")],
        };
        h.state.users.put(user_b).expect("put b");
        let id_b = hex::encode(b"bb");
        let (s, _, _) = exchange(
            &h.app,
            Method::DELETE,
            &format!("/api/auth/passkeys/{id_b}"),
            None,
            None,
            Some(&cookie_a),
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    });
}
