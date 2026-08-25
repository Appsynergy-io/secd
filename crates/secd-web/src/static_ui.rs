use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::headers::{content_type_for, json_status};
use crate::state::AppState;

include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

pub fn router() -> Router<AppState> {
    let mut r = Router::new()
        .route_service("/", get(index))
        .route_service("/device", get(index))
        .route_service("/register", get(index))
        .route_service("/activity", get(index))
        .route_service("/account", get(index));
    for path in ASSET_PATHS {
        r = r.route_service(path, get(asset));
    }
    r
}

fn shell() -> Response {
    bytes(
        content_type_for("/"),
        lookup("index.html").expect("invariant: ui/dist contains index.html"),
    )
}

async fn index() -> Response {
    shell()
}

async fn asset(req: Request<axum::body::Body>) -> Response {
    serve_path(req.uri().path())
}

pub fn serve_path(path: &str) -> Response {
    if path.starts_with("/api/") {
        return json_status(StatusCode::NOT_FOUND, "not found");
    }
    if let Some(body) = lookup(path) {
        return bytes(content_type_for(path), body);
    }
    shell()
}

fn lookup(path: &str) -> Option<&'static [u8]> {
    let key = asset_key(path)?;
    ASSETS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, b)| *b)
}

fn asset_key(path: &str) -> Option<&str> {
    let p = path.trim_start_matches('/');
    if p.is_empty() {
        return Some("index.html");
    }
    if p.as_bytes().contains(&0) {
        return None;
    }
    for part in p.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
    }
    Some(p)
}

fn bytes(content_type: &'static str, body: &'static [u8]) -> Response {
    let mut res = Response::new(axum::body::Body::from(body));
    if let Ok(v) = HeaderValue::from_str(content_type) {
        res.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    res
}
