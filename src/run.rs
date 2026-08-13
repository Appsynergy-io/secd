//! `secd run --with P=B -- CMD`. Collision refuses the run. Child output is redacted.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::process::Stdio;

use anyhow::Context;
use zeroize::Zeroize;

pub fn run() -> anyhow::Result<()> {
    let matches = crate::cli().get_matches();
    let sub = matches
        .subcommand_matches("run")
        .expect("invariant: run subcommand");
    let specs: Vec<crate::policy::WithSpec> = match sub.get_many::<String>("with") {
        Some(vals) => vals
            .map(|s| crate::policy::parse_with(s))
            .collect::<anyhow::Result<Vec<_>>>()?,
        None => Vec::new(),
    };
    let cmd: Vec<String> = sub
        .get_many::<String>("cmd")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    crate::policy::check_with_collision(&specs)?;
    let unlocked = crate::policy::require_unlocked()?;
    let entries = crate::policy::load_entries(&unlocked.token, &unlocked.dek)?;
    let bundles = crate::policy::discover_bundles(&entries);
    let extra = crate::policy::apply_with(&specs, &bundles)?;
    let mut redact = crate::policy::redact_values(&entries);
    redact.extend(crate::policy::env_values(&extra));
    exec(&cmd, extra, redact)
}

pub fn exec(
    argv: &[String],
    mut extra: BTreeMap<String, String>,
    mut redact_vals: Vec<String>,
) -> anyhow::Result<()> {
    if argv.is_empty() {
        anyhow::bail!("missing command after --");
    }
    let lease = crate::policy::Lease::create();
    let mut child = std::process::Command::new(&argv[0]);
    child.args(&argv[1..]);
    for (k, v) in &extra {
        child.env(k, v);
    }
    let out = child
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("spawn {}", argv[0]))?;
    let refs: Vec<&str> = redact_vals.iter().map(String::as_str).collect();
    let stdout = secd_core::redact(&String::from_utf8_lossy(&out.stdout), &refs);
    let stderr = secd_core::redact(&String::from_utf8_lossy(&out.stderr), &refs);
    io::stdout().write_all(stdout.as_bytes())?;
    io::stdout().flush()?;
    io::stderr().write_all(stderr.as_bytes())?;
    io::stderr().flush()?;
    let code = out.status.code();
    drop(lease);
    for v in extra.values_mut() {
        v.zeroize();
    }
    extra.clear();
    for v in &mut redact_vals {
        v.zeroize();
    }
    match code {
        Some(0) => Ok(()),
        Some(c) => std::process::exit(c),
        None => anyhow::bail!("command terminated by signal"),
    }
}
