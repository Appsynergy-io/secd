use std::path::{Path, PathBuf};

use webauthn_rs::prelude::Webauthn;

use crate::audit::AuditLog;
use crate::auth::{self, Pending, UserStore};
use crate::db::Db;
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
    pub webauthn: Webauthn,
    pub origin: String,
}

impl AppState {
    /// Opens with the compiled-in RP ID and origin defaults.
    pub fn open(data: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::open_with_hostname(data, None)
    }

    /// Opens with RP ID and origin derived from `hostname` (the `--hostname`
    /// flag), falling back to the `RP_ID`/`ORIGIN` defaults when `None`.
    /// Passkeys are bound to the RP ID, so this must match the host the
    /// browser actually connects to. The hostname is validated here, once,
    /// at startup: a bad value fails this call instead of panicking later
    /// on the first WebAuthn request.
    pub fn open_with_hostname(
        data: impl AsRef<Path>,
        hostname: Option<&str>,
    ) -> anyhow::Result<Self> {
        let db = data.as_ref().to_path_buf();
        std::fs::create_dir_all(&db)?;
        let handle = Db::open(&db)?;
        let (rp_id, origin) = match hostname {
            Some(h) => (h.to_string(), format!("https://{h}")),
            None => (RP_ID.to_string(), ORIGIN.to_string()),
        };
        let webauthn = auth::webauthn(&rp_id, &origin)?;
        Ok(Self {
            pending: Pending::new(),
            devices: DevicePending::new(),
            users: UserStore::open(&db)?,
            sessions: SessionStore::from_db(handle.clone()),
            vault: VaultStore::from_db(handle.clone(), &db),
            audit: AuditLog::from_db(handle, &db)?,
            db,
            webauthn,
            origin,
        })
    }
}
