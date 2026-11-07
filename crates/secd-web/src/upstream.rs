//! A central OpenID Provider you control, in front of secd's own sign-in.
//!
//! secd is a confidential relying party using the authorization code flow
//! with PKCE (OIDC Core §3.1, RFC 7636 S256). Every outbound call is HTTPS to
//! endpoints the provider's discovery names, trusting only the CA configured
//! for it: a provider you control is a provider whose CA you name.
//!
//! A verified upstream identity is not a session. It proves who is at this
//! browser; the vault key still comes from a passkey or password, and so does
//! the console session. With `required`, passkey and password sign-in are
//! refused unless the provider vouched for that same account within the last
//! ten minutes. Existing secd sessions retain their own expiry and revocation.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use secd_core::jwt::{self, Expect, JwtError};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::headers::{json_status, json_value};
use crate::state::AppState;

const FLOW_TTL: Duration = Duration::from_secs(10 * 60);
const VERIFIED_TTL: Duration = Duration::from_secs(10 * 60);
const META_TTL: Duration = Duration::from_secs(60 * 60);
const JWKS_TTL: Duration = Duration::from_secs(10 * 60);
/// A token naming a key we do not hold refetches the JWK Set, but no more
/// often than this: an attacker cannot turn bad tokens into fetch traffic.
const JWKS_MIN_REFRESH: Duration = Duration::from_secs(30);
const MAX_FLOWS: usize = 256;
const RESP_CAP: usize = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const FLOW_COOKIE: &str = "__Host-secd-oidc";
const IDP_COOKIE: &str = "__Host-secd-idp";
pub const CALLBACK_PATH: &str = "/api/auth/oidc/callback";

pub struct UpstreamConfig {
    /// Exactly as the provider's discovery states it.
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Zeroizing<String>,
    /// PEM roots trusted for every call to this provider, and only these.
    pub ca_pem: Vec<u8>,
    /// What the sign-in button calls the provider.
    pub name: String,
    pub required: bool,
}

/// One HTTPS exchange. A trait so the flow is tested without a network.
pub trait Transport: Send + Sync {
    fn get(&self, url: &str) -> anyhow::Result<(u16, Vec<u8>)>;
    fn post_form(&self, url: &str, form: &str, basic: &str) -> anyhow::Result<(u16, Vec<u8>)>;
}

struct Meta {
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    fetched: Instant,
}

struct Flow {
    return_to: String,
    nonce: String,
    verifier: Zeroizing<String>,
    binding: [u8; 32],
    created: Instant,
}

struct Verified {
    email: String,
    at: Instant,
}

pub struct Upstream {
    cfg: UpstreamConfig,
    redirect_uri: String,
    transport: Arc<dyn Transport>,
    meta: Mutex<Option<Meta>>,
    jwks: Mutex<Option<(Value, Instant)>>,
    flows: Mutex<HashMap<String, Flow>>,
    verified: Mutex<HashMap<[u8; 32], Verified>>,
}

impl Upstream {
    pub fn new(cfg: UpstreamConfig, origin: &str) -> anyhow::Result<Self> {
        let transport = Arc::new(Https::new(&cfg.ca_pem)?);
        Self::with_transport(cfg, origin, transport)
    }

    pub fn with_transport(
        cfg: UpstreamConfig,
        origin: &str,
        transport: Arc<dyn Transport>,
    ) -> anyhow::Result<Self> {
        https_only(&cfg.issuer)?;
        https_only(origin)?;
        let iss = url::Url::parse(&cfg.issuer).context("--oidc-issuer")?;
        if iss.scheme() != "https" || iss.query().is_some() || iss.fragment().is_some() {
            bail!("--oidc-issuer must be an https URL without query or fragment");
        }
        if cfg.client_id.is_empty() || cfg.client_secret.is_empty() {
            bail!("upstream OIDC needs a client id and secret");
        }
        Ok(Self {
            redirect_uri: format!("{origin}{CALLBACK_PATH}"),
            cfg,
            transport,
            meta: Mutex::new(None),
            jwks: Mutex::new(None),
            flows: Mutex::new(HashMap::new()),
            verified: Mutex::new(HashMap::new()),
        })
    }

