use axum::http::{header, HeaderValue};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;

const INDEX: &str = include_str!("../../secd-ui/index.html");
const JS: &str = include_str!("../../secd-ui/dist/secd-ui.js");
const WASM: &[u8] = include_bytes!("../../secd-ui/dist/secd-ui.wasm");

pub fn router() -> Router<AppState> {
    Router::new()
        .route_service("/", get(index))
        .route_service("/secd-ui.js", get(js))
        .route_service("/secd-ui.wasm", get(wasm))
}

async fn index() -> Response {
    bytes("text/html; charset=UTF-8", INDEX.as_bytes().to_vec())
}

async fn js() -> Response {
    bytes("text/javascript; charset=UTF-8", JS.as_bytes().to_vec())
}

async fn wasm() -> Response {
    bytes("application/wasm", WASM.to_vec())
}

fn bytes(content_type: &'static str, body: Vec<u8>) -> Response {
    let mut res = Response::new(axum::body::Body::from(body));
    if let Ok(v) = HeaderValue::from_str(content_type) {
        res.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    res
}
