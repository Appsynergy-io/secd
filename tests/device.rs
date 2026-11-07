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

async fn get_json(app: &Router, path: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let (s, _, b) = exchange(app, Method::GET, path, None, None, cookie, None).await;
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

fn pw_wrap_json(dek: &[u8; 32]) -> Value {
    let w = secd_core::wrap_password(dek, PW.as_bytes()).expect("wrap");
    json!({"factor": "password", "salt": w.salt.expect("salt"), "blob": w.blob})
}

async fn login(h: &H) -> String {
    let (s, hdrs, _) = post_json(
        &h.app,
        "/api/auth/password/register",
        &json!({"email": "op@secd.test", "password": PW, "wrap": pw_wrap_json(&[0x44; 32])}),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    cookie_token(&hdrs).expect("cookie")
}

/// Browser-side seal: X25519 to the device pub, raw shared secret as the
/// XChaCha key, AAD "dek" — the exact shape `secd login` unseals.
fn seal_to(eph_hex: &str, dek: &[u8; 32]) -> Value {
    let their: [u8; 32] = hex::decode(eph_hex)
        .expect("hex")
        .try_into()
        .expect("32 bytes");
    let secret = x25519_dalek::StaticSecret::random();
    let public = x25519_dalek::PublicKey::from(&secret);
    let shared = secret.diffie_hellman(&x25519_dalek::PublicKey::from(their));
    let blob = secd_core::seal(shared.as_bytes(), "dek", dek).expect("seal");
    json!({
        "alg": "x25519-xchacha20poly1305",
        "eph_pub": hex::encode(public.as_bytes()),
        "blob": hex::encode(blob),
    })
}

async fn start_ok(h: &H) -> Value {
    let (s, _, v) = post_json(
        &h.app,
        "/api/v1/device/start",
        &json!({"client_id": "secd-cli", "scope": "openid email",
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

fn looks_rfc3339(s: &str) -> bool {
    let Some((date, rest)) = s.split_once('T') else {
        return false;
    };
    date.len() == 10 && (rest.ends_with('Z') || rest.contains('+'))
}

fn lowercase_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn open_sealed(sealed: &Value, secret: &x25519_dalek::StaticSecret) -> Option<[u8; 32]> {
    let eph: [u8; 32] = hex::decode(sealed.get("eph_pub")?.as_str()?)
        .ok()?
        .try_into()
        .ok()?;
    let blob = hex::decode(sealed.get("blob")?.as_str()?).ok()?;
    let shared = secret.diffie_hellman(&x25519_dalek::PublicKey::from(eph));
    let pt = secd_core::open(shared.as_bytes(), "dek", &blob).ok()?;
    pt.as_bytes().try_into().ok()
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
        assert!(uri.contains("/device"), "{uri}");
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
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": "XXXX-YYYY"}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v, json!({"error": "expired_token"}));
    });
}

#[test]
fn T_DEV_POLL_PENDING() {
    block_on(async {
        let h = fresh();
        let start = start_ok(&h).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": start["device_code"]}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v, json!({"error": "authorization_pending"}));
        assert!(v.get("access_token").is_none());
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
        let device = x25519_dalek::StaticSecret::random();
        let device_pub = hex::encode(x25519_dalek::PublicKey::from(&device).as_bytes());
        let sealed = seal_to(&device_pub, &dek);
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
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": start["device_code"]}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["token_type"], "Bearer");
        assert!(v["access_token"].as_str().is_some_and(|t| !t.is_empty()));
        assert!(v.get("sealed_dek").is_some());
        let wrong = x25519_dalek::StaticSecret::random();
        match open_sealed(&v["sealed_dek"], &wrong) {
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
        tokio::time::advance(Duration::from_secs(10 * 60 + 1)).await;
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": start["device_code"]}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v, json!({"error": "expired_token"}));
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
        let body = json!({"user_code": code, "sealed_dek": seal_to(EPH, &[0x21; 32])});
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
        let sealed = seal_to(EPH, &dek);
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
            &json!({"user_code": code, "sealed_dek": seal_to(EPH, &[0x22; 32])}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (_, _, poll) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": start["device_code"]}),
            None,
            None,
        )
        .await;
        let token = poll["access_token"].as_str().expect("token").to_string();
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
fn T_DEV_SEAL_E2E() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        // CLI side: fresh ephemeral key pair, code from /device/start.
        let device = x25519_dalek::StaticSecret::random();
        let device_pub = hex::encode(x25519_dalek::PublicKey::from(&device).as_bytes());
        let (s, _, start) = post_json(
            &h.app,
            "/api/v1/device/start",
            &json!({"client_id": "secd-cli", "scope": "openid email", "eph_pub": device_pub, "device_id": "dev-e2e", "hostname": "e2ehost"}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let code = start["user_code"].as_str().expect("code");
        // Browser side: seal the vault DEK to the device pub from the URL.
        let dek = [0x7au8; 32];
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code, "sealed_dek": seal_to(&device_pub, &dek)}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        // CLI side: poll and unseal with the ephemeral secret.
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": start["device_code"]}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["token_type"], "Bearer");
        let sealed = &v["sealed_dek"];
        let eph = sealed["eph_pub"].as_str().expect("eph_pub");
        assert_eq!(eph.len(), 64, "{eph}");
        let blob = sealed["blob"].as_str().expect("blob");
        assert!(blob.len() >= 80, "{blob}");
        let opened = open_sealed(sealed, &device).expect("unseal");
        assert_eq!(opened, dek);
    });
}

#[test]
fn T_DEV_APPROVE_BAD_SEALED() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code").to_string();
        let good = seal_to(EPH, &[0x33; 32]);
        let eph_hex = good["eph_pub"].as_str().expect("eph").to_string();
        let blob_hex = good["blob"].as_str().expect("blob").to_string();
        let bad: Vec<Value> = vec![
            json!({"x": 1}),
            json!({"eph_pub": eph_hex}),
            json!({"eph_pub": eph_hex.to_uppercase(), "blob": blob_hex}),
            json!({"eph_pub": &eph_hex[..63], "blob": blob_hex}),
            json!({"eph_pub": eph_hex, "blob": "zz".repeat(40)}),
            json!({"eph_pub": eph_hex, "blob": "ab".repeat(39)}),
        ];
        for sealed in bad {
            let (s, _, v) = post_json(
                &h.app,
                "/api/v1/device/approve",
                &json!({"user_code": code, "sealed_dek": sealed}),
                Some(&cookie),
                None,
            )
            .await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        }
        // The code survives every rejected shape and still approves.
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code, "sealed_dek": good}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    });
}

#[test]
fn T_DEVICE_PENDING_LIST() {
    block_on(async {
        let h = fresh();
        let eph_a = "aa".repeat(32);
        let eph_b = "BB".repeat(32);
        let (s, _, a) = post_json(
            &h.app,
            "/api/v1/device/start",
            &json!({"client_id": "secd-cli", "scope": "openid email", "eph_pub": eph_a, "device_id": "dev-a", "hostname": "host-a"}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{a}");
        let code_a = a["user_code"].as_str().expect("code a").to_string();
        let (s, _, b) = post_json(
            &h.app,
            "/api/v1/device/start",
            &json!({"client_id": "secd-cli", "scope": "openid email", "eph_pub": eph_b, "device_id": "dev-b", "hostname": "host-b"}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let code_b = b["user_code"].as_str().expect("code b").to_string();

        let (s, v) = get_json(&h.app, "/api/v1/device/pending", None).await;
        assert!(
            s == StatusCode::UNAUTHORIZED || s == StatusCode::FORBIDDEN,
            "{s} {v}"
        );

        let cookie = login(&h).await;
        let (s, v) = get_json(&h.app, "/api/v1/device/pending", Some(&cookie)).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let rows = v["pending"].as_array().expect("pending");
        assert_eq!(rows.len(), 2, "{v}");
        assert_eq!(rows[0]["user_code"], code_a);
        assert_eq!(rows[0]["hostname"], "host-a");
        assert_eq!(rows[1]["user_code"], code_b);
        assert_eq!(rows[1]["hostname"], "host-b");
        for (row, want_eph) in [(&rows[0], "aa".repeat(32)), (&rows[1], "bb".repeat(32))] {
            let eph = row["eph_pub"].as_str().expect("eph_pub");
            assert!(lowercase_hex64(eph), "{eph}");
            assert_eq!(eph, want_eph);
            let created = row["created"].as_str().expect("created");
            assert!(looks_rfc3339(created), "{created}");
            let expires = row["expires_in"].as_u64().expect("expires_in");
            assert!(expires > 0 && expires <= 600, "{expires}");
        }

        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code_a, "sealed_dek": seal_to(&eph_a, &[0x21; 32])}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, v) = get_json(&h.app, "/api/v1/device/pending", Some(&cookie)).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let rows = v["pending"].as_array().expect("pending");
        assert_eq!(rows.len(), 1, "{v}");
        assert_eq!(rows[0]["user_code"], code_b);
        assert_eq!(rows[0]["hostname"], "host-b");
    });
}

#[test]
fn T_DEVICE_DENY() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().expect("code").to_string();

        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/deny",
            &json!({"user_code": code}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v, json!({"ok": true}));

        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": code, "sealed_dek": seal_to(EPH, &[0x21; 32])}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        assert_eq!(v, json!({"error": "denied"}));

        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": start["device_code"]}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        assert_eq!(v, json!({"error": "access_denied"}));

        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": start["device_code"]}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        assert_eq!(
            v,
            json!({"error": "expired_token"}),
            "a denial is reported once"
        );

        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/deny",
            &json!({"user_code": "XXXX-YYYY"}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
        assert_eq!(v, json!({"error": "not found"}));
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

#[test]
fn T_DEV_POLL_PRIVATE_CODE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let code = start["user_code"].as_str().unwrap();
        let private = start["device_code"].as_str().unwrap();
        assert_eq!(private.len(), 64);
        assert_ne!(private, code);
        let (_, pending) = get_json(&h.app, "/api/v1/device/pending", Some(&cookie)).await;
        assert!(!pending.to_string().contains(private));
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code":code,"sealed_dek":seal_to(EPH, &[0x21;32])}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        for body in [
            json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "user_code":code}),
            json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": code}),
            json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": "00".repeat(32)}),
        ] {
            let (s, _, v) = post_json(&h.app, "/api/v1/device/poll", &body, None, None).await;
            assert!(s.is_client_error());
            assert!(v.get("access_token").is_none());
        }
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/poll",
            &json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "client_id": "secd-cli", "device_code": private}),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["token_type"], "Bearer");
    });
}

