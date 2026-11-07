use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Redirect;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use rand::RngCore;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::Instant;

use crate::auth::decode_bytes;
use crate::headers::{fail_auth, json_status, json_value};
use crate::state::AppState;

const DEVICE_TTL: Duration = Duration::from_secs(10 * 60);
const INTERVAL: u64 = 5;
/// RFC 8628 §3.5: each `slow_down` adds five seconds to the interval.
const SLOW_DOWN_STEP: u64 = 5;
/// Network jitter can land two polls slightly closer than the client slept;
/// only a poll this much early is a client polling too fast.
const POLL_GRACE: Duration = Duration::from_secs(1);
/// RFC 8628 §3.4.
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

struct Device {
    /// sha256 of the private device code: the code itself is a bearer
    /// credential for the session it will release, so it is not kept.
    device_hash: [u8; 32],
    code_flow: Option<CodeFlow>,
    nonce: Option<String>,
    scope: String,
    /// Seconds the client must wait between polls; grows on `slow_down`.
    interval: u64,
    last_poll: Option<Instant>,
    hostname: String,
    eph_pub: String,
    created: Instant,
    created_at: SystemTime,
    approved: Option<Approved>,
    denied: bool,
}

struct CodeFlow {
    redirect_uri: String,
    state: String,
    challenge: String,
    code_hash: Option<[u8; 32]>,
}

struct Approved {
    token: String,
    sealed_dek: Value,
    /// Who approved, for the ID token: the account's stable id and email,
    /// the device session's id, and when the approval happened.
    sub: String,
    email: String,
    sid: String,
    auth_time: i64,
}

#[derive(Clone)]
pub struct DevicePending {
    inner: Arc<Mutex<HashMap<String, Device>>>,
}

impl Default for DevicePending {
    fn default() -> Self {
        Self::new()
    }
}

impl DevicePending {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn sweep(map: &mut HashMap<String, Device>) {
        let now = Instant::now();
        map.retain(|_, d| now.saturating_duration_since(d.created) <= DEVICE_TTL);
    }

    fn insert(&self, d: Device) -> Option<String> {
        let mut g = lock(&self.inner);
        Self::sweep(&mut g);
        if g.len() >= 256 {
            return None;
        }
        let mut code = user_code();
        while g.contains_key(&code) {
            code = user_code();
        }
        g.insert(code.clone(), d);
        Some(code)
    }
}

/// RFC 8628 §3.1 device authorization request, plus the CLI's ephemeral
/// X25519 key: the approving browser seals the vault key to it.
#[derive(Deserialize)]
struct StartBody {
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    eph_pub: Option<String>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
}

#[derive(Deserialize)]
struct CodeBody {
    user_code: String,
}

/// The RFC 8628 §3.4 device access token request, as JSON.
#[derive(Deserialize)]
struct PollBody {
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    grant_type: Option<String>,
    #[serde(default)]
    device_code: Option<String>,
    code: Option<String>,
    code_verifier: Option<String>,
    redirect_uri: Option<String>,
}

#[derive(Deserialize)]
struct ApproveBody {
    user_code: String,
    sealed_dek: Value,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/oidc/authorize", get(authorize))
        .route("/api/v1/device/start", post(start))
        .route("/api/v1/device/poll", post(poll))
        .route("/api/v1/device/approve", post(approve))
        .route("/api/v1/device/deny", post(deny))
        .route("/api/v1/device/revoke", post(revoke))
        .route("/api/v1/device/pending", get(pending))
}

/// Registered native client: RFC 8252 loopback redirect, with a variable
/// ephemeral port. No web origin, userinfo, fragment or wildcard path.
fn native_redirect(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|u| {
        u.scheme() == "http"
            && u.host_str() == Some("127.0.0.1")
            && u.port().is_some()
            && u.username().is_empty()
            && u.password().is_none()
            && u.path() == "/oidc/callback"
            && u.query().is_none()
            && u.fragment().is_none()
    })
}

