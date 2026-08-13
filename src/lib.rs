use clap::{Arg, ArgAction, Command};

pub mod doctor;
pub mod gen;
pub mod git_credential;
pub mod gitea;
pub mod info;
pub mod keyring;
pub mod login;
pub mod logout;
pub mod ls;
pub mod policy;
pub mod providers;
pub mod redact;
pub mod run;
pub mod skills_install;
pub mod tui;
pub mod update;

pub use secd_core::Secret;

pub fn cli() -> Command {
    Command::new("secd")
        .bin_name("secd")
        .version(env!("CARGO_PKG_VERSION"))
        .about("LAN secrets store")
        .disable_help_subcommand(true)
        .subcommand(Command::new("logout").about("Drop DEK and HTTP session"))
        .subcommand(
            Command::new("gitea")
                .about("Run a command with the gitea bundle")
                .arg(
                    Arg::new("bundle")
                        .long("bundle")
                        .value_name("N")
                        .num_args(1),
                )
                .arg(
                    Arg::new("install-git")
                        .long("install-git")
                        .action(ArgAction::SetTrue)
                        .help("Install git credential helper for the gitea host"),
                )
                .arg(
                    Arg::new("cmd")
                        .value_name("CMD")
                        .num_args(0..)
                        .trailing_var_arg(true)
                        .allow_hyphen_values(true),
                ),
        )
        .subcommand(Command::new("git-credential").about("git credential helper"))
        .subcommand(
            Command::new("run")
                .about("Run a command with provider env")
                .arg(
                    Arg::new("with")
                        .long("with")
                        .value_name("P=B")
                        .action(ArgAction::Append),
                )
                .arg(
                    Arg::new("cmd")
                        .value_name("CMD")
                        .num_args(0..)
                        .trailing_var_arg(true)
                        .allow_hyphen_values(true),
                ),
        )
        .subcommand(Command::new("ls").about("List secret names"))
        .subcommand(
            Command::new("info")
                .about("Show metadata for a name")
                .arg(Arg::new("name").value_name("NAME").required(true)),
        )
        .subcommand(Command::new("providers").about("List providers"))
        .subcommand(Command::new("redact").about("Redact secret values on stdin"))
        .subcommand(
            Command::new("gen")
                .about("Generate a secret")
                .arg(Arg::new("name").value_name("NAME").required(true)),
        )
        .subcommand(Command::new("doctor").about("Check local setup"))
        .subcommand(
            Command::new("update")
                .about("Update the secd binary")
                .arg(Arg::new("check").long("check").action(ArgAction::SetTrue)),
        )
}
