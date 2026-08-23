use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get};
use axum::Json;
use axum::Router;
use secd_core::{check_name, providers, CustomProvider, Field};
use serde_json::{json, Value};

use crate::headers::{fail_auth, json_status, json_value};
use crate::state::AppState;
use crate::vault::{fields_json, has_forbidden};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/providers", get(get_providers).put(put_provider))
        .route(
            concat!("/api/v1/providers/", "{name}"),
            delete(delete_provider),
        )
}

async fn get_providers(State(state): State<AppState>) -> Response {
    let mut out = Vec::new();
    for p in providers() {
        out.push(json!({
            "name": p.name,
            "title": p.title,
            "builtin": true,
            "fields": fields_json(&p.fields),
        }));
    }
    match state.vault.list_custom_providers() {
        Ok(custom) => {
            for p in custom {
                out.push(json!({
                    "name": p.name,
                    "title": p.title,
                    "builtin": false,
                    "fields": fields_json(&p.fields),
                }));
            }
        }
        Err(_) => return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store"),
    }
    json_value(StatusCode::OK, json!({ "providers": out }))
}

async fn put_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if state.sessions.console_from_headers(&headers).is_none() {
        return fail_auth();
    }
    let provider = match parse_custom(&body) {
        Ok(p) => p,
        Err(e) => return json_status(StatusCode::BAD_REQUEST, e),
    };
    if is_builtin(&provider.name) {
        return json_status(StatusCode::BAD_REQUEST, "builtin");
    }
    if state
        .vault
        .put_custom_provider(&provider, &state.audit)
        .is_err()
    {
        return json_status(StatusCode::INTERNAL_SERVER_ERROR, "store");
    }
    json_value(StatusCode::OK, json!({ "ok": true }))
}

async fn delete_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    if state.sessions.console_from_headers(&headers).is_none() {
        return fail_auth();
    }
    if is_builtin(&name) {
        return json_status(StatusCode::BAD_REQUEST, "builtin");
    }
    match state.vault.delete_custom_provider(&name, &state.audit) {
        Ok(true) => json_value(StatusCode::OK, json!({ "ok": true })),
        Ok(false) => json_status(StatusCode::NOT_FOUND, "not found"),
        Err(_) => json_status(StatusCode::INTERNAL_SERVER_ERROR, "store"),
    }
}

fn is_builtin(name: &str) -> bool {
    providers().iter().any(|p| p.name == name)
}

fn parse_custom(body: &Value) -> Result<CustomProvider, &'static str> {
    if has_forbidden(body) {
        return Err("plaintext");
    }
    let obj = body.as_object().ok_or("schema")?;
    for k in obj.keys() {
        if !matches!(k.as_str(), "name" | "title" | "fields") {
            return Err("schema");
        }
    }
    let name = obj.get("name").and_then(Value::as_str).ok_or("name")?;
    if !provider_name_ok(name) {
        return Err("name");
    }
    let title = obj.get("title").and_then(Value::as_str).ok_or("title")?;
    if title.is_empty() || title.len() > 256 {
        return Err("title");
    }
    let fields_v = obj
        .get("fields")
        .and_then(Value::as_array)
        .ok_or("fields")?;
    let mut fields = Vec::with_capacity(fields_v.len());
    for f in fields_v {
        fields.push(parse_field(f)?);
    }
    Ok(CustomProvider {
        name: name.to_string(),
        title: title.to_string(),
        fields,
    })
}

fn parse_field(v: &Value) -> Result<Field, &'static str> {
    if has_forbidden(v) {
        return Err("plaintext");
    }
    let obj = v.as_object().ok_or("fields")?;
    let key = obj.get("key").and_then(Value::as_str).ok_or("key")?;
    let env = obj.get("env").and_then(Value::as_str).ok_or("env")?;
    if key.is_empty() || env.is_empty() || key.len() > 256 || env.len() > 256 {
        return Err("fields");
    }
    Ok(Field {
        key: key.to_string(),
        secret: obj.get("secret").and_then(Value::as_bool).unwrap_or(false),
        optional: obj
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        env: env.to_string(),
    })
}

fn provider_name_ok(name: &str) -> bool {
    check_name(name).is_ok() && !name.contains('/')
}