async fn authorize(State(state): State<AppState>, uri: Uri) -> Response {
    let mut p = HashMap::new();
    for (k, v) in url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()) {
        if p.insert(k.into_owned(), v.into_owned()).is_some() {
            return grant_error("invalid_request");
        }
    }
    let field = |key: &str| p.get(key).map(String::as_str).unwrap_or("");
    // Invalid redirect URIs are never followed, including error responses.
    if field("client_id") != crate::oidc::CLI_CLIENT_ID || !native_redirect(field("redirect_uri")) {
        return grant_error("invalid_request");
    }
    if field("response_type") != "code" {
        return grant_error("unsupported_response_type");
    }
    let scope = field("scope");
    if !scope.split(' ').any(|s| s == "openid")
        || scope.split(' ').any(|s| !matches!(s, "openid" | "email"))
    {
        return grant_error("invalid_scope");
    }
    let challenge = field("code_challenge");
    if field("code_challenge_method") != "S256"
        || secd_core::jwt::b64url_decode(challenge).map_or(true, |v| v.len() != 32)
        || field("state").is_empty()
        || field("state").len() > 256
        || field("nonce").is_empty()
        || field("nonce").len() > 256
        || field("hostname").is_empty()
        || field("hostname").len() > 256
    {
        return grant_error("invalid_request");
    }
    let Some(eph) = decode_bytes(field("eph_pub")).filter(|v| v.len() == 32) else {
        return grant_error("invalid_request");
    };
    let eph_pub = hex::encode(eph);
    let Some(code) = state.devices.insert(Device {
        device_hash: [0; 32],
        code_flow: Some(CodeFlow {
            redirect_uri: field("redirect_uri").to_owned(),
            state: field("state").to_owned(),
            challenge: challenge.to_owned(),
            code_hash: None,
        }),
        nonce: Some(field("nonce").to_owned()),
        scope: scope.to_owned(),
        interval: INTERVAL,
        last_poll: None,
        hostname: field("hostname").to_owned(),
        eph_pub: eph_pub.clone(),
        created: Instant::now(),
        created_at: SystemTime::now(),
        approved: None,
        denied: false,
    }) else {
        return json_status(StatusCode::TOO_MANY_REQUESTS, "temporarily_unavailable");
    };
    use axum::response::IntoResponse;
    Redirect::to(&format!("/device?code={code}&eph={eph_pub}")).into_response()
}

fn exchange_code(state: &AppState, body: PollBody) -> Response {
    let (Some(code), Some(verifier), Some(redirect)) =
        (body.code, body.code_verifier, body.redirect_uri)
    else {
        return grant_error("invalid_request");
    };
    if !(43..=128).contains(&verifier.len())
        || !verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
    {
        return grant_error("invalid_grant");
    }
    let hash = crate::audit::sha256(code.as_bytes());
    let mut g = lock(&state.devices.inner);
    DevicePending::sweep(&mut g);
    let Some(key) = g.iter().find_map(|(k, d)| {
        d.code_flow
            .as_ref()
            .filter(|f| {
                f.code_hash == Some(hash)
                    && f.redirect_uri == redirect
                    && f.challenge == secd_core::jwt::pkce_challenge(&verifier)
            })
            .filter(|_| !d.denied && d.approved.is_some())
            .map(|_| k.clone())
    }) else {
        return grant_error("invalid_grant");
    };
    let d = g.remove(&key).expect("request held under lock");
    drop(g);
    token_response(state, d)
}

