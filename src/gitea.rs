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
    match policy::pick_gitea(&bundles, bundle) {
        GiteaPick::One(picked) => {
            if install {
                return install_git(picked);
            }
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

fn install_git(bundle: &policy::Bundle) -> anyhow::Result<()> {
    let url =
        policy::gitea_url(bundle).ok_or_else(|| anyhow::anyhow!("gitea bundle has no url"))?;
    let origin = policy::origin_url(url);
    let helper = format!("!secd git-credential --bundle {}", bundle.name);
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
