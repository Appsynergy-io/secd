use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::Json;
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;
use webauthn_rs::prelude::DiscoverableKey;
use zeroize::Zeroize;

use crate::auth::{
    normalize_email, now_rfc3339, parse_login_cred, parse_prf, parse_register_cred, passkey_wrap,
    password_ok, password_wrap, webauthn, wrap_json_list, PendingEntry, PendingErr, PrfErr,
    StoredPasskey, User,
};
use crate::headers::{fail_auth, json_status, json_value};
use crate::sessions::{clear_cookie, with_cookie};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/start", post(start))
        .route("/api/auth/passkey/register/start", post(pk_reg_start))
        .route("/api/auth/passkey/register/finish", post(pk_reg_finish))
        .route("/api/auth/passkey/login/start", post(pk_login_start))
        .route("/api/auth/passkey/login/finish", post(pk_login_finish))
        .route("/api/auth/password/register", post(pw_register))
        .route("/api/auth/password/login", post(pw_login))
        .route("/api/auth/logout", post(logout))
        .route("/api/session", get(session))
        .route("/api/auth/passkeys", get(list_passkeys))
        .route(
            concat!("/api/auth/passkeys/", "{id}"),
            delete(delete_passkey),
        )
}

#[derive(Deserialize)]
struct EmailBody {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Deserialize)]
struct PasswordBody {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    password: String,
}

#[derive(Deserialize)]
struct FinishBody {
    handle: String,
    credential: Value,
    #[serde(default)]
    prf: Option<Value>,
    #[serde(default)]
    email: Option<String>,
}