    pub fn name(&self) -> &str {
        &self.cfg.name
    }

    pub fn required(&self) -> bool {
        self.cfg.required
    }

    fn fetch_json(&self, url: &str) -> anyhow::Result<Value> {
        https_only(url)?;
        let (status, body) = self.transport.get(url)?;
        if status != 200 {
            bail!("GET {url}: {status}");
        }
        serde_json::from_slice(&body).with_context(|| format!("GET {url}: not JSON"))
    }

    /// OIDC Discovery §4: the document's issuer must be the configured one.
    fn meta(&self) -> anyhow::Result<(String, String, String)> {
        {
            let g = lock(&self.meta);
            if let Some(m) = g.as_ref().filter(|m| m.fetched.elapsed() < META_TTL) {
                return Ok((
                    m.authorization_endpoint.clone(),
                    m.token_endpoint.clone(),
                    m.jwks_uri.clone(),
                ));
            }
        }
        let url = format!(
            "{}/.well-known/openid-configuration",
            self.cfg.issuer.trim_end_matches('/')
        );
        let doc = self.fetch_json(&url)?;
        if doc.get("issuer").and_then(Value::as_str) != Some(self.cfg.issuer.as_str()) {
            bail!("discovery names a different issuer");
        }
        let field = |k: &str| -> anyhow::Result<String> {
            let v = doc
                .get(k)
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("discovery: no {k}"))?;
            https_only(v)?;
            Ok(v.to_string())
        };
        let m = Meta {
            authorization_endpoint: field("authorization_endpoint")?,
            token_endpoint: field("token_endpoint")?,
            jwks_uri: field("jwks_uri")?,
            fetched: Instant::now(),
        };
        let out = (
            m.authorization_endpoint.clone(),
            m.token_endpoint.clone(),
            m.jwks_uri.clone(),
        );
        *lock(&self.meta) = Some(m);
        Ok(out)
    }

    fn jwks(&self, jwks_uri: &str, refresh: bool) -> anyhow::Result<Value> {
        {
            let g = lock(&self.jwks);
            if let Some((keys, at)) = g.as_ref() {
                let fresh = at.elapsed() < JWKS_TTL;
                if fresh && !(refresh && at.elapsed() >= JWKS_MIN_REFRESH) {
                    return Ok(keys.clone());
                }
            }
        }
        let keys = self.fetch_json(jwks_uri)?;
        *lock(&self.jwks) = Some((keys.clone(), Instant::now()));
        Ok(keys)
    }

    /// Begin a sign-in: the URL to send the browser to, and the cookie value
    /// that binds the flow to this browser.
    pub fn begin(&self, return_to: &str) -> anyhow::Result<(String, String)> {
        let return_to = checked_return(return_to)?;
        let (authorize, _, _) = self.meta()?;
        let state = random_b64(32);
        let nonce = random_b64(32);
        let verifier = Zeroizing::new(random_b64(32));
        let binding = random_b64(32);
        let challenge = jwt::pkce_challenge(&verifier);
        {
            let mut g = lock(&self.flows);
            g.retain(|_, f| f.created.elapsed() < FLOW_TTL);
            if g.len() >= MAX_FLOWS {
                bail!("too many sign-ins in flight");
            }
            g.insert(
                state.clone(),
                Flow {
                    return_to,
                    nonce: nonce.clone(),
                    verifier,
                    binding: crate::audit::sha256(binding.as_bytes()),
                    created: Instant::now(),
                },
            );
        }
        let mut url = url::Url::parse(&authorize)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.cfg.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", "openid email")
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        Ok((url.into(), binding))
    }

    /// Finish a sign-in: exchange the code, verify the ID token, and return
    /// the secd account it names.
    pub fn finish(
        &self,
        state: &str,
        code: &str,
        binding: Option<&str>,
        known: impl Fn(&str) -> bool,
    ) -> anyhow::Result<(String, String)> {
        // The state is single-use whatever happens next.
        let flow = lock(&self.flows)
            .remove(state)
            .ok_or_else(|| anyhow!("unknown or used state"))?;
        if flow.created.elapsed() >= FLOW_TTL {
            bail!("sign-in took too long");
        }
        let presented = crate::audit::sha256(binding.unwrap_or_default().as_bytes());
        if binding.is_none() || !ct_eq(&presented, &flow.binding) {
            bail!("sign-in was started in another browser");
        }
        let (_, token_endpoint, jwks_uri) = self.meta()?;
        let form = Zeroizing::new(
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair("grant_type", "authorization_code")
                .append_pair("code", code)
                .append_pair("redirect_uri", &self.redirect_uri)
                .append_pair("code_verifier", &flow.verifier)
                .finish(),
        );
        // RFC 6749 §2.3.1: client_secret_basic, each part form-encoded.
        let enc = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
        let basic = Zeroizing::new(jwt::b64std(
            format!(
                "{}:{}",
                enc(&self.cfg.client_id),
                enc(&self.cfg.client_secret)
            )
            .as_bytes(),
        ));
        https_only(&token_endpoint)?;
        let (status, body) = self.transport.post_form(&token_endpoint, &form, &basic)?;
        if status != 200 {
            bail!("token endpoint answered {status}");
        }
        let resp: Value = serde_json::from_slice(&body).context("token response")?;
        let id_token = resp
            .get("id_token")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("token response has no id_token"))?;
        let expect = Expect {
            issuer: &self.cfg.issuer,
            audience: &self.cfg.client_id,
            nonce: Some(&flow.nonce),
            now: crate::oidc::unix_now(),
        };
        let claims = match jwt::verify(id_token, &self.jwks(&jwks_uri, false)?, &expect) {
            Err(JwtError::UnknownKey) => {
                jwt::verify(id_token, &self.jwks(&jwks_uri, true)?, &expect)
            }
            other => other,
        }
        .map_err(|e| anyhow!("id_token: {e}"))?;
        let verified = match claims.get("email_verified") {
            Some(Value::Bool(b)) => *b,
            _ => false,
        };
        if !verified {
            bail!("the provider has not verified this email");
        }
        let email = claims
            .get("email")
            .and_then(Value::as_str)
            .and_then(crate::auth::normalize_email)
            .ok_or_else(|| anyhow!("id_token has no usable email"))?;
        if !known(&email) {
            bail!("no secd account for this identity");
        }
        Ok((email, flow.return_to))
    }

    /// Remember a verified identity for this browser; returns the cookie value.
    pub fn remember(&self, email: String) -> String {
        let cookie = random_b64(32);
        let mut g = lock(&self.verified);
        g.retain(|_, v| v.at.elapsed() < VERIFIED_TTL);
        if g.len() >= MAX_FLOWS {
            if let Some(oldest) = g.iter().min_by_key(|(_, v)| v.at).map(|(k, _)| *k) {
                g.remove(&oldest);
            }
        }
        g.insert(
            crate::audit::sha256(cookie.as_bytes()),
            Verified {
                email,
                at: Instant::now(),
            },
        );
        cookie
    }

    /// The account the provider vouched for at this browser, if recently.
    pub fn verified_email(&self, headers: &HeaderMap) -> Option<String> {
        let cookie = cookie_value(headers, IDP_COOKIE)?;
        let g = lock(&self.verified);
        g.get(&crate::audit::sha256(cookie.as_bytes()))
            .filter(|v| v.at.elapsed() < VERIFIED_TTL)
            .map(|v| v.email.clone())
    }

    pub fn forget(&self, headers: &HeaderMap) {
        if let Some(cookie) = cookie_value(headers, IDP_COOKIE) {
            lock(&self.verified).remove(&crate::audit::sha256(cookie.as_bytes()));
        }
    }

    /// Whether a passkey or password sign-in as `email` may proceed.
    pub fn admits(&self, headers: &HeaderMap, email: &str) -> bool {
        !self.cfg.required || self.verified_email(headers).as_deref() == Some(email)
    }
}

