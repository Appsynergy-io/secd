//! `secd gitea [--bundle N] -- CMD` and `--install-git`.

use std::process::Stdio;

use anyhow::Context;

use crate::policy::{self, GiteaPick};

pub fn run() -> anyhow::Result<()> {
    let matches = crate::cli().get_matches();
    let sub = matches
        .subcommand_matches("gitea")
        .expect("invariant: gitea subcommand");
    let bundle = sub.get_one::<String>("bundle").map(String::as_str);
    let install = sub.get_flag("install-git");
    let cmd: Vec<String> = sub
        .get_many::<String>("cmd")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();

    let unlocked = policy::require_unlocked()?;
    let entries = policy::load_entries(&unlocked.token, &unlocked.dek)?;
    let bundles = policy::discover_bundles(&entries);
    // Installing the helper is not a gitea question. git picks a helper by
    // host, so every forge bundle that names one gets wired up, or a GitHub
    // credential the helper can serve is one git never asks for.
    if install {
        return install_git(&bundles, bundle);
    }

    match policy::pick_gitea(&bundles, bundle) {
        GiteaPick::One(picked) => {
            let extra = policy::gitea_env(picked);
            let mut redact = policy::redact_values(&entries);
            redact.extend(policy::env_values(&extra));
            crate::run::exec(&cmd, extra, redact)
        }
        GiteaPick::Zero => {
            eprintln!("no gitea credential — add one in secd");
            drop(bundles);
            drop(entries);
            std::process::exit(2);
        }
        GiteaPick::Many(names) => {
            for n in &names {
                println!("{n}");
            }
            eprintln!("secd gitea --bundle <name> -- …");
            drop(bundles);
            drop(entries);
            std::process::exit(2);
        }
    }
}

fn install_git(bundles: &[policy::Bundle], want: Option<&str>) -> anyhow::Result<()> {
    let mut done: Vec<String> = Vec::new();
    for b in bundles {
        if want.is_some_and(|n| n != b.name) {
            continue;
        }
        if policy::forge_token(b).is_none() {
            continue;
        }
        let Some(origin) = policy::forge_origin(b) else {
            continue;
        };
        // One helper per origin. A second bundle for the same host would
        // silently replace the first, so it is named and skipped instead.
        if done.contains(&origin) {
            eprintln!(
                "{origin} already served by another bundle; skipped {}",
                b.name
            );
            continue;
        }
        write_helper(&origin, &b.name)?;
        println!("{origin} {}", b.name);
        done.push(origin);
    }
    if done.is_empty() {
        eprintln!("no forge credential — add one in secd");
        std::process::exit(2);
    }
    Ok(())
}

fn write_helper(origin: &str, bundle: &str) -> anyhow::Result<()> {
    let helper = format!("!secd git-credential --bundle {bundle}");
    let key = format!("credential.{origin}.helper");
    let git = "git";
    let status = std::process::Command::new(git)
        .args(["config", "--global", &key, &helper])
        .stdin(Stdio::null())
        .status()
        .context("git config")?;
    if !status.success() {
        anyhow::bail!("git config failed");
    }
    Ok(())
}