async fn start(State(state): State<AppState>, Json(body): Json<EmailBody>) -> Response {
    let Some(raw) = body.email.as_deref() else {
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    let Some(email) = normalize_email(raw) else {
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    let method = if state.users.is_empty() {
        "register"
    } else if let Some(user) = state.users.get(&email) {
        user.method()
    } else {
        "passkey"
    };
    json_value(StatusCode::OK, json!({ "method": method }))
}

async fn pk_reg_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EmailBody>,
) -> Response {
    let Some(raw) = body.email.as_deref() else {
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    let Some(email) = normalize_email(raw) else {
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    let session = state.sessions.console_from_headers(&headers);
    let existing = state.users.get(&email);
    let add = match (&session, &existing, state.users.is_empty()) {
        (_, _, true) => false,
        (Some(s), Some(_), _) if s.email == email => true,
        (Some(s), None, _) if s.email == email => true,
        (Some(_), _, _) => return fail_auth(),
        (None, Some(_), _) => return fail_auth(),
        (None, None, false) => return fail_auth(),
    };
    if let Some(s) = &session {
        if s.email != email {
            return fail_auth();
        }
    }
    let user_id = existing.as_ref().map(|u| u.id).unwrap_or_else(Uuid::new_v4);
    let exclude = existing.as_ref().map(|u| {
        u.passkeys
            .iter()
            .map(|p| p.passkey.cred_id().clone())
            .collect()
    });
    let wa = webauthn();
    let (ccr, reg) = match wa.start_passkey_registration(user_id, &email, &email, exclude) {
        Ok(v) => v,
        Err(_) => return json_status(StatusCode::BAD_REQUEST, "webauthn"),
    };
    let handle = state.pending.insert(PendingEntry::Register {
        email,
        user_id,
        state: reg,
        add,
        created: tokio::time::Instant::now(),
    });
    let mut out = serde_json::to_value(&ccr).unwrap_or_else(|_| json!({}));
    if let Value::Object(ref mut m) = out {
        m.insert("handle".into(), json!(handle));
    }
    json_value(StatusCode::OK, out)
}

async fn pk_reg_finish(State(state): State<AppState>, Json(body): Json<FinishBody>) -> Response {
    let prf = match parse_prf(&body.prf) {
        Ok(p) => p,
        Err(PrfErr::Missing) | Err(PrfErr::Bad) => {
            let _ = state.pending.take(&body.handle);
            return json_status(StatusCode::BAD_REQUEST, "prf");
        }
    };
    let entry = match state.pending.take(&body.handle) {
        Ok(e) => e,
        Err(PendingErr::Missing) | Err(PendingErr::Expired) => return fail_auth(),
    };
    let PendingEntry::Register {
        email,
        user_id,
        state: reg_state,
        add,
        ..
    } = entry
    else {
        return fail_auth();
    };
    if let Some(raw) = body.email.as_deref() {
        match normalize_email(raw) {
            Some(e) if e == email => {}
            _ => return fail_auth(),
        }
    }
    let Some(cred) = parse_register_cred(&body.credential) else {
        return json_status(StatusCode::BAD_REQUEST, "credential");
    };
    let wa = webauthn();
    let passkey = match wa.finish_passkey_registration(&cred, &reg_state) {
        Ok(p) => p,
        Err(_) => return fail_auth(),
    };
    let cred_id = hex::encode(passkey.cred_id().as_slice());
    let wrap = passkey_wrap(&prf, &cred_id);
    let stored = StoredPasskey {
        id: cred_id,
        created: now_rfc3339(),
        passkey,
        wrap,
    };
    if add {
        let Some(mut user) = state.users.get(&email) else {
            return fail_auth();
        };
        user.passkeys.push(stored);
        if state.users.put(user).is_err() {
            return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
        }
        return json_value(StatusCode::OK, json!({ "ok": true }));
    }
    if !state.users.is_empty() {
        return fail_auth();
    }
    let user = User {
        id: user_id,
        email: email.clone(),
        password: None,
        passkeys: vec![stored],
    };
    if state.users.put(user).is_err() {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    }
    let (_id, token) = state.sessions.create_console(&email);
    with_cookie(json_value(StatusCode::OK, json!({ "ok": true })), &token)
}

async fn pk_login_start(State(state): State<AppState>, Json(body): Json<EmailBody>) -> Response {
    let wa = webauthn();
    let email = match body.email.as_deref() {
        None => None,
        Some("") => None,
        Some(s) => match normalize_email(s) {
            Some(e) => Some(e),
            None => return json_status(StatusCode::BAD_REQUEST, "email"),
        },
    };
    let (out, entry) = if let Some(email) = email {
        if let Some(user) = state.users.get(&email) {
            if user.has_passkey() {
                let creds: Vec<_> = user.passkeys.iter().map(|p| p.passkey.clone()).collect();
                match wa.start_passkey_authentication(&creds) {
                    Ok((rcr, ast)) => {
                        let handle_entry = PendingEntry::LoginSpecific {
                            email,
                            state: ast,
                            created: tokio::time::Instant::now(),
                        };
                        (
                            serde_json::to_value(&rcr).unwrap_or_else(|_| json!({})),
                            handle_entry,
                        )
                    }
                    Err(_) => return json_status(StatusCode::BAD_REQUEST, "webauthn"),
                }
            } else {
                match discoverable_start(&wa) {
                    Ok(v) => v,
                    Err(()) => return json_status(StatusCode::BAD_REQUEST, "webauthn"),
                }
            }
        } else {
            match discoverable_start(&wa) {
                Ok(v) => v,
                Err(()) => return json_status(StatusCode::BAD_REQUEST, "webauthn"),
            }
        }
    } else {
        match discoverable_start(&wa) {
            Ok(v) => v,
            Err(()) => return json_status(StatusCode::BAD_REQUEST, "webauthn"),
        }
    };
    let handle = state.pending.insert(entry);
    let mut out = out;
    if let Value::Object(ref mut m) = out {
        m.insert("handle".into(), json!(handle));
    }
    json_value(StatusCode::OK, out)
}

fn discoverable_start(wa: &webauthn_rs::prelude::Webauthn) -> Result<(Value, PendingEntry), ()> {
    let (rcr, ast) = wa.start_discoverable_authentication().map_err(|_| ())?;
    Ok((
        serde_json::to_value(&rcr).unwrap_or_else(|_| json!({})),
        PendingEntry::LoginDiscoverable {
            state: ast,
            created: tokio::time::Instant::now(),
        },
    ))
}

async fn pk_login_finish(State(state): State<AppState>, Json(body): Json<FinishBody>) -> Response {
    let prf = match parse_prf(&body.prf) {
        Ok(p) => p,
        Err(_) => {
            let _ = state.pending.take(&body.handle);
            return json_status(StatusCode::BAD_REQUEST, "prf");
        }
    };
    let _ = prf;
    let entry = match state.pending.take(&body.handle) {
        Ok(e) => e,
        Err(_) => return fail_auth(),
    };
    let Some(cred) = parse_login_cred(&body.credential) else {
        return fail_auth();
    };
    let wa = webauthn();
    let email = match entry {
        PendingEntry::LoginSpecific {
            email, state: ast, ..
        } => {
            if wa.finish_passkey_authentication(&cred, &ast).is_err() {
                return fail_auth();
            }
            email
        }
        PendingEntry::LoginDiscoverable { state: ast, .. } => {
            let Some((user, pk)) = state.users.by_cred_bytes(cred.get_credential_id()) else {
                return fail_auth();
            };
            let keys = [DiscoverableKey::from(&pk.passkey)];
            if wa
                .finish_discoverable_authentication(&cred, ast, &keys)
                .is_err()
            {
                return fail_auth();
            }
            user.email
        }
        PendingEntry::Register { .. } => return fail_auth(),
    };
    let Some(user) = state.users.get(&email) else {
        return fail_auth();
    };
    let wraps = wrap_json_list(&user.wraps());
    let (_id, token) = state.sessions.create_console(&email);
    with_cookie(
        json_value(StatusCode::OK, json!({ "wraps": wraps })),
        &token,
    )
}

async fn pw_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut body): Json<PasswordBody>,
) -> Response {
    let Some(raw) = body.email.as_deref() else {
        body.password.zeroize();
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    let Some(email) = normalize_email(raw) else {
        body.password.zeroize();
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    if !password_ok(&body.password) {
        body.password.zeroize();
        return json_status(StatusCode::BAD_REQUEST, "password");
    }
    let session = state.sessions.console_from_headers(&headers);
    if state.users.is_empty() {
        let (stored, wraps) = password_wrap(&body.password);
        body.password.zeroize();
        let user = User {
            id: Uuid::new_v4(),
            email: email.clone(),
            password: Some(stored),
            passkeys: vec![],
        };
        if state.users.put(user).is_err() {
            return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
        }
        let (_id, token) = state.sessions.create_console(&email);
        return with_cookie(
            json_value(StatusCode::OK, json!({ "wraps": wrap_json_list(&wraps) })),
            &token,
        );
    }
    if let Some(s) = session {
        if s.email != email {
            body.password.zeroize();
            return fail_auth();
        }
        let Some(mut user) = state.users.get(&email) else {
            body.password.zeroize();
            return fail_auth();
        };
        if user.has_password() {
            body.password.zeroize();
            return fail_auth();
        }
        let (stored, _wraps) = password_wrap(&body.password);
        body.password.zeroize();
        user.password = Some(stored);
        if state.users.put(user.clone()).is_err() {
            return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
        }
        return json_value(
            StatusCode::OK,
            json!({ "wraps": wrap_json_list(&user.wraps()) }),
        );
    }
    body.password.zeroize();
    fail_auth()
}

async fn pw_login(State(state): State<AppState>, Json(mut body): Json<PasswordBody>) -> Response {
    let Some(raw) = body.email.as_deref() else {
        body.password.zeroize();
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    let Some(email) = normalize_email(raw) else {
        body.password.zeroize();
        return json_status(StatusCode::BAD_REQUEST, "email");
    };
    if !password_ok(&body.password) {
        body.password.zeroize();
        return json_status(StatusCode::BAD_REQUEST, "password");
    }
    let user = state.users.get(&email);
    let ok = match &user {
        Some(u) => state.users.verify_password(u, body.password.as_bytes()),
        None => {
            state.users.dummy_argon2(body.password.as_bytes());
            false
        }
    };
    body.password.zeroize();
    if !ok {
        return fail_auth();
    }
    let user = user.expect("invariant: verified user exists");
    let wraps = wrap_json_list(&user.wraps());
    let (_id, token) = state.sessions.create_console(&email);
    with_cookie(
        json_value(StatusCode::OK, json!({ "wraps": wraps })),
        &token,
    )
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = crate::sessions::cookie_token(&headers) {
        state.sessions.revoke_token(&token);
    }
    let mut res = json_value(StatusCode::OK, json!({ "ok": true }));
    res.headers_mut()
        .insert(axum::http::header::SET_COOKIE, clear_cookie());
    res
}

async fn session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(s) = state.sessions.console_from_headers(&headers) else {
        return fail_auth();
    };
    let Some(user) = state.users.get(&s.email) else {
        return fail_auth();
    };
    json_value(
        StatusCode::OK,
        json!({
            "email": user.email,
            "has_passkey": user.has_passkey(),
            "has_password": user.has_password(),
            "session_id": s.id,
        }),
    )
}

async fn list_passkeys(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(s) = state.sessions.console_from_headers(&headers) else {
        return fail_auth();
    };
    let Some(user) = state.users.get(&s.email) else {
        return fail_auth();
    };
    json_value(StatusCode::OK, user.passkeys_json())
}

async fn delete_passkey(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(s) = state.sessions.console_from_headers(&headers) else {
        return fail_auth();
    };
    let Some((mut user, idx)) = state.users.by_passkey_id(&id) else {
        return json_status(StatusCode::NOT_FOUND, "not found");
    };
    if user.email != s.email {
        return json_status(StatusCode::NOT_FOUND, "not found");
    }
    let last_factor = user.factor_count() <= 1;
    let last_pk_no_pw = user.passkeys.len() == 1 && !user.has_password();
    if last_factor || last_pk_no_pw {
        return json_status(StatusCode::BAD_REQUEST, "last factor");
    }
    user.passkeys.remove(idx);
    if state.users.put(user).is_err() {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    }
    json_value(StatusCode::OK, json!({ "ok": true }))
}
