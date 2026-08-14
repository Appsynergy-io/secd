use axum::http::{header, HeaderValue};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;

const INDEX: &str = "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>secd</title></head><body></body></html>";

pub fn router() -> Router<AppState> {
    Router::new().route_service("/", get(index))
}

async fn index() -> Response {
    bytes("text/html; charset=UTF-8", INDEX.as_bytes().to_vec())
}

fn bytes(content_type: &'static str, body: Vec<u8>) -> Response {
    let mut res = Response::new(axum::body::Body::from(body));
    if let Ok(v) = HeaderValue::from_str(content_type) {
        res.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    res
}
