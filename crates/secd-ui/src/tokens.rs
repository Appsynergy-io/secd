pub const BREAKPOINT_PX: u32 = 900;
pub const REMEMBER_DAYS: i64 = 30;
pub const LAST_KEY: &str = "secd.last";
pub const FAIL_SENTENCE: &str = "That email and credential do not match.";
pub const RATE_SENTENCE: &str = "Too many attempts. Wait a minute.";
pub const EMAIL_AUTOCOMPLETE: &str = "username webauthn";
pub const PRF_SALT: [u8; 32] = *b"secd-prf-kek-v1\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
