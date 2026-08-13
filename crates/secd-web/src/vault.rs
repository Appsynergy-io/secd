use axum::Router;

use crate::state::AppState;

#[derive(Clone)]
pub struct VaultStore;

impl VaultStore {
    pub fn open(_: &std::path::Path) -> anyhow::Result<Self> {
        Ok(Self)
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
}
