//! Metadata for one name. No values.

use secd_core::infer;

pub fn run() -> anyhow::Result<()> {
    let name = crate::cli()
        .get_matches()
        .subcommand_matches("info")
        .and_then(|m| m.get_one::<String>("name"))
        .cloned()
        .expect("invariant: clap requires NAME");
    secd_core::check_name(&name).map_err(|e| anyhow::anyhow!("{e}"))?;
    let unlocked = crate::policy::require_unlocked()?;
    let entries = crate::policy::load_entries(&unlocked.token, &unlocked.dek)?;
    let Some(entry) = entries.iter().find(|e| e.name == name) else {
        anyhow::bail!("not found");
    };
    println!("{name}");
    println!("bytes {}", entry.value.len());
    let provider = entry.meta.get("provider").and_then(|v| v.as_str());
    if let Some(p) = provider {
        println!("provider {p}");
    }
    if let Some(fields) = entry.meta.get("fields").and_then(|v| v.as_array()) {
        let keys: Vec<&str> = fields.iter().filter_map(|v| v.as_str()).collect();
        if !keys.is_empty() {
            println!("fields {}", keys.join(" "));
        }
    } else if let Ok(s) = std::str::from_utf8(entry.value.as_bytes()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            if let Some(obj) = v.as_object() {
                let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
                if provider.is_none() {
                    if let Some(p) = infer(&keys) {
                        println!("provider {p}");
                    }
                }
                if !keys.is_empty() {
                    println!("fields {}", keys.join(" "));
                }
            }
        }
    }
    Ok(())
}
