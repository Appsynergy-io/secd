use std::path::{Path, PathBuf};

use crate::audit::AuditLog;
use crate::auth::{Pending, UserStore};
use crate::device::DevicePending;
use crate::sessions::SessionStore;
use crate::vault::VaultStore;

pub const RP_ID: &str = "secd.imabee.com";
pub const ORIGIN: &str = "https://secd.imabee.com";

#[derive(Clone)]
pub struct AppState {
    pub db: PathBuf,
    pub pending: Pending,
    pub devices: DevicePending,
    pub users: UserStore,
    pub sessions: SessionStore,
    pub vault: VaultStore,
    pub audit: AuditLog,
    pub rp_id: &'static str,
    pub origin: &'static str,
}

impl AppState {
    pub fn open(data: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db = data.as_ref().to_path_buf();
        std::fs::create_dir_all(&db)?;
        Ok(Self {
            pending: Pending::new(),
            devices: DevicePending::new(),
            users: UserStore::open(&db)?,
            sessions: SessionStore::open(&db)?,
            vault: VaultStore::open(&db)?,
            audit: AuditLog::open(&db)?,
            db,
            rp_id: RP_ID,
            origin: ORIGIN,
        })
    }
}