/// The gate every passkey and password sign-in passes.
pub fn admits(state: &AppState, headers: &HeaderMap, email: &str) -> bool {
    state
        .upstream
        .as_ref()
        .is_none_or(|u| u.admits(headers, email))
}

pub(crate) fn clear_identity(state: &AppState, headers: &HeaderMap, res: &mut Response) {
    if let Some(upstream) = &state.upstream {
        upstream.forget(headers);
    }
    res.headers_mut()
        .append(header::SET_COOKIE, cookie(IDP_COOKIE, "", 0));
    res.headers_mut()
        .append(header::SET_COOKIE, cookie(FLOW_COOKIE, "", 0));
}

/// Only the console and device approval are valid return targets.
fn checked_return(target: &str) -> anyhow::Result<String> {
    if target.len() > 2048 {
        bail!("return target too long");
    }
    let url = url::Url::parse("https://secd.invalid")?.join(target)?;
    if url.origin().ascii_serialization() != "https://secd.invalid"
        || !matches!(url.path(), "/" | "/device")
        || url.fragment().is_some()
    {
        bail!("invalid return target");
    }
    let mut out = url.path().to_string();
    if let Some(q) = url.query() {
        out.push('?');
        out.push_str(q);
    }
    Ok(out)
}

fn https_only(url: &str) -> anyhow::Result<()> {
    let u = url::Url::parse(url).with_context(|| format!("bad URL {url}"))?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
    {
        bail!("refusing a non-https provider URL");
    }
    Ok(())
}

fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_b64(n: usize) -> String {
    let mut b = vec![0u8; n];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut b);
    jwt::b64url(&b)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (k, v) = part.trim().split_once('=')?;
        (k == name && !v.is_empty()).then(|| v.to_string())
    })
}

fn cookie(name: &str, value: &str, max_age: u64) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age}"
    ))
    .expect("invariant: cookie value is base64url")
}

fn redirect(to: &str, cookies: &[HeaderValue]) -> Response {
    let mut res = (StatusCode::FOUND, "").into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    for c in cookies {
        res.headers_mut().append(header::SET_COOKIE, c.clone());
    }
    res
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/oidc", get(info))
        .route("/api/auth/oidc/start", get(start))
        .route(CALLBACK_PATH, get(callback))
}

async fn info(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(u) = state.upstream.as_ref() else {
        return json_value(StatusCode::OK, json!({ "enabled": false }));
    };
    json_value(
        StatusCode::OK,
        json!({
            "enabled": true,
            "name": u.name(),
            "required": u.required(),
            "verified_email": u.verified_email(&headers),
        }),
    )
}

async fn start(State(state): State<AppState>, uri: Uri) -> Response {
    let Some(u) = state.upstream.clone() else {
        return json_status(StatusCode::NOT_FOUND, "not found");
    };
    let return_to = url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
        .find(|(k, _)| k == "return_to")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| "/".into());
    if checked_return(&return_to).is_err() {
        return json_status(StatusCode::BAD_REQUEST, "return_to");
    }
    match tokio::task::spawn_blocking(move || u.begin(&return_to)).await {
        Ok(Ok((url, binding))) => {
            redirect(&url, &[cookie(FLOW_COOKIE, &binding, FLOW_TTL.as_secs())])
        }
        Ok(Err(e)) => {
            eprintln!("secd-web: oidc start: {e:#}");
            redirect("/?signin=unavailable", &[])
        }
        Err(_) => redirect("/?signin=unavailable", &[]),
    }
}

/// `state` and `code` from the callback query, each exactly once.
fn callback_params(uri: &Uri) -> (Option<String>, Option<String>) {
    let (mut st, mut code, mut dup) = (None, None, false);
    for (k, v) in url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()) {
        if k == "error" {
            return (None, None);
        }
        let slot = match k.as_ref() {
            "state" => &mut st,
            "code" => &mut code,
            _ => continue,
        };
        dup |= slot.is_some();
        *slot = Some(v.into_owned());
    }
    if dup {
        return (None, None);
    }
    (st, code)
}

