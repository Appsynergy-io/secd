//! List secret names. No values.

pub fn run() -> anyhow::Result<()> {
    let unlocked = crate::policy::require_unlocked()?;
    let mut entries = crate::policy::load_entries(&unlocked.token, &unlocked.dek)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    for e in &entries {
        println!("{}", e.name);
    }
    Ok(())
}
