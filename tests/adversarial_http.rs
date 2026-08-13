#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use std::future::Future;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use secd_web::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

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
    let dir = std::env::temp_dir().join(format!("secd-t4-adv-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data dir");
    let state = AppState::open(&dir).expect("open");
    H {
        app: secd_web::app(state),
        dir,
    }
}

fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_multi_thread()
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
    extra: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(path);
    if let Some(ct) = content_type {
        b = b.header(header::CONTENT_TYPE, ct);
    }
    for (k, v) in extra {
        b = b.header(*k, *v);
    }
    let req = b
        .body(Body::from(body.unwrap_or_default()))
        .expect("request");
    let res = app.clone().oneshot(req).await.expect("oneshot");
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = to_bytes(res.into_body(), 16 * 1024 * 1024)
        .await
        .expect("body");
    (status, headers, bytes.to_vec())
}

async fn post_json(
    app: &Router,
    path: &str,
    body: &Value,
    extra: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let (s, h, b) = exchange(
        app,
        Method::POST,
        path,
        Some(body.to_string().into_bytes()),
        Some("application/json"),
        extra,
    )
    .await;
    (s, h, serde_json::from_slice(&b).unwrap_or(Value::Null))
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

fn assert_no_stack(raw: &[u8]) {
    let text = String::from_utf8_lossy(raw).to_ascii_lowercase();
    for needle in [
        "stack backtrace",
        "stacktrace",
        "panicked at",
        "unwrap(",
        "src/",
        "crates/",
        "thread '",
    ] {
        assert!(!text.contains(needle), "stack leak");
    }
}

fn write_tls_pair(dir: &std::path::Path) -> (PathBuf, PathBuf) {
    let cert_dir = dir.join("tls");
    std::fs::create_dir_all(&cert_dir).expect("tls dir");
    let cert = cert_dir.join("tls.crt");
    let key = cert_dir.join("tls.key");
    let out = Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-sha256", "-days", "1", "-nodes", "-keyout",
        ])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .args([
            "-subj",
            "/CN=secd.imabee.com",
            "-addext",
            "subjectAltName=DNS:secd.imabee.com",
        ])
        .output()
        .expect("openssl req");
    assert!(out.status.success(), "cert gen failed");
    (cert, key)
}

#[test]
fn T_ADV_NO_HTTP_CLEAR() {
    block_on(async {
        let h = fresh();
        let (cert, key) = write_tls_pair(&h.dir);
        let tls = secd_web::tls::rustls_config(&cert, &key).expect("tls");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let addr = listener.local_addr().expect("addr");
        let app = h.app.clone();
        tokio::spawn(async move {
            axum_server::from_tcp_rustls(listener, tls)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .expect("serve");
        });
        for _ in 0..50 {
            if tokio::net::TcpStream::connect(addr).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let host = format!("{}:{}", addr.ip(), addr.port());
        let mut stream = TcpStream::connect(addr).expect("plain tcp");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .expect("write timeout");
        let sent = stream
            .write_all(b"GET / HTTP/1.1\r\nHost: secd.imabee.com\r\nConnection: close\r\n\r\n");
        let mut buf = Vec::new();
        let _ = stream.read_to_end(&mut buf);
        let text = String::from_utf8_lossy(&buf);
        let http_ok = sent.is_ok()
            && (text.starts_with("HTTP/1.0")
                || text.starts_with("HTTP/1.1")
                || text.starts_with("HTTP/2"));
        assert!(!http_ok, "plain HTTP/1.1 GET must fail on the TLS port");
        let v13 = Command::new("openssl")
            .args([
                "s_client",
                "-connect",
                &host,
                "-servername",
                "secd.imabee.com",
                "-CAfile",
            ])
            .arg(&cert)
            .args(["-tls1_3", "-brief"])
            .output()
            .expect("s_client 1.3");
        let out13 = String::from_utf8_lossy(&v13.stdout);
        let err13 = String::from_utf8_lossy(&v13.stderr);
        let tls_up = v13.status.success()
            || out13.contains("TLSv1.3")
            || err13.contains("TLSv1.3")
            || out13.contains("Protocol");
        assert!(tls_up, "TLS listener must accept TLS 1.3");
    });
}

#[test]
fn T_ADV_UNKNOWN_ROUTE() {
    block_on(async {
        let h = fresh();
        let (s, _, raw) = exchange(&h.app, Method::GET, "/api/v1/admin", None, None, &[]).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_no_stack(&raw);
        let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
        assert_eq!(v, json!({"error": "not found"}));
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains("/api/v1/vault"));
        assert!(!text.contains("router"));
    });
}

#[test]
fn T_ADV_METHOD() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = exchange(&h.app, Method::GET, "/api/auth/start", None, None, &[]).await;
        assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
        let (s, _, _) =
            exchange(&h.app, Method::GET, "/api/v1/device/start", None, None, &[]).await;
        assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
    });
}

