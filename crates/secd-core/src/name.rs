/// Why `check_name` rejected a secret name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameError {
    Empty,
    Length,
    Slash,
    DotDot,
    BadChar,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Empty => "empty name",
            Self::Length => "name must be 1..=256 bytes",
            Self::Slash => "name must not start or end with /",
            Self::DotDot => "name must not contain ..",
            Self::BadChar => "name has a character outside [A-Za-z0-9._@-/]",
        })
    }
}

impl std::error::Error for NameError {}

/// `[A-Za-z0-9._@-]+(?:/[A-Za-z0-9._@-]+)*`, no `..`, no leading/trailing `/`, 1..=256 bytes.
pub fn check_name(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.len() > 256 {
        return Err(NameError::Length);
    }
    if name.starts_with('/') || name.ends_with('/') {
        return Err(NameError::Slash);
    }
    if name.contains("..") {
        return Err(NameError::DotDot);
    }
    for seg in name.split('/') {
        if seg.is_empty() || !seg.bytes().all(is_name_byte) {
            return Err(NameError::BadChar);
        }
    }
    Ok(())
}

fn is_name_byte(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'@' | b'-'
    )
}