const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

async fn poll(h: &H, device_code: &Value) -> (StatusCode, Value) {
    let (s, _, v) = post_json(
        &h.app,
        "/api/v1/device/poll",
        &json!({"grant_type": GRANT, "client_id": "secd-cli", "device_code": device_code}),
        None,
        None,
    )
    .await;
    (s, v)
}

/// RFC 8628 §3.4: a token request names the device_code grant; anything else
/// is `unsupported_grant_type`, and a missing code `invalid_request`.
#[test]
fn T_DEV_POLL_GRANT_TYPE() {
    block_on(async {
        let h = fresh();
        let start = start_ok(&h).await;
        for grant in [json!(null), json!("refresh_token"), json!("device_code")] {
            let body = json!({"grant_type": grant, "client_id": "secd-cli", "device_code": start["device_code"]});
            let (s, _, v) = post_json(&h.app, "/api/v1/device/poll", &body, None, None).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "{grant}");
            assert_eq!(v, json!({"error": "unsupported_grant_type"}), "{grant}");
        }
        let body = json!({"grant_type": GRANT, "client_id": "secd-cli"});
        let (s, _, v) = post_json(&h.app, "/api/v1/device/poll", &body, None, None).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v, json!({"error": "invalid_request"}));
    });
}

/// An approved session is released exactly once: the request is consumed by
/// the response that carries the token, so the device code cannot mint a
/// second copy of the session.
#[test]
fn T_DEV_POLL_ONCE() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let start = start_ok(&h).await;
        let (s, _, _) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code": start["user_code"], "sealed_dek": seal_to(EPH, &[0x31; 32])}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, v) = poll(&h, &start["device_code"]).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["token_type"], "Bearer");
        assert!(v["access_token"].as_str().is_some_and(|t| !t.is_empty()));
        let (s, v) = poll(&h, &start["device_code"]).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v, json!({"error": "expired_token"}));
    });
}

