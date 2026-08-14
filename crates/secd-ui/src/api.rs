//! JSON API paths used by the console. Cookie credentials, no CORS.

use serde_json::Value;

pub fn start_url() -> &'static str {
    "/api/auth/start"
}

pub fn session_url() -> &'static str {
    "/api/session"
}

pub fn logout_url() -> &'static str {
    "/api/auth/logout"
}

pub fn password_login_url() -> &'static str {
    "/api/auth/password/login"
}

pub fn password_register_url() -> &'static str {
    "/api/auth/password/register"
}

pub fn passkey_register_start_url() -> &'static str {
    "/api/auth/passkey/register/start"
}

pub fn passkey_register_finish_url() -> &'static str {
    "/api/auth/passkey/register/finish"
}

pub fn passkey_login_start_url() -> &'static str {
    "/api/auth/passkey/login/start"
}

pub fn passkey_login_finish_url() -> &'static str {
    "/api/auth/passkey/login/finish"
}

pub fn passkeys_url() -> &'static str {
    "/api/auth/passkeys"
}

pub fn sessions_url() -> &'static str {
    "/api/v1/sessions"
}

pub fn vault_url() -> &'static str {
    "/api/v1/vault"
}

pub fn providers_url() -> &'static str {
    "/api/v1/providers"
}

pub fn audit_url() -> &'static str {
    "/api/v1/audit"
}

pub fn device_approve_url() -> &'static str {
    "/api/v1/device/approve"
}

pub fn session_revoke_path(id: &str) -> String {
    format!("/api/v1/sessions/{id}")
}

pub fn passkey_delete_path(id: &str) -> String {
    format!("/api/auth/passkeys/{id}")
}

/// Client issues DELETE `/api/v1/sessions/:id` (percent-encoded).
pub fn session_revoke_delete(id: &str) -> String {
    format!("DELETE /api/v1/sessions/{}", utf8_percent_encode(id))
}

fn utf8_percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn error_message(v: &Value) -> Option<String> {
    v.get("error").and_then(|e| e.as_str()).map(str::to_string)
}
