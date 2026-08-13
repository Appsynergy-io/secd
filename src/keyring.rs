//! DEK in the kernel keyring (Linux) or macOS keychain. Not a file.

use secd_core::Secret;

const DEK_LEN: usize = 32;

/// Persist the 32-byte DEK until logout or reboot.
pub fn store(dek: &[u8]) -> anyhow::Result<()> {
    if dek.len() != DEK_LEN {
        anyhow::bail!("dek must be 32 bytes");
    }
    let _ = delete();
    backend::store(dek)
}

/// Load the DEK for this `SECD_HOME`. Missing key is `None`.
pub fn load() -> Option<Secret> {
    let bytes = backend::load()?;
    if bytes.len() != DEK_LEN {
        return None;
    }
    Some(Secret::new(bytes))
}

/// Remove the DEK. Missing key is success.
pub fn delete() -> anyhow::Result<()> {
    backend::delete()
}

fn description() -> String {
    let home = crate::login::home();
    let raw = home.to_string_lossy();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in raw.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("secd-dek-{h:016x}")
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod backend {
    use super::description;
    use anyhow::Context;
    use std::ffi::CString;

    const SYS_ADD_KEY: i64 = 248;
    const SYS_REQUEST_KEY: i64 = 249;
    const SYS_KEYCTL: i64 = 250;
    const KEY_SPEC_USER_KEYRING: i64 = -4;
    const KEYCTL_READ: i64 = 11;
    const KEYCTL_INVALIDATE: i64 = 21;

    pub(super) fn store(dek: &[u8]) -> anyhow::Result<()> {
        let typ = CString::new("user").expect("invariant: type has no nul");
        let desc = CString::new(description()).context("key description")?;
        let id = sys5(
            SYS_ADD_KEY,
            typ.as_ptr() as i64,
            desc.as_ptr() as i64,
            dek.as_ptr() as i64,
            dek.len() as i64,
            KEY_SPEC_USER_KEYRING,
        );
        if id < 0 {
            anyhow::bail!("keyring add failed ({})", -id);
        }
        Ok(())
    }

    pub(super) fn load() -> Option<Vec<u8>> {
        let id = find()?;
        let mut buf = vec![0u8; 64];
        let n = sys5(
            SYS_KEYCTL,
            KEYCTL_READ,
            id,
            buf.as_mut_ptr() as i64,
            buf.len() as i64,
            0,
        );
        if n < 0 {
            return None;
        }
        let n = n as usize;
        if n > buf.len() {
            buf.resize(n, 0);
            let n2 = sys5(
                SYS_KEYCTL,
                KEYCTL_READ,
                id,
                buf.as_mut_ptr() as i64,
                buf.len() as i64,
                0,
            );
            if n2 < 0 {
                return None;
            }
            buf.truncate(n2 as usize);
        } else {
            buf.truncate(n);
        }
        Some(buf)
    }

    pub(super) fn delete() -> anyhow::Result<()> {
        let Some(id) = find() else {
            return Ok(());
        };
        let rc = sys5(SYS_KEYCTL, KEYCTL_INVALIDATE, id, 0, 0, 0);
        if rc < 0 {
            anyhow::bail!("keyring delete failed ({})", -rc);
        }
        Ok(())
    }

    fn find() -> Option<i64> {
        let typ = CString::new("user").ok()?;
        let desc = CString::new(description()).ok()?;
        let id = sys5(
            SYS_REQUEST_KEY,
            typ.as_ptr() as i64,
            desc.as_ptr() as i64,
            0,
            KEY_SPEC_USER_KEYRING,
            0,
        );
        if id < 0 {
            None
        } else {
            Some(id)
        }
    }

    fn sys5(nr: i64, a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {
        let ret: i64;
        // SAFETY: Linux x86_64 syscall ABI; r10 holds arg4.
        unsafe {
            core::arch::asm!(
                "syscall",
                inout("rax") nr => ret,
                in("rdi") a,
                in("rsi") b,
                in("rdx") c,
                in("r10") d,
                in("r8") e,
                lateout("rcx") _,
                lateout("r11") _,
                options(nostack)
            );
        }
        ret
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use super::description;
    use anyhow::Context;
    use std::process::Stdio;

    pub(super) fn store(dek: &[u8]) -> anyhow::Result<()> {
        let hex = hex::encode(dek);
        let service = description();
        let bin = "security";
        let status = std::process::Command::new(bin)
            .args([
                "add-generic-password",
                "-a",
                "secd",
                "-s",
                &service,
                "-U",
                "-w",
                &hex,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("security add")?;
        if !status.success() {
            anyhow::bail!("keychain add failed");
        }
        Ok(())
    }

    pub(super) fn load() -> Option<Vec<u8>> {
        let service = description();
        let bin = "security";
        let out = std::process::Command::new(bin)
            .args(["find-generic-password", "-a", "secd", "-s", &service, "-w"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let hex = String::from_utf8(out.stdout).ok()?;
        hex::decode(hex.trim()).ok()
    }

    pub(super) fn delete() -> anyhow::Result<()> {
        let service = description();
        let bin = "security";
        let status = std::process::Command::new(bin)
            .args(["delete-generic-password", "-a", "secd", "-s", &service])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("security delete")?;
        if !status.success() {
            // missing item is success
            return Ok(());
        }
        Ok(())
    }
}

#[cfg(not(any(all(target_os = "linux", target_arch = "x86_64"), target_os = "macos")))]
mod backend {
    pub(super) fn store(_: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("keyring: unsupported target")
    }
    pub(super) fn load() -> Option<Vec<u8>> {
        None
    }
    pub(super) fn delete() -> anyhow::Result<()> {
        Ok(())
    }
}
