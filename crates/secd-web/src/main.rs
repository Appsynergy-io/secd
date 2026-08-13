use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context};
use secd_web::{app, AppState};

struct Args {
    hostname: String,
    cert: PathBuf,
    key: PathBuf,
    data: PathBuf,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut hostname = None;
    let mut cert = None;
    let mut key = None;
    let mut data = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--hostname" => hostname = Some(need(&mut args, "--hostname")?),
            "--cert" => cert = Some(PathBuf::from(need(&mut args, "--cert")?)),
            "--key" => key = Some(PathBuf::from(need(&mut args, "--key")?)),
            "--data" => data = Some(PathBuf::from(need(&mut args, "--data")?)),
            "-h" | "--help" => {
                eprintln!("secd-web --hostname HOST --cert PATH --key PATH --data PATH");
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    Ok(Args {
        hostname: hostname.context("missing --hostname")?,
        cert: cert.context("missing --cert")?,
        key: key.context("missing --key")?,
        data: data.context("missing --data")?,
    })
}

fn need(args: &mut impl Iterator<Item = String>, flag: &str) -> anyhow::Result<String> {
    args.next()
        .with_context(|| format!("missing value for {flag}"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let _hostname = args.hostname;
    let state = AppState::open(&args.data)?;
    let router = app(state);
    let tls = secd_web::tls::rustls_config(&args.cert, &args.key)?;
    let addr = SocketAddr::from(([0, 0, 0, 0], 443));
    axum_server::bind_rustls(addr, tls)
        .serve(router.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .context("listen 0.0.0.0:443")?;
    Ok(())
}