#[test]
fn T_ADV_JSON_BOMB() {
    block_on(async {
        let h = fresh();
        let depth = 10 * 1024 * 1024 / 6 + 8;
        let mut bomb = Vec::with_capacity(10 * 1024 * 1024 + 64);
        for _ in 0..depth {
            bomb.extend_from_slice(b"{\"a\":");
        }
        bomb.push(b'0');
        bomb.extend(std::iter::repeat_n(b'}', depth));
        assert!(bomb.len() >= 10 * 1024 * 1024);
        let (s, _, _) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/start",
            Some(bomb),
            Some("application/json"),
            &[],
        )
        .await;
        assert!(
            s == StatusCode::BAD_REQUEST || s == StatusCode::PAYLOAD_TOO_LARGE,
            "json bomb status"
        );
        let (s2, _, v) = post_json(
            &h.app,
            "/api/auth/start",
            &json!({"email": "op@secd.test"}),
            &[],
        )
        .await;
        assert_eq!(s2, StatusCode::OK, "process must stay up");
        assert_eq!(v["method"], "register");
    });
}

#[test]
fn T_ADV_HOST_HEADER() {
    block_on(async {
        let h = fresh();
        let extra = [("host", "evil.test")];
        let (s, hdrs, v) = post_json(
            &h.app,
            "/api/auth/passkey/register/start",
            &json!({"email": "op@secd.test"}),
            &extra,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let raw = v.to_string();
        assert!(raw.contains("secd.imabee.com"));
        assert!(!raw.contains("evil.test"));
        let rp = v
            .pointer("/publicKey/rp/id")
            .or_else(|| v.pointer("/rp/id"))
            .and_then(Value::as_str);
        if let Some(id) = rp {
            assert_eq!(id, "secd.imabee.com");
        }
        if let Some(loc) = hdrs.get(header::LOCATION).and_then(|x| x.to_str().ok()) {
            assert!(loc.contains("secd.imabee.com"));
            assert!(!loc.contains("evil.test"));
        }
        let (s, _, start) = post_json(
            &h.app,
            "/api/v1/device/start",
            &json!({"eph_pub": EPH, "device_id": "dev-1", "hostname": "testhost"}),
            &extra,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let uri = start["verification_uri"].as_str().unwrap_or("");
        assert!(uri.contains("secd.imabee.com"));
        assert!(!uri.contains("evil.test"));
    });
}

#[test]
fn T_ADV_PATH_DOTDOT() {
    block_on(async {
        let h = fresh();
        let (s, _, raw) = exchange(
            &h.app,
            Method::GET,
            "/api/v1/../auth/start",
            None,
            None,
            &[],
        )
        .await;
        assert_no_stack(&raw);
        if s == StatusCode::OK {
            let v: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
            panic!("GET dotted path bypassed: {s} {v}");
        }
        assert!(
            s == StatusCode::NOT_FOUND || s == StatusCode::METHOD_NOT_ALLOWED,
            "dotted GET must not bypass"
        );
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/../auth/start",
            &json!({"email": "op@secd.test"}),
            &[],
        )
        .await;
        if s == StatusCode::OK {
            assert_eq!(v, json!({"method": "register"}));
        } else {
            assert!(
                s == StatusCode::NOT_FOUND || s == StatusCode::BAD_REQUEST,
                "dotted POST must not invent a route"
            );
        }
        let (s, _, raw) = exchange(
            &h.app,
            Method::GET,
            "/api/v1/vault/../../etc/passwd",
            None,
            None,
            &[],
        )
        .await;
        assert_no_stack(&raw);
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains("root:"));
        assert!(s == StatusCode::NOT_FOUND || s == StatusCode::UNAUTHORIZED);
        let (s, _, _) = exchange(&h.app, Method::GET, "/api/v1/../v1/vault", None, None, &[]).await;
        assert!(s == StatusCode::NOT_FOUND || s == StatusCode::UNAUTHORIZED);
    });
}

#[test]
fn T_ADV_SSRF_URI() {
    block_on(async {
        let h = fresh();
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/start",
            &json!({
                "eph_pub": EPH,
                "device_id": "dev-1",
                "hostname": "testhost",
                "verification_uri": "https://evil.test/steal"
            }),
            &[],
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let uri = v["verification_uri"].as_str().expect("uri");
        assert!(uri.contains("secd.imabee.com"), "uri host locked");
        assert!(!uri.contains("evil.test"));
        assert!(!uri.contains("169.254.169.254"));
        let (s, _, v) = post_json(
            &h.app,
            "/api/v1/device/start",
            &json!({
                "eph_pub": EPH,
                "device_id": "dev-2",
                "hostname": "https://evil.test/",
                "verification_uri": "http://169.254.169.254/latest/meta-data/"
            }),
            &[],
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let uri = v["verification_uri"].as_str().expect("uri");
        assert!(uri.contains("secd.imabee.com"));
        assert!(!uri.contains("evil.test"));
        assert!(!uri.contains("169.254.169.254"));
    });
}

#[test]
fn T_ADV_COOKIE_PREFIX() {
    block_on(async {
        let h = fresh();
        let (s, hdrs, _) = post_json(
            &h.app,
            "/api/auth/password/register",
            &json!({"email": "op@secd.test", "password": PW}),
            &[],
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let token = cookie_token(&hdrs).expect("cookie");
        let secd = format!("secd={token}");
        let (s, _, _) = exchange(
            &h.app,
            Method::GET,
            "/api/session",
            None,
            None,
            &[("cookie", secd.as_str())],
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, _, _) = exchange(
            &h.app,
            Method::GET,
            "/api/v1/vault",
            None,
            None,
            &[("cookie", secd.as_str())],
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let host = format!("__Host-secd={token}");
        let (s, _, _) = exchange(
            &h.app,
            Method::GET,
            "/api/session",
            None,
            None,
            &[("cookie", host.as_str())],
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    });
}
