fn main() -> anyhow::Result<()> {
    match secd::cli().get_matches().subcommand() {
        None => secd::tui::run(),
        Some(("logout", _)) => secd::logout::run(),
        Some(("gitea", _)) => secd::gitea::run(),
        Some(("git-credential", _)) => secd::git_credential::run(),
        Some(("run", _)) => secd::run::run(),
        Some(("ls", _)) => secd::ls::run(),
        Some(("info", _)) => secd::info::run(),
        Some(("providers", _)) => secd::providers::run(),
        Some(("redact", _)) => secd::redact::run(),
        Some(("gen", _)) => secd::gen::run(),
        Some(("doctor", _)) => secd::doctor::run(),
        Some(("update", _)) => secd::update::run(),
        Some(_) => unreachable!("clap rejects unknown subcommands"),
    }
}
