//! Generate a secret. Prints name and length only.

use std::fs::File;
use std::io::Read;

use secd_core::Secret;
use serde_json::json;

const GEN_BYTES: usize = 32;

pub fn run() -> anyhow::Result<()> {
    let name = crate::cli()
        .get_matches()
        .subcommand_matches("gen")
        .and_then(|m| m.get_one::<String>("name"))
        .cloned()
        .expect("invariant: clap requires NAME");
    secd_core::check_name(&name).map_err(|e| anyhow::anyhow!("{e}"))?;
    let unlocked = crate::policy::require_unlocked()?;
    let loaded = crate::policy::load_vault(&unlocked.token, &unlocked.dek)?;
    // The save below replaces the whole vault, so an entry that did not decode
    // is an entry it deletes.
    if let Some(refusal) = loaded.drop_refusal() {
        anyhow::bail!("{refusal}");
    }
    let crate::policy::VaultLoad {
        mut entries,
        before,
        ..
    } = loaded;
    let hex = random_hex(GEN_BYTES)?;
    let len = hex.len();
    let value = Secret::new(hex.into_bytes());
    if let Some(existing) = entries.iter_mut().find(|e| e.name == name) {
        existing.value = value;
    } else {
        entries.push(crate::policy::Entry {
            name: name.clone(),
            value,
            meta: json!({}),
        });
    }
    let rows: Vec<crate::policy::Row<'_>> = entries.iter().map(crate::policy::Entry::row).collect();
    crate::policy::save_entries_read_back(&unlocked.token, &unlocked.dek, &rows, &before)?;
    println!("{name} {len}");
    Ok(())
}

fn random_hex(n: usize) -> anyhow::Result<String> {
    let mut f = File::open("/dev/urandom").map_err(|e| anyhow::anyhow!("urandom: {e}"))?;
    let mut b = vec![0u8; n];
    f.read_exact(&mut b)
        .map_err(|e| anyhow::anyhow!("urandom read: {e}"))?;
    Ok(hex::encode(b))
}