async fn callback(State(state): State<AppState>, headers: HeaderMap, uri: Uri) -> Response {
    let Some(u) = state.upstream.clone() else {
        return json_status(StatusCode::NOT_FOUND, "not found");
    };
    let clear = cookie(FLOW_COOKIE, "", 0);
    // An `error` from the provider, or anything missing, is the same refusal.
    let (Some(st), Some(code)) = callback_params(&uri) else {
        return redirect("/?signin=failed", &[clear]);
    };
    let binding = cookie_value(&headers, FLOW_COOKIE);
    let users = state.users.clone();
    let u2 = u.clone();
    let done = tokio::task::spawn_blocking(move || {
        u2.finish(&st, &code, binding.as_deref(), |email| {
            users.is_empty() || users.get(email).is_some()
        })
    })
    .await;
    match done {
        Ok(Ok((email, return_to))) => {
            let idp = u.remember(email);
            redirect(
                &format!(
                    "{return_to}{}signin=verified",
                    if return_to.contains('?') { "&" } else { "?" }
                ),
                &[clear, cookie(IDP_COOKIE, &idp, VERIFIED_TTL.as_secs())],
            )
        }
        Ok(Err(e)) => {
            eprintln!("secd-web: oidc callback: {e:#}");
            redirect("/?signin=failed", &[clear])
        }
        Err(_) => redirect("/?signin=failed", &[clear]),
    }
}

/// HTTPS/1.1 over rustls, trusting only the configured roots.
struct Https {
    tls: Arc<rustls::ClientConfig>,
}

impl Https {
    fn new(ca_pem: &[u8]) -> anyhow::Result<Self> {
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_pemfile::certs(&mut &ca_pem[..]) {
            roots
                .add(cert.context("--oidc-ca: bad certificate")?)
                .context("--oidc-ca: unusable certificate")?;
        }
        if roots.is_empty() {
            bail!("--oidc-ca holds no certificate");
        }
        let tls = rustls::ClientConfig::builder_with_provider(
            rustls::crypto::aws_lc_rs::default_provider().into(),
        )
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Self { tls: Arc::new(tls) })
    }

    fn exchange(
        &self,
        url: &str,
        method: &str,
        extra: &str,
        body: &[u8],
    ) -> anyhow::Result<(u16, Vec<u8>)> {
        let u = url::Url::parse(url)?;
        let host = u.host_str().ok_or_else(|| anyhow!("no host"))?.to_string();
        let port = u.port_or_known_default().unwrap_or(443);
        let addr = (host.as_str(), port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("{host}: no address"))?;
        let tcp = TcpStream::connect_timeout(&addr, IO_TIMEOUT)?;
        tcp.set_read_timeout(Some(IO_TIMEOUT))?;
        tcp.set_write_timeout(Some(IO_TIMEOUT))?;
        let name = rustls::pki_types::ServerName::try_from(host.clone())?;
        let conn = rustls::ClientConnection::new(self.tls.clone(), name)?;
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        let mut target = u.path().to_string();
        if let Some(q) = u.query() {
            target.push('?');
            target.push_str(q);
        }
        let host_header = match u.port() {
            Some(p) => format!("{host}:{p}"),
            None => host.clone(),
        };
        let head = format!(
            "{method} {target} HTTP/1.1\r\nHost: {host_header}\r\nAccept: application/json\r\n\
             User-Agent: secd-web\r\nConnection: close\r\n{extra}Content-Length: {}\r\n\r\n",
            body.len()
        );
        tls.write_all(head.as_bytes())?;
        tls.write_all(body)?;
        tls.flush()?;
        let mut raw = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        loop {
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    raw.extend_from_slice(&buf[..n]);
                    if raw.len() > RESP_CAP {
                        bail!("response too large");
                    }
                }
                // Servers that close without close_notify: what arrived stands
                // only if the framing below says it is complete.
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
        }
        parse_response(&raw)
    }
}

impl Transport for Https {
    fn get(&self, url: &str) -> anyhow::Result<(u16, Vec<u8>)> {
        self.exchange(url, "GET", "", b"")
    }

    fn post_form(&self, url: &str, form: &str, basic: &str) -> anyhow::Result<(u16, Vec<u8>)> {
        let extra = format!(
            "Authorization: Basic {basic}\r\nContent-Type: application/x-www-form-urlencoded\r\n"
        );
        let extra = Zeroizing::new(extra);
        self.exchange(url, "POST", &extra, form.as_bytes())
    }
}

