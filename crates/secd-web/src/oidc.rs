//! secd as an OpenID Provider for its own CLI.
//!
//! The CLI is a public client (`secd-cli`) using the device authorization
//! grant (RFC 8628). The token response carries an ID token (OIDC Core §2)
//! signed RS256 with a key that never leaves the server's data directory, and
//! the provider publishes discovery (OIDC Discovery §3) and its JWK Set so a
//! client verifies the token against keys it fetched itself.
//!
//! Native clients may also use authorization code with mandatory S256 PKCE.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::{Padding, Rsa},
    sign::Signer,
};
use secd_core::jwt::b64url;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::headers::json_value;
use crate::state::AppState;

/// The CLI's client identifier. A public client: it holds no secret, and the
/// device code is what binds the token request to the authorization.
pub const CLI_CLIENT_ID: &str = "secd-cli";
/// Lifetime of an ID token. It proves who approved the sign-in at the moment
/// of the exchange; the session it accompanies has its own lifetime.
pub const ID_TOKEN_TTL: i64 = 5 * 60;
const KEY_FILE: &str = "oidc-rs256.pk8";

pub struct Identity<'a> {
    pub subject: &'a str,
    pub email: &'a str,
    pub session_id: &'a str,
    pub auth_time: i64,
}

#[derive(Clone)]
pub struct Issuer {
    pub issuer: String,
    signer: Arc<PKey<Private>>,
}

impl Issuer {
    /// Load the signing key from `dir`, creating it on first start. The file
    /// is 0600 and created exclusively, so two starts never race to two keys.
    pub fn open(dir: &Path, issuer: &str) -> anyhow::Result<Self> {
        let path = dir.join(KEY_FILE);
        let der = match std::fs::read(&path) {
            Ok(der) => Zeroizing::new(der),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let der =
                    Zeroizing::new(PKey::from_rsa(Rsa::generate(2048)?)?.private_key_to_pkcs8()?);
                let tmp = dir.join(format!("{KEY_FILE}.tmp-{}", std::process::id()));
                {
                    let mut f = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(&tmp)
                        .with_context(|| format!("create {}", tmp.display()))?;
                    f.write_all(&der)?;
                    f.sync_all()?;
                }
                // hard_link fails if another start won: then use its key.
                let won = std::fs::hard_link(&tmp, &path).is_ok();
                let _ = std::fs::remove_file(&tmp);
                if won {
                    der
                } else {
                    Zeroizing::new(
                        std::fs::read(&path).with_context(|| format!("read {}", path.display()))?,
                    )
                }
            }
            Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
        };
        let signer = PKey::private_key_from_pkcs8(&der)?;
        if signer.rsa()?.size() < 256 {
            return Err(anyhow!("OIDC signing key is too small"));
        }
        Ok(Self {
            issuer: issuer.to_string(),
            signer: Arc::new(signer),
        })
    }

    fn jwk(&self) -> Value {
        let rsa = self.signer.rsa().expect("validated RSA key");
        let mut key =
            json!({"e": b64url(&rsa.e().to_vec()), "kty": "RSA", "n": b64url(&rsa.n().to_vec())});
        let kid = b64url(&crate::audit::sha256(key.to_string().as_bytes()));
        key["kid"] = json!(kid);
        key["alg"] = json!("RS256");
        key["use"] = json!("sig");
        key
    }

    pub fn jwks(&self) -> Value {
        json!({ "keys": [self.jwk()] })
    }

    pub fn discovery(&self) -> Value {
        let iss = &self.issuer;
        json!({
            "issuer": iss,
            "jwks_uri": format!("{iss}/.well-known/jwks.json"),
            "device_authorization_endpoint": format!("{iss}/api/v1/device/start"),
            "token_endpoint": format!("{iss}/api/v1/device/poll"),
            "authorization_endpoint": format!("{iss}/api/oidc/authorize"),
            "response_types_supported": ["code"],
            "response_modes_supported": ["query"],
            "code_challenge_methods_supported": ["S256"],
            "grant_types_supported": ["urn:ietf:params:oauth:grant-type:device_code", "authorization_code"],
            "token_endpoint_auth_methods_supported": ["none"],
            "scopes_supported": ["openid", "email"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "claims_supported": ["iss", "sub", "aud", "exp", "iat", "auth_time", "email", "sid"],
        })
    }

    /// An ID token for `client_id` about the account `sub`, authenticated at
    /// `auth_time`, for the session `sid`.
    pub fn id_token(
        &self,
        client_id: &str,
        identity: Identity<'_>,
        nonce: Option<&str>,
        access_token: &str,
        include_email: bool,
    ) -> anyhow::Result<String> {
        let now = unix_now();
        let mut jti = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut jti);
        let mut claims = json!({
            "iss": self.issuer,
            "sub": identity.subject,
            "aud": client_id,
            "exp": now + ID_TOKEN_TTL,
            "iat": now,
            "auth_time": identity.auth_time,
            "at_hash": secd_core::jwt::token_hash(access_token),
            "sid": identity.session_id,
            "jti": b64url(&jti),
        });
        if let Some(nonce) = nonce {
            claims["nonce"] = json!(nonce);
        }
        if include_email {
            claims["email"] = json!(identity.email);
        }
        let header = json!({"alg": "RS256", "typ": "JWT", "kid": self.jwk()["kid"]});
        let input = format!(
            "{}.{}",
            b64url(header.to_string().as_bytes()),
            b64url(claims.to_string().as_bytes())
        );
        let mut signer = Signer::new(MessageDigest::sha256(), &self.signer)?;
        signer.set_rsa_padding(Padding::PKCS1)?;
        signer.update(input.as_bytes())?;
        Ok(format!("{input}.{}", b64url(&signer.sign_to_vec()?)))
    }
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/.well-known/jwks.json", get(jwks))
}

async fn discovery(State(state): State<AppState>) -> Response {
    json_value(StatusCode::OK, state.oidc.discovery())
}

async fn jwks(State(state): State<AppState>) -> Response {
    json_value(StatusCode::OK, state.oidc.jwks())
}
