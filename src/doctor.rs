//! Local setup. No values.

use std::os::unix::fs::PermissionsExt;

pub fn run() -> anyhow::Result<()> {
    let home = crate::login::home();
    println!("home {}", home.display());
    let session = crate::login::session_path();
    match std::fs::metadata(&session) {
        Ok(meta) => {
            let mode = meta.permissions().mode() & 0o777;
            println!("session {mode:03o}");
        }
        Err(_) => println!("session missing"),
    }
    let dek = crate::keyring::load();
    match &dek {
        Some(_) => println!("dek yes"),
        None => println!("dek missing"),
    }
    if crate::login::load_session().is_none() || dek.is_none() {
        println!("locked");
    }
    println!("host secd.imabee.com:443");
    Ok(())
}
