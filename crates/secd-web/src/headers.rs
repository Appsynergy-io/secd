use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::body::{to_bytes, Body};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;

pub const FAIL_SENTENCE: &str = "That email and credential do not match.";
pub const RATE_SENTENCE: &str = "Too many attempts. Wait a minute.";

const HSTS: &str = "max-age=63072000";
const CSP: &str = "default-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; object-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self'; connect-src 'self'; worker-src 'self'; upgrade-insecure-requests";
const PERMISSIONS: &str = "accelerometer=(), autoplay=(), camera=(), display-capture=(), encrypted-media=(), fullscreen=(), gamepad=(), geolocation=(), gyroscope=(), hid=(), identity-credentials-get=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-create=(self), publickey-credentials-get=(self), screen-wake-lock=(), serial=(), storage-access=(), usb=(), window-management=(), xr-spatial-tracking=()";

const AUTH_BODY: usize = 64 * 1024;
const VAULT_BODY: usize = 1024 * 1024;

pub fn fail_auth() -> Response {
    json_status(StatusCode::UNAUTHORIZED, FAIL_SENTENCE)
}

pub fn json_status(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({"error": error}))).into_response()
}

pub fn json_value(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}

pub async fn gate(State(state): State<AppState>, req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let ip = peer_ip(&req);

    if !matches!(
        method,
        Method::GET | Method::POST | Method::PUT | Method::DELETE
    ) {
        return finish(&path, json_status(StatusCode::METHOD_NOT_ALLOWED, "method"));
    }

    if rate_limited_path(&path) && !state.pending.allow_rate(ip) {
        return finish(
            &path,
            json_status(StatusCode::TOO_MANY_REQUESTS, RATE_SENTENCE),
        );
    }

    let req = if method == Method::POST || method == Method::PUT {
        if !is_json_content_type(req.headers()) {
            return finish(&path, json_status(StatusCode::BAD_REQUEST, "content-type"));
        }
        let limit = body_limit(&path);
        let (parts, body) = req.into_parts();
        let bytes = match to_bytes(body, limit).await {
            Ok(b) => b,
            Err(_) => {
                return finish(
                    &path,
                    json_status(StatusCode::PAYLOAD_TOO_LARGE, "too large"),
                );
            }
        };
        Request::from_parts(parts, Body::from(bytes))
    } else {
        req
    };

    let res = next.run(req).await;
    finish(&path, res)
}

fn finish(path: &str, mut res: Response) -> Response {
    apply_security_headers(path, &mut res);
    res
}

fn apply_security_headers(path: &str, res: &mut Response) {
    let headers = res.headers_mut();
    insert(headers, header::STRICT_TRANSPORT_SECURITY, HSTS);
    insert(headers, header::CONTENT_SECURITY_POLICY, CSP);
    insert(headers, header::X_FRAME_OPTIONS, "DENY");
    insert(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    insert_name(headers, "x-xss-protection", "0");
    insert(headers, header::REFERRER_POLICY, "no-referrer");
    insert_name(headers, "permissions-policy", PERMISSIONS);
    insert_name(headers, "cross-origin-opener-policy", "same-origin");
    insert_name(headers, "cross-origin-embedder-policy", "require-corp");
    insert_name(headers, "cross-origin-resource-policy", "same-origin");
    insert(headers, header::CACHE_CONTROL, "no-store, max-age=0");
    insert(headers, header::PRAGMA, "no-cache");
    insert(headers, header::EXPIRES, "0");
    insert_name(headers, "x-robots-tag", "noindex, nofollow");
    insert(headers, header::CONTENT_TYPE, content_type_for(path));
    headers.remove(header::SERVER);
    headers.remove("x-powered-by");
    headers.remove("x-aspnet-version");
    headers.remove("expect-ct");
    headers.remove("public-key-pins");
    headers.remove("public-key-pins-report-only");
    let cors: Vec<_> = headers
        .keys()
        .filter(|k| k.as_str().starts_with("access-control-"))
        .cloned()
        .collect();
    for k in cors {
        headers.remove(k);
    }
}

pub(crate) fn content_type_for(path: &str) -> &'static str {
    if path.starts_with("/api/") {
        return "application/json; charset=UTF-8";
    }
    match extension(path) {
        "js" => "text/javascript; charset=UTF-8",
        "css" => "text/css; charset=UTF-8",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=UTF-8",
        "json" => "application/json; charset=UTF-8",
        "html" => "text/html; charset=UTF-8",
        _ => "text/html; charset=UTF-8",
    }
}

fn extension(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() && !name.starts_with('.') => ext,
        _ => "",
    }
}

fn insert(headers: &mut HeaderMap, name: header::HeaderName, value: &'static str) {
    if let Ok(v) = HeaderValue::from_str(value) {
        headers.insert(name, v);
    }
}

fn insert_name(headers: &mut HeaderMap, name: &'static str, value: &'static str) {
    if let (Ok(n), Ok(v)) = (
        HeaderName::from_static_ok(name),
        HeaderValue::from_str(value),
    ) {
        headers.insert(n, v);
    }
}

trait FromStaticOk {
    fn from_static_ok(s: &'static str) -> Result<HeaderName, ()>;
}

impl FromStaticOk for HeaderName {
    fn from_static_ok(s: &'static str) -> Result<HeaderName, ()> {
        HeaderName::try_from(s).map_err(|_| ())
    }
}

fn is_json_content_type(headers: &HeaderMap) -> bool {
    let Some(v) = headers.get(header::CONTENT_TYPE) else {
        return false;
    };
    let Ok(s) = v.to_str() else {
        return false;
    };
    let s = s.trim();
    let (ty, rest) = match s.split_once(';') {
        Some((ty, rest)) => (ty, rest),
        None => (s, ""),
    };
    if !ty.trim().eq_ignore_ascii_case("application/json") {
        return false;
    }
    if rest.trim().is_empty() {
        return true;
    }
    for part in rest.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((k, v)) = part.split_once('=') else {
            return false;
        };
        if !k.trim().eq_ignore_ascii_case("charset") {
            return false;
        }
        let v = v.trim().trim_matches('"');
        if !v.eq_ignore_ascii_case("utf-8") {
            return false;
        }
    }
    true
}

fn body_limit(path: &str) -> usize {
    if path.starts_with("/api/v1/vault") || path.starts_with("/api/v1/providers") {
        VAULT_BODY
    } else {
        AUTH_BODY
    }
}

fn rate_limited_path(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/start"
            | "/api/auth/password/login"
            | "/api/auth/password/register"
            | "/api/auth/passkey/register/start"
            | "/api/auth/passkey/register/finish"
            | "/api/auth/passkey/login/start"
            | "/api/auth/passkey/login/finish"
    )
}

fn peer_ip(req: &Request<Body>) -> IpAddr {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}
