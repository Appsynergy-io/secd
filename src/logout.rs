//! Drop DEK from the keyring and revoke the HTTP device session.

pub fn run() -> anyhow::Result<()> {
    if let Some(token) = crate::login::load_session() {
        crate::policy::revoke_session(&token);
    }
    let path = crate::login::session_path();
    let _ = std::fs::remove_file(path);
    crate::keyring::delete()?;
    Ok(())
}
