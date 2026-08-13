use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::Router;

use crate::headers::json_status;

pub mod audit;
pub mod auth;
pub mod auth_routes;
pub mod device;
pub mod headers;
pub mod providers_api;
pub mod sessions;
pub mod state;
pub mod static_ui;
pub mod tls;
pub mod vault;

pub use state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .merge(auth_routes::router())
        .merge(device::router())
        .merge(sessions::router())
        .merge(vault::router())
        .merge(providers_api::router())
        .merge(audit::router())
        .merge(static_ui::router())
        .fallback(fallback)
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), vault_auth))
        .layer(middleware::from_fn_with_state(state, headers::gate))
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
        return crate::headers::fail_auth();
    }
    next.run(req).await
}

async fn fallback(req: Request<Body>) -> Response {
    if req.uri().path().starts_with("/api/") {
        return json_status(StatusCode::NOT_FOUND, "not found");
    }
    if req.method() != axum::http::Method::GET {
        return json_status(StatusCode::METHOD_NOT_ALLOWED, "method");
    }
    Html("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>secd</title></head><body></body></html>").into_response()
}
