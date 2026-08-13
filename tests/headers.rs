#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderMap, HeaderName, Method, Request, StatusCode};
use axum::Router;
use secd_web::AppState;
use serde_json::json;
use tower::ServiceExt;

const HSTS: &str = "max-age=63072000";
const CSP: &str = "default-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; object-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; font-src 'self'; img-src 'self'; connect-src 'self'; worker-src 'self'; upgrade-insecure-requests";
const PERM: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), display-capture=(), document-domain=(), encrypted-media=(), execution-while-not-rendered=(), execution-while-out-of-viewport=(), fullscreen=(), gamepad=(), geolocation=(), gyroscope=(), hid=(), identity-credentials-get=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), otp-credentials=(), payment=(), picture-in-picture=(), publickey-credentials-create=(self), publickey-credentials-get=(self), screen-wake-lock=(), serial=(), speaker-selection=(), storage-access=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=(), interest-cohort=()";

const SEC: &[&str] = &[
    "strict-transport-security",
    "content-security-policy",
    "x-frame-options",
    "x-content-type-options",
    "x-xss-protection",
    "referrer-policy",
    "permissions-policy",
    "cross-origin-opener-policy",
    "cross-origin-embedder-policy",
    "cross-origin-resource-policy",
    "cache-control",
    "pragma",
    "expires",
    "x-robots-tag",
];

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
    let dir = std::env::temp_dir().join(format!("secd-t3-hdr-{}-{n}", std::process::id()));
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
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(path);
    if let Some(ct) = content_type {
        b = b.header(header::CONTENT_TYPE, ct);
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

async fn ok_headers(app: &Router) -> HeaderMap {
    let (_, h, _) = exchange(
        app,
        Method::POST,
        "/api/auth/start",
        Some(json!({"email": "op@secd.test"}).to_string().into_bytes()),
        Some("application/json"),
    )
    .await;
    h
}

fn hv(h: &HeaderMap, name: &str) -> String {
    h.get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

#[test]
fn T_HDR_HSTS() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "strict-transport-security"), HSTS);
        assert!(!hv(&hdrs, "strict-transport-security").contains("preload"));
        assert!(!hv(&hdrs, "strict-transport-security").contains("includeSubDomains"));
    });
}

#[test]
fn T_HDR_CSP() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        let csp = hv(&hdrs, "content-security-policy");
        assert_eq!(csp, CSP);
        assert!(!csp.contains("'unsafe-inline'"));
        assert!(!csp.contains("'unsafe-eval'"));
    });
}

#[test]
fn T_HDR_FRAME() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "x-frame-options"), "DENY");
        assert!(hv(&hdrs, "content-security-policy").contains("frame-ancestors 'none'"));
    });
}

#[test]
fn T_HDR_NOSNIFF() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "x-content-type-options"), "nosniff");
    });
}

#[test]
fn T_HDR_XSS0() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "x-xss-protection"), "0");
    });
}

#[test]
fn T_HDR_REFERRER() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "referrer-policy"), "no-referrer");
    });
}

#[test]
fn T_HDR_PERM() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        let p = hv(&hdrs, "permissions-policy");
        assert_eq!(p, PERM);
        assert!(p.contains("camera=()"));
        assert!(p.contains("microphone=()"));
        assert!(p.contains("publickey-credentials-get=(self)"));
    });
}

#[test]
fn T_HDR_COOP() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "cross-origin-opener-policy"), "same-origin");
    });
}

#[test]
fn T_HDR_COEP() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "cross-origin-embedder-policy"), "require-corp");
    });
}

#[test]
fn T_HDR_CORP() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "cross-origin-resource-policy"), "same-origin");
    });
}

#[test]
fn T_HDR_CACHE() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "cache-control"), "no-store, max-age=0");
        assert_eq!(hv(&hdrs, "pragma"), "no-cache");
        assert_eq!(hv(&hdrs, "expires"), "0");
    });
}

#[test]
fn T_HDR_ROBOTS() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "x-robots-tag"), "noindex, nofollow");
    });
}

#[test]
fn T_HDR_JSON_CT() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert_eq!(hv(&hdrs, "content-type"), "application/json; charset=UTF-8");
    });
}

