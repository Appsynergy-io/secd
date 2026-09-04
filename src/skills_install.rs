//! Install the agent skill into Claude and Grok harness dirs.

use std::fs;
use std::path::{Path, PathBuf};

/// Skill body. Same bytes as `skills/grok/SKILL.md` and `skills/claude/SKILL.md`.
pub const SKILL: &[u8] = include_bytes!("../skills/grok/SKILL.md");

/// Write `SKILL.md` under `~/.claude/skills/secd` and `~/.grok/skills/secd`,
/// then retire the skill it replaces.
pub fn run() -> anyhow::Result<()> {
    for path in dest_files() {
        write_one(&path)?;
    }
    // Only once the new skill is on disk: two resident skills contradict each
    // other about `get`, rotation and grants, and the wrong one is the one that
    // still exists.
    for dir in stale_dirs() {
        remove_dir(&dir);
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

/// Skill directories this skill supersedes, for the current `$HOME`.
pub fn stale_dirs() -> [PathBuf; 2] {
    let home = user_home();
    [
        home.join(".claude/skills/sdxd"),
        home.join(".grok/skills/sdxd"),
    ]
}

/// Best effort. A skill that could not be removed is not a failed update, and
/// an absent one is the state we wanted.
fn remove_dir(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
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
