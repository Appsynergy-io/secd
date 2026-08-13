use axum::http::{header, HeaderValue};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;

const INDEX: &str = include_str!("../../secd-ui/index.html");
const CSS: &str = include_str!("../../secd-ui/src/style.css");
const JS: &str = include_str!("../../secd-ui/src/app.js");
const BOOT: &str = r#"<div class="app" data-page="boot"></div>"#;

pub fn router() -> Router<AppState> {
    Router::new()
        .route_service("/", get(index))
        .route_service("/app.css", get(css))
        .route_service("/app.js", get(js))
}

async fn index() -> Response {
    bytes(
        "text/html; charset=UTF-8",
        INDEX.replace("__LEPTOS__", BOOT).into_bytes(),
    )
}

async fn css() -> Response {
    bytes("text/css; charset=UTF-8", CSS.as_bytes().to_vec())
}

async fn js() -> Response {
    bytes("text/javascript; charset=UTF-8", JS.as_bytes().to_vec())
}

fn bytes(content_type: &'static str, body: Vec<u8>) -> Response {
    let mut res = Response::new(axum::body::Body::from(body));
    if let Ok(v) = HeaderValue::from_str(content_type) {
        res.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    res
}
