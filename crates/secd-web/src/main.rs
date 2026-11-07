use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context};
use secd_web::upstream::{Upstream, UpstreamConfig};
use secd_web::{app, AppState};
use zeroize::Zeroizing;

struct Args {
    hostname: Option<String>,
    cert: PathBuf,
    key: PathBuf,
    data: PathBuf,
    oidc: OidcArgs,
}

#[derive(Default)]
struct OidcArgs {
    issuer: Option<String>,
    client_id: Option<String>,
    secret_file: Option<PathBuf>,
    ca: Option<PathBuf>,
    name: Option<String>,
    required: bool,
}

impl OidcArgs {
    /// All of issuer, client id, secret file and CA, or none of them.
    fn config(self) -> anyhow::Result<Option<UpstreamConfig>> {
        let any = self.issuer.is_some()
            || self.client_id.is_some()
            || self.secret_file.is_some()
            || self.ca.is_some()
            || self.name.is_some()
            || self.required;
        if !any {
            return Ok(None);
        }
        let issuer = self
            .issuer
            .context("--oidc-issuer is required with any --oidc-* flag")?;
        let client_id = self.client_id.context("--oidc-client-id is required")?;
        let secret_file = self
            .secret_file
            .context("--oidc-client-secret-file is required")?;
        let ca = self
            .ca
            .context("--oidc-ca is required: name the CA the provider's certificate chains to")?;
        let secret = Zeroizing::new(
            std::fs::read_to_string(&secret_file)
                .with_context(|| format!("read {}", secret_file.display()))?,
        );
        let client_secret = Zeroizing::new(secret.trim_end_matches(['\r', '\n']).to_string());
        let ca_pem = std::fs::read(&ca).with_context(|| format!("read {}", ca.display()))?;
        Ok(Some(UpstreamConfig {
            issuer,
            client_id,
            client_secret,
            ca_pem,
            name: self.name.unwrap_or_else(|| "your identity provider".into()),
            required: self.required,
        }))
    }
}

fn parse_args() -> anyhow::Result<Args> {
    let mut hostname = None;
    let mut cert = None;
    let mut key = None;
    let mut data = None;
    let mut oidc = OidcArgs::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--hostname" => hostname = Some(need(&mut args, "--hostname")?),
            "--cert" => cert = Some(PathBuf::from(need(&mut args, "--cert")?)),
            "--key" => key = Some(PathBuf::from(need(&mut args, "--key")?)),
            "--data" => data = Some(PathBuf::from(need(&mut args, "--data")?)),
            "--oidc-issuer" => oidc.issuer = Some(need(&mut args, "--oidc-issuer")?),
            "--oidc-client-id" => oidc.client_id = Some(need(&mut args, "--oidc-client-id")?),
            "--oidc-client-secret-file" => {
                oidc.secret_file =
                    Some(PathBuf::from(need(&mut args, "--oidc-client-secret-file")?))
            }
            "--oidc-ca" => oidc.ca = Some(PathBuf::from(need(&mut args, "--oidc-ca")?)),
            "--oidc-name" => oidc.name = Some(need(&mut args, "--oidc-name")?),
            "--oidc-required" => oidc.required = true,
            "-h" | "--help" => {
                eprintln!(
                    "secd-web [--hostname HOST] --cert PATH --key PATH --data PATH\n\
                     \x20        [--oidc-issuer URL --oidc-client-id ID \
                     --oidc-client-secret-file PATH --oidc-ca PEM\n\
                     \x20         [--oidc-name NAME] [--oidc-required]]\n\
                     --hostname sets the WebAuthn RP ID and origin; defaults to \
                     secd_web::state::RP_ID / ORIGIN when omitted.\n\
                     --oidc-* puts a central OpenID Provider you control in front of \
                     sign-in; --oidc-required makes it mandatory."
                );
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    Ok(Args {
        hostname,
        cert: cert.context("missing --cert")?,
        key: key.context("missing --key")?,
        data: data.context("missing --data")?,
        oidc,
    })
}

fn need(args: &mut impl Iterator<Item = String>, flag: &str) -> anyhow::Result<String> {
    args.next()
        .with_context(|| format!("missing value for {flag}"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let mut state = AppState::open_with_hostname(&args.data, args.hostname.as_deref())?;
    if let Some(cfg) = args.oidc.config()? {
        let upstream = Upstream::new(cfg, &state.origin)?;
        state = state.with_upstream(upstream);
    }
    let router = app(state);
    let tls = secd_web::tls::rustls_config(&args.cert, &args.key)?;
    let addr = SocketAddr::from(([0, 0, 0, 0], 443));
    axum_server::bind_rustls(addr, tls)
        .serve(router.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .context("listen 0.0.0.0:443")?;
    Ok(())
}