async fn start(State(state): State<AppState>, headers: HeaderMap, bytes: Bytes) -> Response {
    let body: StartBody = match oauth_body(&headers, &bytes) {
        Ok(body) => body,
        Err(code) => return grant_error(code),
    };

    if body.client_id.as_deref() != Some(crate::oidc::CLI_CLIENT_ID) {
        return grant_error("invalid_client");
    }
    let scope = body.scope.as_deref().unwrap_or("");
    if !scope.split(' ').any(|s| s == "openid")
        || scope.split(' ').any(|s| !matches!(s, "openid" | "email"))
    {
        return grant_error("invalid_scope");
    }
    if body
        .nonce
        .as_ref()
        .is_some_and(|n| n.is_empty() || n.len() > 256)
    {
        return grant_error("invalid_request");
    }
    let Some(raw) = body.eph_pub.as_deref() else {
        return json_status(StatusCode::BAD_REQUEST, "eph_pub");
    };
    let Some(eph) = decode_bytes(raw) else {
        return json_status(StatusCode::BAD_REQUEST, "eph_pub");
    };
    if eph.len() != 32 {
        return json_status(StatusCode::BAD_REQUEST, "eph_pub");
    }
    let device_id = body.device_id.unwrap_or_default();
    let hostname = body.hostname.unwrap_or_default();
    if device_id.is_empty() || device_id.len() > 256 || hostname.is_empty() || hostname.len() > 256
    {
        return json_status(StatusCode::BAD_REQUEST, "device");
    }
    let eph_pub = hex::encode(&eph[..32]);
    let verification_uri = format!("{}/device", state.origin);
    let device_code = crate::auth::random_token();
    let Some(code) = state.devices.insert(Device {
        device_hash: crate::audit::sha256(device_code.as_bytes()),
        code_flow: None,
        nonce: body.nonce,
        scope: scope.to_string(),
        interval: INTERVAL,
        last_poll: None,
        hostname,
        eph_pub,
        created: Instant::now(),
        created_at: SystemTime::now(),
        approved: None,
        denied: false,
    }) else {
        return json_status(StatusCode::TOO_MANY_REQUESTS, "temporarily_unavailable");
    };
    json_value(
        StatusCode::OK,
        json!({
            "user_code": code,
            "device_code": device_code,
            "expires_in": DEVICE_TTL.as_secs(),
            "interval": INTERVAL,
            "verification_uri": verification_uri,
        }),
    )
}

/// OAuth requests use form encoding. JSON is retained as a secd extension;
/// both representations reject duplicate parameters and malformed bodies.
fn oauth_body<T: serde::de::DeserializeOwned>(
    headers: &HeaderMap,
    bytes: &[u8],
) -> Result<T, &'static str> {
    let ty = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if ty.eq_ignore_ascii_case("application/json") {
        return serde_json::from_slice(bytes).map_err(|_| "invalid_request");
    }
    if !ty.eq_ignore_ascii_case("application/x-www-form-urlencoded") {
        return Err("invalid_request");
    }
    let mut params = serde_json::Map::new();
    for (key, value) in url::form_urlencoded::parse(bytes) {
        if params
            .insert(key.into_owned(), Value::String(value.into_owned()))
            .is_some()
        {
            return Err("invalid_request");
        }
    }
    serde_json::from_value(Value::Object(params)).map_err(|_| "invalid_request")
}

/// RFC 8628 §3.5 error response: 400 with the registered error code.
fn grant_error(code: &str) -> Response {
    json_value(StatusCode::BAD_REQUEST, json!({ "error": code }))
}

