use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use serde::Serialize;

use crate::state::AppState;

#[derive(Clone, Debug, Serialize)]
pub struct AuditEvent {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Clone)]
pub struct AuditLog {
    path: PathBuf,
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl AuditLog {
    pub fn open(dir: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            path: dir.join("audit.jsonl"),
            events: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn record(&self, action: &str, session_id: Option<&str>) {
        let event = AuditEvent {
            action: action.to_string(),
            session_id: session_id.map(str::to_string),
        };
        if let Ok(line) = serde_json::to_string(&event) {
            if let Ok(mut f) = OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(&self.path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
        lock(&self.events).push(event);
    }

    pub fn events(&self) -> Vec<AuditEvent> {
        lock(&self.events).clone()
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
