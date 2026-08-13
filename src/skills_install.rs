//! Install the agent skill into Claude and Grok harness dirs.

use std::fs;
use std::path::{Path, PathBuf};

/// Skill body. Same bytes as `skills/grok/SKILL.md` and `skills/claude/SKILL.md`.
pub const SKILL: &[u8] = include_bytes!("../skills/grok/SKILL.md");

/// Write `SKILL.md` under `~/.claude/skills/secd` and `~/.grok/skills/secd`.
pub fn run() -> anyhow::Result<()> {
    for path in dest_files() {
        write_one(&path)?;
    }
    Ok(())
}

/// `SKILL.md` paths for the current `$HOME`.
pub fn dest_files() -> [PathBuf; 2] {
    let home = user_home();
    [
        home.join(".claude/skills/secd/SKILL.md"),
        home.join(".grok/skills/secd/SKILL.md"),
    ]
}

fn user_home() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn write_one(path: &Path) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if fs::read(path).ok().as_deref() == Some(SKILL) {
        return Ok(());
    }
    fs::write(path, SKILL)?;
    Ok(())
}
