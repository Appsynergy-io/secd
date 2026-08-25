pub mod aead;
pub mod group;
pub mod name;
pub mod parity;
pub mod provider;
pub mod redact;
pub mod wrap;

mod secret;

pub use aead::{open, seal};
pub use group::{group_names, NameGroup};
pub use name::{check_name, NameError};
pub use provider::{infer, providers, CustomProvider, Field, Provider};
pub use redact::redact;
pub use secret::Secret;
pub use wrap::{
    remove_wrap, unwrap_passkey, unwrap_password, wrap_passkey, wrap_password, Factor, Wrap,
};

/// Failure from AEAD, wrap, or a 32-byte key/PRF check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Aead,
    Truncated,
    Key,
    LastFactor,
    Prf,
    Hex,
    Rng,
    Factor,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Aead => "authentication failed",
            Self::Truncated => "blob shorter than 25 bytes",
            Self::Key => "key must be 32 bytes",
            Self::LastFactor => "cannot remove the last factor",
            Self::Prf => "PRF must be at least 32 bytes",
            Self::Hex => "invalid hex",
            Self::Rng => "rng failed",
            Self::Factor => "wrap factor mismatch",
        })
    }
}

impl std::error::Error for Error {}
