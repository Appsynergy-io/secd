//! git credential helper. Parent must be git; host must match the bundle url.

use std::io::{self, Read, Write};

use crate::policy;
use zeroize::Zeroize;

pub fn run() -> anyhow::Result<()> {
    if !parent_is_git() {
        return Ok(());
    }
    // git names the operation in argv. `store` and `erase` are git telling us
    // what it did with a credential; only `get` is a question, and only a
    // question deserves a password line.
    if !matches!(git_action().as_deref(), None | Some("get")) {
        return Ok(());
    }
    let mut req = String::new();
    io::stdin().read_to_string(&mut req)?;
    match fill(&req) {
        Ok(Some(mut body)) => {
            let mut out = io::stdout().lock();
            let _ = out.write_all(body.as_bytes());
            let _ = out.flush();
            body.zeroize();
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => {
            if e.to_string() == policy::LOCKED {
                return Err(e);
            }
            Ok(())
        }
    }
}

/// True when the parent process is `git` (or `git-*`).
pub fn parent_is_git() -> bool {
    parent_name()
        .map(|n| n == "git" || n.starts_with("git-"))
        .unwrap_or(false)
}

/// Build the credential protocol response. `None` is refuse (no password line).
pub fn fill(request: &str) -> anyhow::Result<Option<String>> {
    let host = match protocol_field(request, "host") {
        Some(h) => h,
        None => return Ok(None),
    };
    let protocol = protocol_field(request, "protocol").unwrap_or_else(|| "https".into());
    let unlocked = policy::require_unlocked()?;
    let entries = policy::load_entries(&unlocked.token, &unlocked.dek)?;
    let bundles = policy::discover_bundles(&entries);
    let want = requested_bundle();
    // The host git asked for is the whole selector: a bundle that serves a
    // different one is not a fallback, it is the wrong credential.
    let Some(bundle) = policy::pick_forge(&bundles, want.as_deref(), &host) else {
        return Ok(None);
    };
    let Some(token) = policy::forge_token(bundle) else {
        return Ok(None);
    };
    let user = policy::forge_user(bundle);
    let mut out = String::new();
    out.push_str("protocol=");
    out.push_str(&protocol);
    out.push('\n');
    out.push_str("host=");
    out.push_str(&host);
    out.push('\n');
    out.push_str("username=");
    out.push_str(user);
    out.push('\n');
    out.push_str("password=");
    out.push_str(token);
    out.push('\n');
    Ok(Some(out))
}

fn requested_bundle() -> Option<String> {
    parse_args(helper_args()).0
}

/// The operation git named: `get`, `store` or `erase`.
fn git_action() -> Option<String> {
    parse_args(helper_args()).1
}

/// `(--bundle, action)`. `--bundle` takes a value, so the action is the first
/// bare word that is not one.
fn parse_args(mut args: impl Iterator<Item = String>) -> (Option<String>, Option<String>) {
    let mut want = None;
    let mut action = None;
    while let Some(a) = args.next() {
        if a == "--bundle" {
            want = args.next();
        } else if let Some(v) = a.strip_prefix("--bundle=") {
            want = Some(v.to_string());
        } else if !a.starts_with('-') && action.is_none() {
            action = Some(a);
        }
    }
    (want, action)
}

fn helper_args() -> impl Iterator<Item = String> {
    std::env::args()
        .skip_while(|a| a != "git-credential")
        .skip(1)
}

fn protocol_field(req: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    for line in req.lines() {
        if let Some(v) = line.strip_prefix(&prefix) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn parent_name() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let ppid = status
            .lines()
            .find(|l| l.starts_with("PPid:"))?
            .split_whitespace()
            .nth(1)?
            .parse::<u32>()
            .ok()?;
        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{ppid}/comm")) {
            let name = comm.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        if let Ok(exe) = std::fs::read_link(format!("/proc/{ppid}/exe")) {
            if let Some(name) = exe.file_name().and_then(|s| s.to_str()) {
                return Some(name.to_string());
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        parent_name_macos()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn parent_name_macos() -> Option<String> {
    extern "C" {
        fn getppid() -> i32;
        fn proc_pidpath(pid: i32, buffer: *mut u8, buffersize: u32) -> i32;
    }
    let ppid = unsafe { getppid() };
    if ppid <= 1 {
        return None;
    }
    let mut buf = [0u8; 4096];
    let n = unsafe { proc_pidpath(ppid, buf.as_mut_ptr(), buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    let path = std::str::from_utf8(&buf[..n as usize]).ok()?;
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_string)
}
