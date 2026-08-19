#![allow(non_snake_case)]
//! Agents touch this repo from Claude sessions, local and sandboxed, and from
//! other harnesses. The guards that keep them off main and away from
//! credentials are only worth having if they actually fire, so this runs them.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Feed a Bash tool call to the PreToolUse guard; true means it was allowed.
fn allowed(command: &str) -> bool {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": command},
    })
    .to_string();

    let mut child = Command::new(root().join(".claude/hooks/guard-bash.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn guard-bash.py");
    child
        .stdin
        .as_mut()
        .expect("guard stdin")
        .write_all(payload.as_bytes())
        .expect("write payload");
    child.wait().expect("guard exit").success()
}

#[test]
fn T_AGENT_SETUP() {
    let settings = std::fs::read_to_string(root().join(".claude/settings.json"))
        .expect(".claude/settings.json");
    let parsed: serde_json::Value = serde_json::from_str(&settings).expect("settings.json is json");

    for event in ["SessionStart", "PreToolUse"] {
        assert!(
            parsed["hooks"][event].is_array(),
            "settings.json registers no {event} hook"
        );
    }
    assert!(
        settings.contains("session-start.sh") && settings.contains("guard-bash.py"),
        "settings.json does not point at the hooks in .claude/hooks"
    );

    for hook in ["session-start.sh", "guard-bash.py"] {
        let path = root().join(".claude/hooks").join(hook);
        assert!(path.is_file(), "missing {}", path.display());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path)
                .expect("stat hook")
                .permissions()
                .mode();
            assert!(mode & 0o111 != 0, "{hook} is not executable");
        }
    }
}

#[test]
fn T_AGENT_GUARD_REFUSES() {
    for command in [
        "git push origin main",
        "git push --force origin topic",
        "git push --no-verify origin topic",
        "secd run -- env",
        "secd gitea -- git push",
        "secd git-credential",
        "gh release create v9.9.9",
        "docker push ghcr.io/appsynergy-io/secd-web:9",
        "cosign sign ghcr.io/x@sha256:abc",
        "scripts/publish-release.sh --tag v9.9.9",
        "scripts/k3s-apply.sh",
        "scripts/release.sh --target x86_64-unknown-linux-musl",
    ] {
        assert!(!allowed(command), "the guard allowed `{command}`");
    }
}

#[test]
fn T_AGENT_GUARD_ALLOWS() {
    // A guard that refuses ordinary work gets turned off.
    for command in [
        "git push -u origin claude/some-branch",
        "git log --oneline -f",
        "grep -f patterns file",
        "cargo test --locked --workspace",
        "scripts/check.sh fast",
        "scripts/release.sh --target x86_64-unknown-linux-musl --dry-run",
        // Reading a guarded script is not running it. Matching the bare name
        // anywhere refused all of these.
        "sed -n '1,5p' scripts/release.sh",
        "grep -n cosign scripts/publish-release.sh",
        "cat scripts/k3s-apply.sh",
        "shellcheck -x scripts/release.sh scripts/k3s-apply.sh",
        "wc -l scripts/publish-release.sh deploy/agent/secd-agent.sh",
        // Markdown inline code spells script names the same way a backtick
        // substitution would, and this repo's prose is full of them.
        "python3 - <<'P'\nt = t.replace('`k3s-apply.sh` refuses', 'x')\nP",
    ] {
        assert!(allowed(command), "the guard refused `{command}`");
    }
}

/// The read exemption must not become an escape hatch: the same script names in
/// command position are still refused, however they are reached.
#[test]
fn T_AGENT_GUARD_INVOCATION_FORMS() {
    for command in [
        "bash scripts/publish-release.sh",
        "sh ./scripts/k3s-apply.sh",
        "./scripts/k3s-apply.sh --expect-digest sha256:0",
        "make build && scripts/release.sh --target x",
        "cd /tmp; scripts/release.sh --target x",
        "echo $(scripts/k3s-apply.sh)",
        "exec scripts/publish-release.sh --tag v9.9.9",
    ] {
        assert!(!allowed(command), "the guard allowed `{command}`");
    }
}
