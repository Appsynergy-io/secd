//! Replace whole secret values on stdin.

use std::io::{self, Read, Write};

pub fn run() -> anyhow::Result<()> {
    let unlocked = crate::policy::require_unlocked()?;
    let entries = crate::policy::load_entries(&unlocked.token, &unlocked.dek)?;
    let values = crate::policy::redact_values(&entries);
    let refs: Vec<&str> = values.iter().map(String::as_str).collect();
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let out = secd_core::redact(&input, &refs);
    let mut w = io::stdout().lock();
    w.write_all(out.as_bytes())?;
    w.flush()?;
    Ok(())
}
