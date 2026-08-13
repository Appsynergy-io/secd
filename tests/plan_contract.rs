#![allow(non_snake_case)]

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

const CONTRACT: &str = include_str!("../contract.toml");

const ROUTES: &[&str] = &[
    "POST /api/auth/start",
    "POST /api/auth/passkey/register/start",
    "POST /api/auth/passkey/register/finish",
    "POST /api/auth/passkey/login/start",
    "POST /api/auth/passkey/login/finish",
    "POST /api/auth/password/register",
    "POST /api/auth/password/login",
    "POST /api/auth/logout",
    "GET /api/session",
    "GET /api/v1/sessions",
    "DELETE /api/v1/sessions/:id",
    "GET /api/auth/passkeys",
    "DELETE /api/auth/passkeys/:id",
    "POST /api/v1/device/start",
    "POST /api/v1/device/poll",
    "POST /api/v1/device/approve",
    "POST /api/v1/device/revoke",
    "GET /api/v1/vault",
    "PUT /api/v1/vault",
    "GET /api/v1/providers",
    "PUT /api/v1/providers",
    "DELETE /api/v1/providers/:name",
    "GET /api/v1/audit",
];

const PROVIDERS: &[&str] = &[
    "cloudflare",
    "aws",
    "s3",
    "github",
    "gitea",
    "gitlab",
    "slack",
    "digitalocean",
    "npm",
    "xai",
    "sendgrid",
    "pypi",
    "anthropic",
    "openai",
    "vault",
];

fn parse_string_array(src: &str, key: &str) -> Vec<String> {
    let header = format!("{key} = [");
    let start = src
        .find(&header)
        .unwrap_or_else(|| panic!("missing {key} array"));
    let rest = &src[start + header.len()..];
    let end = rest.find(']').expect("array end");
    rest[..end]
        .lines()
        .map(str::trim)
        .map(|line| line.trim_end_matches(',').trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_matches('"').to_string())
        .collect()
}

#[test]
fn T_CONTRACT_COMMANDS() {
    let mut contract = parse_string_array(CONTRACT, "commands");
    let mut clap: Vec<String> = secd::cli()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    contract.sort();
    clap.sort();
    assert_eq!(clap, contract);
}

#[test]
fn T_CONTRACT_NO_GET() {
    let mut cmd = secd::cli();
    assert!(cmd.find_subcommand("get").is_none());
    assert!(secd::cli().try_get_matches_from(["secd", "get"]).is_err());
    let mut help = Vec::new();
    cmd.write_long_help(&mut help).expect("help");
    let help = String::from_utf8(help).expect("utf8 help");
    for line in help.lines() {
        let trimmed = line.trim();
        assert_ne!(trimmed, "get", "help lists get: {line}");
        assert!(
            !trimmed.starts_with("get ") && !trimmed.starts_with("get\t"),
            "help lists get: {line}"
        );
    }
}

#[test]
fn T_CONTRACT_ROUTES() {
    let contract = parse_string_array(CONTRACT, "routes");
    assert_eq!(contract, ROUTES);
    let web = Path::new("crates/secd-web/src");
    if !web.exists() {
        return;
    }
    let contract_paths: BTreeSet<&str> = ROUTES
        .iter()
        .map(|row| row.split_once(' ').map(|(_, p)| p).unwrap_or(row))
        .collect();
    let mut extras = Vec::new();
    visit_rs(web, &mut |src| {
        let mut rest = src;
        while let Some(i) = rest.find(".route(") {
            rest = &rest[i + 7..];
            let rest_t = rest.trim_start();
            let Some(s) = rest_t.strip_prefix('"') else {
                continue;
            };
            let Some(end) = s.find('"') else {
                continue;
            };
            let path = &s[..end];
            let ok =
                contract_paths.contains(path) || contract_paths.iter().any(|p| p.ends_with(path));
            if !ok {
                extras.push(path.to_string());
            }
        }
    });
    assert!(extras.is_empty(), "axum paths not in [routes]: {extras:?}");
}

#[test]
fn T_CONTRACT_PROVIDERS() {
    let contract = parse_string_array(CONTRACT, "providers");
    assert_eq!(contract, PROVIDERS);
}

#[test]
fn T_CONTRACT_ONE_DOC() {
    assert!(Path::new("AGENTS.md").is_file());
    assert!(Path::new("CLAUDE.md").is_file());
    let output = Command::new("git")
        .args(["ls-files", "--", "*.md"])
        .output()
        .expect("git ls-files");
    assert!(output.status.success(), "git ls-files failed");
    let tracked = String::from_utf8(output.stdout).expect("utf8");
    let allowed = [
        "AGENTS.md",
        "CLAUDE.md",
        "skills/grok/SKILL.md",
        "skills/claude/SKILL.md",
    ];
    for file in tracked.lines().filter(|l| !l.is_empty()) {
        assert!(
            allowed.contains(&file),
            "unexpected tracked markdown: {file}"
        );
    }
}

#[test]
fn T_CONTRACT_AGENTS_EQ_CLAUDE() {
    let agents = fs::read("AGENTS.md").expect("AGENTS.md");
    let claude = fs::read("CLAUDE.md").expect("CLAUDE.md");
    assert_eq!(agents, claude);
}

fn visit_rs(dir: &Path, f: &mut impl FnMut(&str)) {
    let entries = fs::read_dir(dir).expect("read dir");
    for entry in entries {
        let entry = entry.expect("dirent");
        let path = entry.path();
        if path.is_dir() {
            visit_rs(&path, f);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let src = fs::read_to_string(&path).expect("read rust");
            f(&src);
        }
    }
}
