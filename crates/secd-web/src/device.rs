use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::post;
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
const VERIFICATION_URI: &str = "https://secd.imabee.com/device";

struct Device {
    hostname: String,
    created: Instant,
    approved: Option<Approved>,
}

struct Approved {
    token: String,
    sealed_dek: Value,
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

    fn insert(&self, d: Device) -> String {
        let mut g = lock(&self.inner);
        Self::sweep(&mut g);
        let mut code = user_code();
        while g.contains_key(&code) {
            code = user_code();
        }
        g.insert(code.clone(), d);
        code
    }
}

#[derive(Deserialize)]
struct StartBody {
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

#[derive(Deserialize)]
struct ApproveBody {
    user_code: String,
    sealed_dek: Value,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/device/start", post(start))
        .route("/api/v1/device/poll", post(poll))
        .route("/api/v1/device/approve", post(approve))
        .route("/api/v1/device/revoke", post(revoke))
}

async fn start(State(state): State<AppState>, Json(body): Json<StartBody>) -> Response {
    let Some(raw) = body.eph_pub.as_deref() else {
        return json_status(StatusCode::BAD_REQUEST, "eph_pub");
    };
    let Some(eph) = decode_bytes(raw) else {
        return json_status(StatusCode::BAD_REQUEST, "eph_pub");
    };
    if eph.len() < 32 {
        return json_status(StatusCode::BAD_REQUEST, "eph_pub");
    }
    let device_id = body.device_id.unwrap_or_default();
    let hostname = body.hostname.unwrap_or_default();
    if device_id.is_empty() || hostname.is_empty() {
        return json_status(StatusCode::BAD_REQUEST, "device");
    }
    let _ = (eph, device_id);
    let code = state.devices.insert(Device {
        hostname,
        created: Instant::now(),
        approved: None,
    });
    json_value(
        StatusCode::OK,
        json!({
            "user_code": code,
            "interval": INTERVAL,
            "verification_uri": VERIFICATION_URI,
        }),
    )
}

async fn poll(State(state): State<AppState>, Json(body): Json<CodeBody>) -> Response {
    let now = Instant::now();
    let mut g = lock(&state.devices.inner);
    DevicePending::sweep(&mut g);
    let Some(d) = g.get(&body.user_code) else {
        return json_status(StatusCode::NOT_FOUND, "not found");
    };
    if now.saturating_duration_since(d.created) > DEVICE_TTL {
        g.remove(&body.user_code);
        return json_value(StatusCode::OK, json!({ "status": "expired" }));
    }
    match &d.approved {
        None => json_value(StatusCode::OK, json!({ "status": "pending" })),
        Some(a) => json_value(
            StatusCode::OK,
            json!({
                "status": "ok",
                "token": a.token,
                "sealed_dek": a.sealed_dek,
            }),
        ),
    }
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
    if d.approved.is_some() {
        return json_status(StatusCode::BAD_REQUEST, "already approved");
    }
    if !sealed_ok(&body.sealed_dek) {
        return json_status(StatusCode::BAD_REQUEST, "sealed_dek");
    }
    let hostname = d.hostname.clone();
    let (_id, token) = state.sessions.create_device(&session.email, &hostname);
    d.approved = Some(Approved {
        token,
        sealed_dek: body.sealed_dek,
    });
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
    if let Some(token) = crate::sessions::bearer_token(&headers) {
        state.sessions.revoke_token(&token);
    }
    state.audit.record("session.revoke", Some(&s.id));
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
