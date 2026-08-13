use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

pub fn server_config(cert: &Path, key: &Path) -> anyhow::Result<ServerConfig> {
    let cert_pem = std::fs::read(cert).with_context(|| format!("read cert {}", cert.display()))?;
    let key_pem = std::fs::read(key).with_context(|| format!("read key {}", key.display()))?;
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_pem.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .context("parse cert pem")?;
    if certs.is_empty() {
        return Err(anyhow!("no certificates in {}", cert.display()));
    }
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_pem.as_slice())
        .context("parse key pem")?
        .ok_or_else(|| anyhow!("no private key in {}", key.display()))?;
    let mut cfg = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("rustls server cert")?;
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(cfg)
}

pub fn rustls_config(
    cert: &Path,
    key: &Path,
) -> anyhow::Result<axum_server::tls_rustls::RustlsConfig> {
    let cfg = server_config(cert, key)?;
    Ok(axum_server::tls_rustls::RustlsConfig::from_config(
        Arc::new(cfg),
    ))
}
