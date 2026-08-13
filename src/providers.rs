//! Built-in provider schemas. Env names only; no values.

use secd_core::providers;

pub fn run() -> anyhow::Result<()> {
    for p in providers() {
        println!("{}", p.name);
        for f in &p.fields {
            println!("  {} {}", f.key, f.env);
        }
    }
    Ok(())
}