/// The device access token request (RFC 8628 §3.4/§3.5). The private device
/// code is the only key: the public user code never releases anything. A
/// granted token is released once and the request is gone; a lost response
/// means approving again, never a second copy of the session.
async fn poll(State(state): State<AppState>, headers: HeaderMap, bytes: Bytes) -> Response {
    let body: PollBody = match oauth_body(&headers, &bytes) {
        Ok(body) => body,
        Err(code) => return grant_error(code),
    };
    if body.client_id.as_deref() != Some(crate::oidc::CLI_CLIENT_ID) {
        return grant_error("invalid_client");
    }

    if body.grant_type.as_deref() == Some("authorization_code") {
        return exchange_code(&state, body);
    }
    if body.grant_type.as_deref() != Some(DEVICE_GRANT) {
        return grant_error("unsupported_grant_type");
    }
    let Some(device_code) = body.device_code.filter(|c| !c.is_empty()) else {
        return grant_error("invalid_request");
    };
    let hash = crate::audit::sha256(device_code.as_bytes());
    let now = Instant::now();
    let mut g = lock(&state.devices.inner);
    // Expired requests are swept, so an unknown code and an expired one answer
    // alike: neither tells a guesser which codes ever existed.
    DevicePending::sweep(&mut g);
    let Some(code) = g.iter().find_map(|(code, d)| {
        (d.code_flow.is_none() && d.device_hash == hash).then(|| code.clone())
    }) else {
        return grant_error("expired_token");
    };
    let Some(d) = g.get_mut(&code) else {
        return grant_error("expired_token");
    };
    if d.denied {
        g.remove(&code);
        return grant_error("access_denied");
    }
    if d.approved.is_none() {
        let early = d.last_poll.is_some_and(|last| {
            now.saturating_duration_since(last) + POLL_GRACE < Duration::from_secs(d.interval)
        });
        d.last_poll = Some(now);
        if early {
            d.interval += SLOW_DOWN_STEP;
            return json_value(
                StatusCode::BAD_REQUEST,
                json!({ "error": "slow_down", "interval": d.interval }),
            );
        }
        return grant_error("authorization_pending");
    }
    let Some(d) = g.remove(&code) else {
        return grant_error("expired_token");
    };
    drop(g);
    token_response(&state, d)
}

fn token_response(state: &AppState, d: Device) -> Response {
    let Some(a) = d.approved else {
        return grant_error("invalid_grant");
    };
    if state.sessions.by_token(&a.token).is_none() {
        return grant_error("access_denied");
    }
    let Ok(id_token) = state.oidc.id_token(
        crate::oidc::CLI_CLIENT_ID,
        crate::oidc::Identity {
            subject: &a.sub,
            email: &a.email,
            session_id: &a.sid,
            auth_time: a.auth_time,
        },
        d.nonce.as_deref(),
        &a.token,
        d.scope.split(' ').any(|s| s == "email"),
    ) else {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "sign");
    };
    json_value(
        StatusCode::OK,
        json!({
            "access_token": a.token,
            "token_type": "Bearer",
            "expires_in": crate::sessions::SessionKind::Device.ttl(),
            "scope": d.scope,
            "id_token": id_token,
            "sealed_dek": a.sealed_dek,
        }),
    )
}

async fn approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ApproveBody>,
) -> Response {
    let Some(session) = state.sessions.console_from_headers(&headers) else {
        return fail_auth();
    };
    let now = Instant::now();
    let mut g = lock(&state.devices.inner);
    DevicePending::sweep(&mut g);
    let Some(d) = g.get_mut(&body.user_code) else {
        return fail_auth();
    };
    if now.saturating_duration_since(d.created) > DEVICE_TTL {
        return fail_auth();
    }
    if d.denied {
        return json_status(StatusCode::BAD_REQUEST, "denied");
    }
    if d.approved.is_some() {
        return json_status(StatusCode::BAD_REQUEST, "already approved");
    }
    if !sealed_ok(&body.sealed_dek) {
        return json_status(StatusCode::BAD_REQUEST, "sealed_dek");
    }
    let Some(user) = state.users.get(&session.email) else {
        return fail_auth();
    };
    let hostname = d.hostname.clone();
    let Ok((sid, token)) = state.sessions.create_device(&session.email, &hostname) else {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    };
    d.approved = Some(Approved {
        token,
        sealed_dek: body.sealed_dek,
        sub: user.id.to_string(),
        email: user.email,
        sid,
        auth_time: session
            .created
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    });
    if let Some(flow) = &mut d.code_flow {
        let code = crate::auth::random_token();
        flow.code_hash = Some(crate::audit::sha256(code.as_bytes()));
        let mut target = url::Url::parse(&flow.redirect_uri).expect("validated redirect");
        target
            .query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("state", &flow.state)
            .append_pair("iss", &state.origin);
        return json_value(
            StatusCode::OK,
            json!({"ok": true, "redirect_uri": target.as_str()}),
        );
    }
    json_value(StatusCode::OK, json!({ "ok": true }))
}