/// Status and body of an HTTP/1.1 response: Content-Length or chunked
/// framing, and a body that ends before its framing says is refused.
pub(crate) fn parse_response(raw: &[u8]) -> anyhow::Result<(u16, Vec<u8>)> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow!("no header end"))?;
    let head = std::str::from_utf8(&raw[..split]).context("header utf8")?;
    let body = &raw[split + 4..];
    let mut lines = head.split("\r\n");
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("bad status line"))?;
    let mut length = None;
    let mut chunked = false;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if k.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                bail!("duplicate content-length");
            }
            length = Some(v.parse::<usize>().context("content-length")?);
        } else if k.eq_ignore_ascii_case("transfer-encoding") {
            chunked = v.eq_ignore_ascii_case("chunked");
            if !chunked {
                bail!("unsupported transfer-encoding {v}");
            }
        }
    }
    if chunked && length.is_some() {
        bail!("ambiguous response framing");
    }
    if chunked {
        return Ok((status, dechunk(body)?));
    }
    match length {
        Some(n) if body.len() >= n => Ok((status, body[..n].to_vec())),
        Some(_) => bail!("truncated body"),
        None => Ok((status, body.to_vec())),
    }
}

fn dechunk(mut b: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let eol = b
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| anyhow!("truncated chunk"))?;
        let size_txt = std::str::from_utf8(&b[..eol])?;
        let size_txt = size_txt.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_txt, 16).context("chunk size")?;
        b = &b[eol + 2..];
        if size == 0 {
            if b != b"\r\n" {
                bail!("invalid final chunk");
            }
            return Ok(out);
        }
        if size > RESP_CAP || b.len() < size.saturating_add(2) || &b[size..size + 2] != b"\r\n" {
            bail!("truncated chunk");
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_length_and_chunked_and_refuses_truncation() {
        let r = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(parse_response(r).unwrap(), (200, b"{}".to_vec()));
        let r = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{\"\r\n3;x=y\r\na\":\r\n2\r\n1}\r\n0\r\n\r\n";
        assert_eq!(parse_response(r).unwrap().1, b"{\"a\":1}".to_vec());
        let r = b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n{}";
        assert!(parse_response(r).is_err());
        let r = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nab";
        assert!(parse_response(r).is_err());
        let r = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n";
        assert!(parse_response(r).is_err());
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;

    struct Provider {
        signer: jwt::Es256Signer,
        claims: Mutex<Value>,
        exchange: Mutex<Option<(String, String)>>,
    }
    impl Transport for Provider {
        fn get(&self, url: &str) -> anyhow::Result<(u16, Vec<u8>)> {
            let value = if url.ends_with("openid-configuration") {
                json!({"issuer":"https://identity.test", "authorization_endpoint":"https://identity.test/authorize",
                    "token_endpoint":"https://identity.test/token", "jwks_uri":"https://identity.test/keys"})
            } else {
                json!({"keys":[self.signer.jwk()]})
            };
            Ok((200, serde_json::to_vec(&value)?))
        }
        fn post_form(&self, _: &str, form: &str, basic: &str) -> anyhow::Result<(u16, Vec<u8>)> {
            *lock(&self.exchange) = Some((form.to_owned(), basic.to_owned()));
            let token = self.signer.sign(&lock(&self.claims))?;
            Ok((200, serde_json::to_vec(&json!({"id_token":token}))?))
        }
    }
    fn fixture() -> (Upstream, Arc<Provider>) {
        let provider = Arc::new(Provider {
            signer: jwt::Es256Signer::from_pkcs8(&jwt::Es256Signer::generate_pkcs8().unwrap())
                .unwrap(),
            claims: Mutex::new(json!({})),
            exchange: Mutex::new(None),
        });
        let cfg = UpstreamConfig {
            issuer: "https://identity.test".into(),
            client_id: "secd-test".into(),
            client_secret: Zeroizing::new("fixture-only".into()),
            ca_pem: vec![],
            name: "Company".into(),
            required: true,
        };
        (
            Upstream::with_transport(cfg, "https://secd.test", provider.clone()).unwrap(),
            provider,
        )
    }
    fn begin(up: &Upstream, provider: &Provider) -> (HashMap<String, String>, String) {
        let (url, binding) = up.begin("/device?code=ABCD-EFGH&eph=fixture").unwrap();
        let params = url::Url::parse(&url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect::<HashMap<_, _>>();
        let now = crate::oidc::unix_now();
        *lock(&provider.claims) = json!({"iss":"https://identity.test", "sub":"stable-subject",
            "aud":"secd-test", "iat":now, "exp":now+300, "nonce":params["nonce"],
            "email":"human@example.test", "email_verified":true});
        (params, binding)
    }
    #[test]
    fn code_flow_binds_browser_pkce_nonce_account_and_return_path() {
        let (up, provider) = fixture();
        let (p, binding) = begin(&up, &provider);
        assert_eq!(p["response_type"], "code");
        assert_eq!(p["code_challenge_method"], "S256");
        assert_eq!(
            p["redirect_uri"],
            "https://secd.test/api/auth/oidc/callback"
        );
        let result = up
            .finish(&p["state"], "one-use-code", Some(&binding), |email| {
                email == "human@example.test"
            })
            .unwrap();
        assert_eq!(
            result,
            (
                "human@example.test".into(),
                "/device?code=ABCD-EFGH&eph=fixture".into()
            )
        );
        let g = lock(&provider.exchange);
        let (form, basic) = g.as_ref().unwrap();
        let form: HashMap<_, _> = url::form_urlencoded::parse(form.as_bytes())
            .into_owned()
            .collect();
        assert_eq!(
            jwt::pkce_challenge(&form["code_verifier"]),
            p["code_challenge"]
        );
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(basic, &jwt::b64std(b"secd-test:fixture-only"));
        assert!(up
            .finish(&p["state"], "one-use-code", Some(&binding), |_| true)
            .is_err());
    }
    #[test]
    fn rejects_wrong_browser_expired_state_and_invalid_identity() {
        let (up, provider) = fixture();
        let (p, _) = begin(&up, &provider);
        assert!(up
            .finish(&p["state"], "code", Some("other-browser"), |_| true)
            .is_err());
        assert!(lock(&provider.exchange).is_none());
        let (p, binding) = begin(&up, &provider);
        lock(&up.flows).get_mut(&p["state"]).unwrap().created = Instant::now() - FLOW_TTL;
        assert!(up
            .finish(&p["state"], "code", Some(&binding), |_| true)
            .is_err());
        for (key, value) in [
            ("nonce", json!("wrong")),
            ("iss", json!("https://evil.test")),
            ("aud", json!("another-client")),
            ("email_verified", json!(false)),
            ("email_verified", json!("true")),
            ("exp", json!(1)),
        ] {
            let (p, binding) = begin(&up, &provider);
            lock(&provider.claims)[key] = value;
            assert!(
                up.finish(&p["state"], "code", Some(&binding), |_| true)
                    .is_err(),
                "accepted {key}"
            );
        }
        let (p, binding) = begin(&up, &provider);
        assert!(up
            .finish(&p["state"], "code", Some(&binding), |_| false)
            .is_err());
    }
    #[test]
    fn required_identity_expires_and_logout_forgets_it() {
        let (up, _) = fixture();
        let mut headers = HeaderMap::new();
        assert!(!up.admits(&headers, "human@example.test"));
        let cookie = up.remember("human@example.test".into());
        headers.insert(
            header::COOKIE,
            format!("{IDP_COOKIE}={cookie}").parse().unwrap(),
        );
        assert!(up.admits(&headers, "human@example.test"));
        assert!(!up.admits(&headers, "other@example.test"));
        lock(&up.verified)
            .get_mut(&crate::audit::sha256(cookie.as_bytes()))
            .unwrap()
            .at = Instant::now() - VERIFIED_TTL;
        assert!(!up.admits(&headers, "human@example.test"));
        up.forget(&headers);
        assert!(lock(&up.verified).is_empty());
    }
    #[test]
    fn rejects_redirect_escape_and_insecure_endpoints() {
        for target in [
            "https://evil.test/",
            "//evil.test/",
            "/api/auth/logout",
            "/device#fragment",
        ] {
            assert!(checked_return(target).is_err(), "accepted {target}");
        }
        for target in [
            "http://identity.test",
            "https://user:pw@identity.test",
            "https://identity.test/#fragment",
        ] {
            assert!(https_only(target).is_err());
        }
    }
}