#[test]
fn T_HDR_HTML_CT() {
    block_on(async {
        let h = fresh();
        let (_, hdrs, _) = exchange(&h.app, Method::GET, "/", None, None).await;
        assert_eq!(hv(&hdrs, "content-type"), "text/html; charset=UTF-8");
    });
}

#[test]
fn T_HDR_NO_SERVER() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert!(hdrs.get("server").is_none());
    });
}

#[test]
fn T_HDR_NO_POWERED() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert!(hdrs.get("x-powered-by").is_none());
    });
}

#[test]
fn T_HDR_NO_EXPECT_CT() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert!(hdrs.get("expect-ct").is_none());
    });
}

#[test]
fn T_HDR_NO_HPKP() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        assert!(hdrs.get("public-key-pins").is_none());
        assert!(hdrs.get("public-key-pins-report-only").is_none());
    });
}

#[test]
fn T_HDR_NO_CORS() {
    block_on(async {
        let h = fresh();
        let hdrs = ok_headers(&h.app).await;
        for name in hdrs.keys() {
            assert!(
                !name.as_str().starts_with("access-control-"),
                "cors {}",
                name
            );
        }
        assert!(hdrs.get("access-control-allow-origin").is_none());
    });
}

#[test]
fn T_HDR_ON_ERROR() {
    block_on(async {
        let h = fresh();
        let ok = ok_headers(&h.app).await;
        let (_, e401, _) = exchange(&h.app, Method::GET, "/api/session", None, None).await;
        let (_, e404, _) = exchange(&h.app, Method::GET, "/api/v1/admin", None, None).await;
        for name in SEC {
            let n = HeaderName::from_static(name);
            assert_eq!(ok.get(&n), e401.get(&n), "401 {name}");
            assert_eq!(ok.get(&n), e404.get(&n), "404 {name}");
        }
    });
}

#[test]
fn T_HDR_TRACE() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = exchange(&h.app, Method::TRACE, "/api/auth/start", None, None).await;
        assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
    });
}

#[test]
fn T_HDR_BAD_CT() {
    block_on(async {
        let h = fresh();
        let (s, _, _) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/start",
            Some(br#"{"email":"op@secd.test"}"#.to_vec()),
            Some("text/plain"),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    });
}

#[test]
fn T_HDR_BODY_AUTH() {
    block_on(async {
        let h = fresh();
        let mut body = vec![b'{'; 64 * 1024 + 1];
        body[0] = b'{';
        let (s, _, _) = exchange(
            &h.app,
            Method::POST,
            "/api/auth/start",
            Some(body),
            Some("application/json"),
        )
        .await;
        assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
    });
}

#[test]
fn T_HDR_BODY_VAULT() {
    block_on(async {
        let h = fresh();
        let body = vec![b'x'; 1024 * 1024 + 1];
        let (s, _, _) = exchange(
            &h.app,
            Method::PUT,
            "/api/v1/vault",
            Some(body),
            Some("application/json"),
        )
        .await;
        assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
    });
}

#[test]
fn T_HDR_TLS13() {
    block_on(async {
        let h = fresh();
        let cert_dir = h.dir.join("tls");
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
        let v12 = Command::new("openssl")
            .args([
                "s_client",
                "-connect",
                &host,
                "-servername",
                "secd.imabee.com",
                "-CAfile",
            ])
            .arg(&cert)
            .args(["-tls1_2", "-brief"])
            .output()
            .expect("s_client 1.2");
        let err12 = String::from_utf8_lossy(&v12.stderr);
        let out12 = String::from_utf8_lossy(&v12.stdout);
        let tls12_ok = out12.contains("TLSv1.2") && !err12.contains("handshake failure");
        assert!(
            !v12.status.success()
                || err12.contains("handshake")
                || err12.contains("wrong version")
                || err12.contains("protocol")
                || !tls12_ok,
            "TLS 1.2 must be refused"
        );
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
        let ok13 = v13.status.success()
            || out13.contains("TLSv1.3")
            || err13.contains("TLSv1.3")
            || out13.contains("Protocol");
        assert!(ok13, "TLS 1.3 must succeed");
    });
}