/// RFC 8628 §3.5: a client polling faster than its interval gets `slow_down`
/// and five more seconds; polling at the new interval is pending again.
#[test]
fn T_DEV_POLL_SLOW_DOWN() {
    block_on_paused(async {
        let h = fresh();
        let start = start_ok(&h).await;
        assert_eq!(start["interval"], json!(5));
        let (_, v) = poll(&h, &start["device_code"]).await;
        assert_eq!(
            v,
            json!({"error": "authorization_pending"}),
            "the first poll is never early"
        );
        tokio::time::advance(Duration::from_secs(1)).await;
        let (s, v) = poll(&h, &start["device_code"]).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(v, json!({"error": "slow_down", "interval": 10}));
        tokio::time::advance(Duration::from_secs(5)).await;
        let (_, v) = poll(&h, &start["device_code"]).await;
        assert_eq!(
            v["error"], "slow_down",
            "five seconds is early once the interval is ten"
        );
        assert_eq!(v["interval"], 15);
        tokio::time::advance(Duration::from_secs(15)).await;
        let (_, v) = poll(&h, &start["device_code"]).await;
        assert_eq!(v, json!({"error": "authorization_pending"}));
    });
}

#[test]
fn native_oidc_code_requires_pkce_and_is_single_use() {
    block_on(async {
        let h = fresh();
        let cookie = login(&h).await;
        let verifier = "a".repeat(43);
        let redirect = "http://127.0.0.1:49152/oidc/callback";
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", "secd-cli")
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect)
            .append_pair("scope", "openid email")
            .append_pair("nonce", "request-nonce")
            .append_pair("state", "client-state")
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &secd_core::jwt::pkce_challenge(&verifier))
            .append_pair("eph_pub", EPH)
            .append_pair("hostname", "native-client")
            .finish();
        let (status, headers, _) = exchange(
            &h.app,
            Method::GET,
            &format!("/api/oidc/authorize?{query}"),
            None,
            None,
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        let location = headers[header::LOCATION].to_str().unwrap();
        let target = url::Url::parse(&format!("https://secd.test{location}")).unwrap();
        let public_code = target
            .query_pairs()
            .find(|(k, _)| k == "code")
            .unwrap()
            .1
            .into_owned();
        let (status, _, approved) = post_json(
            &h.app,
            "/api/v1/device/approve",
            &json!({"user_code":public_code,"sealed_dek":seal_to(EPH,&[0x31;32])}),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let target = url::Url::parse(approved["redirect_uri"].as_str().unwrap()).unwrap();
        let params: std::collections::HashMap<_, _> = target.query_pairs().into_owned().collect();
        assert_eq!(params["state"], "client-state");
        let mut request = json!({"client_id":"secd-cli", "grant_type":"authorization_code",
            "code":params["code"], "redirect_uri":redirect, "code_verifier":"b".repeat(43)});
        let (status, _, body) =
            post_json(&h.app, "/api/v1/device/poll", &request, None, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_grant");
        request["code_verifier"] = json!(verifier);
        let form = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(
                request
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str().unwrap())),
            )
            .finish();
        let (status, headers, body) = exchange(
            &h.app,
            Method::POST,
            "/api/v1/device/poll",
            Some(form.into_bytes()),
            Some("application/x-www-form-urlencoded"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        assert!(headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store"));
        let token: Value = serde_json::from_slice(&body).unwrap();
        let (_, _, keys) = exchange(
            &h.app,
            Method::GET,
            "/.well-known/jwks.json",
            None,
            None,
            None,
            None,
        )
        .await;
        let keys: Value = serde_json::from_slice(&keys).unwrap();
        let expect = secd_core::jwt::Expect {
            issuer: &params["iss"],
            audience: "secd-cli",
            nonce: Some("request-nonce"),
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };
        let claims =
            secd_core::jwt::verify(token["id_token"].as_str().unwrap(), &keys, &expect).unwrap();
        assert_eq!(claims["email"], "op@secd.test");
        assert_eq!(
            claims["at_hash"],
            secd_core::jwt::token_hash(token["access_token"].as_str().unwrap())
        );
        let (status, _, body) =
            post_json(&h.app, "/api/v1/device/poll", &request, None, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_grant");
        let bad = query.replace("127.0.0.1", "evil.test");
        let (status, headers, _) = exchange(
            &h.app,
            Method::GET,
            &format!("/api/oidc/authorize?{bad}"),
            None,
            None,
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!headers.contains_key(header::LOCATION));
    });
}

#[test]
fn required_upstream_cannot_be_bypassed_by_direct_password_login() {
    struct NoNetwork;
    impl secd_web::upstream::Transport for NoNetwork {
        fn get(&self, _: &str) -> anyhow::Result<(u16, Vec<u8>)> {
            anyhow::bail!("unexpected network")
        }
        fn post_form(&self, _: &str, _: &str, _: &str) -> anyhow::Result<(u16, Vec<u8>)> {
            anyhow::bail!("unexpected network")
        }
    }
    block_on(async {
        let h = fresh();
        login(&h).await;
        let state = AppState::open(&h.dir).unwrap();
        let upstream = secd_web::upstream::Upstream::with_transport(
            secd_web::upstream::UpstreamConfig {
                issuer: "https://identity.test".into(),
                client_id: "secd-test".into(),
                client_secret: zeroize::Zeroizing::new("fixture-only".into()),
                ca_pem: vec![],
                name: "Company".into(),
                required: true,
            },
            &state.origin,
            std::sync::Arc::new(NoNetwork),
        )
        .unwrap();
        let identity = upstream.remember("op@secd.test".into());
        let other = upstream.remember("other@secd.test".into());
        let app = secd_web::app(state.with_upstream(upstream));
        let payload = json!({"email":"op@secd.test","password":PW});
        let (status, _, _) =
            post_json(&app, "/api/auth/password/login", &payload, None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        for (cookie, expected) in [(other, StatusCode::FORBIDDEN), (identity, StatusCode::OK)] {
            let req = Request::builder()
                .method(Method::POST)
                .uri("/api/auth/password/login")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, format!("__Host-secd-idp={cookie}"))
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap();
            assert_eq!(app.clone().oneshot(req).await.unwrap().status(), expected);
        }
    });
}
