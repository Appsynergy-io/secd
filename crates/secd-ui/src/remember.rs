//! Remembered identity is not a secret. `secd.last` = `{email, has_passkey, at}`.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::tokens::{LAST_KEY, REMEMBER_DAYS};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Remembered {
    pub email: String,
    pub has_passkey: bool,
    pub at: String,
}

pub fn last_key() -> &'static str {
    LAST_KEY
}

pub fn remember_is_fresh(at_iso: &str, now: OffsetDateTime) -> bool {
    let Ok(at) = OffsetDateTime::parse(at_iso, &Rfc3339) else {
        return false;
    };
    now - at <= Duration::days(REMEMBER_DAYS)
}

pub fn remember_is_fresh_unix(at_iso: &str, now_unix: i64) -> bool {
    let now = OffsetDateTime::from_unix_timestamp(now_unix).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    remember_is_fresh(at_iso, now)
}

pub fn parse_remembered(raw: &str) -> Option<Remembered> {
    serde_json::from_str(raw).ok()
}

pub fn encode_remembered(r: &Remembered) -> String {
    serde_json::to_string(r).unwrap_or_else(|_| "{}".into())
}

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}