async fn pending(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if state.sessions.console_from_headers(&headers).is_none() {
        return fail_auth();
    }
    let now = Instant::now();
    let mut g = lock(&state.devices.inner);
    DevicePending::sweep(&mut g);
    let mut rows: Vec<_> = g
        .iter()
        .filter(|(_, d)| d.approved.is_none() && !d.denied)
        .map(|(code, d)| {
            let expires_in = DEVICE_TTL
                .saturating_sub(now.saturating_duration_since(d.created))
                .as_secs();
            (
                d.created,
                json!({
                    "user_code": code,
                    "hostname": d.hostname,
                    "eph_pub": d.eph_pub,
                    "created": crate::sessions::rfc3339(d.created_at),
                    "expires_in": expires_in,
                }),
            )
        })
        .collect();
    rows.sort_by_key(|(created, _)| *created);
    let pending: Vec<Value> = rows.into_iter().map(|(_, row)| row).collect();
    json_value(StatusCode::OK, json!({ "pending": pending }))
}

async fn deny(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CodeBody>,
) -> Response {
    if state.sessions.console_from_headers(&headers).is_none() {
        return fail_auth();
    }
    let now = Instant::now();
    let mut g = lock(&state.devices.inner);
    DevicePending::sweep(&mut g);
    let Some(d) = g.get_mut(&body.user_code) else {
        return json_status(StatusCode::NOT_FOUND, "not found");
    };
    if now.saturating_duration_since(d.created) > DEVICE_TTL {
        return json_status(StatusCode::NOT_FOUND, "not found");
    }
    if d.approved.is_some() {
        return json_status(StatusCode::BAD_REQUEST, "already approved");
    }
    d.denied = true;
    if let Some(flow) = &d.code_flow {
        let mut target = url::Url::parse(&flow.redirect_uri).expect("validated redirect");
        target
            .query_pairs_mut()
            .append_pair("error", "access_denied")
            .append_pair("state", &flow.state)
            .append_pair("iss", &state.origin);
        return json_value(
            StatusCode::OK,
            json!({"ok": true, "redirect_uri": target.as_str()}),
        );
    }
    json_value(StatusCode::OK, json!({ "ok": true }))
}

/// The CLI unseals with eph_pub (32-byte x25519 pub) and blob
/// (24-byte nonce + ciphertext + 16-byte tag); anything else fails closed.
fn sealed_ok(v: &Value) -> bool {
    let Value::Object(m) = v else { return false };
    let eph = m.get("eph_pub").and_then(Value::as_str).unwrap_or("");
    let blob = m.get("blob").and_then(Value::as_str).unwrap_or("");
    let hex_ok = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    eph.len() == 64
        && hex_ok(eph)
        && blob.len() >= (24 + 16) * 2
        && blob.len().is_multiple_of(2)
        && hex_ok(blob)
}

async fn revoke(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(s) = state.sessions.device_from_headers(&headers) else {
        return fail_auth();
    };
    let Some(token) = crate::sessions::bearer_token(&headers) else {
        return fail_auth();
    };
    if state
        .sessions
        .revoke_token_with_audit(&token, &s.id, &state.audit)
        .is_err()
    {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    }
    json_value(StatusCode::OK, json!({ "ok": true }))
}

fn user_code() -> String {
    const ALPH: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut raw = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut raw);
    let mut out = String::with_capacity(9);
    for (i, b) in raw.iter().enumerate() {
        if i == 4 {
            out.push('-');
        }
        out.push(ALPH[(*b as usize) % ALPH.len()] as char);
    }
    out
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
