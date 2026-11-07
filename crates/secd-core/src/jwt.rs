//! JSON Web Token verification for OpenID Connect ID tokens (RFC 7519,
//! RFC 7515 compact serialization, OIDC Core §3.1.3.7).
//!
//! One verifier for both directions: the CLI checks the ID token secd issues,
//! and secd checks the ID token an upstream provider issues. Only asymmetric
//! algorithms are accepted (ES256, RS256), the key is chosen by `kid` and must
//! match the algorithm's key type, and every time claim is checked. `alg:
//! none`, HMAC and anything else fail closed.

use aws_lc_rs::signature::{
    EcdsaKeyPair, KeyPair, RsaPublicKeyComponents, UnparsedPublicKey, ECDSA_P256_SHA256_FIXED,
    ECDSA_P256_SHA256_FIXED_SIGNING, RSA_PKCS1_2048_8192_SHA256,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Map, Value};

/// Clock skew tolerated on `exp`, `iat` and `nbf`.
pub const LEEWAY_SECS: i64 = 60;
/// Largest token accepted, before any decoding.
const MAX_TOKEN: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JwtError {
    Malformed,
    Algorithm,
    UnknownKey,
    Signature,
    Issuer,
    Audience,
    Expired,
    NotYetValid,
    Nonce,
    Claim(&'static str),
    Key,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => f.write_str("malformed token"),
            Self::Algorithm => f.write_str("unsupported algorithm"),
            Self::UnknownKey => f.write_str("no published key matches the token"),
            Self::Signature => f.write_str("bad signature"),
            Self::Issuer => f.write_str("wrong issuer"),
            Self::Audience => f.write_str("wrong audience"),
            Self::Expired => f.write_str("token expired"),
            Self::NotYetValid => f.write_str("token not yet valid"),
            Self::Nonce => f.write_str("nonce mismatch"),
            Self::Claim(c) => write!(f, "missing or invalid claim {c}"),
            Self::Key => f.write_str("bad key"),
        }
    }
}

impl std::error::Error for JwtError {}

/// What a valid token must say about itself.
pub struct Expect<'a> {
    pub issuer: &'a str,
    pub audience: &'a str,
    /// Required when the request carried one (authorization code flow).
    pub nonce: Option<&'a str>,
    /// Seconds since the Unix epoch.
    pub now: i64,
}

pub fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn b64std(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub fn b64url_decode(s: &str) -> Result<Vec<u8>, JwtError> {
    URL_SAFE_NO_PAD
        .decode(s.as_bytes())
        .map_err(|_| JwtError::Malformed)
}

fn segment_json(s: &str) -> Result<Map<String, Value>, JwtError> {
    match serde_json::from_slice::<Value>(&b64url_decode(s)?) {
        Ok(Value::Object(m)) => Ok(m),
        _ => Err(JwtError::Malformed),
    }
}

/// Verify `token` against the JWK Set `jwks` and the expectations. Returns
/// the claims of a token that passed every check.
pub fn verify(
    token: &str,
    jwks: &Value,
    expect: &Expect<'_>,
) -> Result<Map<String, Value>, JwtError> {
    if token.len() > MAX_TOKEN {
        return Err(JwtError::Malformed);
    }
    let mut parts = token.split('.');
    let (Some(h), Some(p), Some(sig), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(JwtError::Malformed);
    };
    let header = segment_json(h)?;
    if header.contains_key("crit") {
        return Err(JwtError::Algorithm);
    }
    let alg = header
        .get("alg")
        .and_then(Value::as_str)
        .ok_or(JwtError::Algorithm)?;
    let kty = match alg {
        "ES256" => "EC",
        "RS256" => "RSA",
        _ => return Err(JwtError::Algorithm),
    };
    let kid = header.get("kid").and_then(Value::as_str);
    let key = select_key(jwks, kty, alg, kid)?;
    let signing_input = &token[..h.len() + 1 + p.len()];
    let sig = b64url_decode(sig)?;
    verify_signature(alg, key, signing_input.as_bytes(), &sig)?;
    let claims = segment_json(p)?;
    check_claims(&claims, expect)?;
    Ok(claims)
}

fn select_key<'a>(
    jwks: &'a Value,
    kty: &str,
    alg: &str,
    kid: Option<&str>,
) -> Result<&'a Map<String, Value>, JwtError> {
    let keys = jwks
        .get("keys")
        .and_then(Value::as_array)
        .ok_or(JwtError::UnknownKey)?;
    let mut fits = keys.iter().filter_map(Value::as_object).filter(|k| {
        k.get("kty").and_then(Value::as_str) == Some(kty)
            && k.get("use")
                .and_then(Value::as_str)
                .is_none_or(|u| u == "sig")
            && k.get("alg")
                .and_then(Value::as_str)
                .is_none_or(|a| a == alg)
            && k.get("key_ops").is_none_or(|ops| {
                ops.as_array()
                    .is_some_and(|ops| ops.iter().any(|op| op.as_str() == Some("verify")))
            })
            && kid.is_none_or(|kid| k.get("kid").and_then(Value::as_str) == Some(kid))
    });
    let key = fits.next().ok_or(JwtError::UnknownKey)?;
    // Without a kid the choice must be unambiguous.
    if fits.next().is_some() {
        return Err(JwtError::UnknownKey);
    }
    Ok(key)
}

fn field(k: &Map<String, Value>, name: &str) -> Result<Vec<u8>, JwtError> {
    b64url_decode(k.get(name).and_then(Value::as_str).ok_or(JwtError::Key)?)
}

fn verify_signature(
    alg: &str,
    key: &Map<String, Value>,
    msg: &[u8],
    sig: &[u8],
) -> Result<(), JwtError> {
    match alg {
        "ES256" => {
            if key.get("crv").and_then(Value::as_str) != Some("P-256") {
                return Err(JwtError::Key);
            }
            let (x, y) = (field(key, "x")?, field(key, "y")?);
            if x.len() != 32 || y.len() != 32 || sig.len() != 64 {
                return Err(JwtError::Signature);
            }
            let mut point = Vec::with_capacity(65);
            point.push(0x04);
            point.extend_from_slice(&x);
            point.extend_from_slice(&y);
            UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, point)
                .verify(msg, sig)
                .map_err(|_| JwtError::Signature)
        }
        "RS256" => {
            let (n, e) = (field(key, "n")?, field(key, "e")?);
            RsaPublicKeyComponents { n: &n, e: &e }
                .verify(&RSA_PKCS1_2048_8192_SHA256, msg, sig)
                .map_err(|_| JwtError::Signature)
        }
        _ => Err(JwtError::Algorithm),
    }
}

fn num(claims: &Map<String, Value>, name: &'static str) -> Result<Option<i64>, JwtError> {
    match claims.get(name) {
        None => Ok(None),
        Some(v) => v
            .as_i64()
            .or_else(|| v.as_f64().filter(|f| f.is_finite()).map(|f| f as i64))
            .map(Some)
            .ok_or(JwtError::Claim(name)),
    }
}

fn check_claims(claims: &Map<String, Value>, expect: &Expect<'_>) -> Result<(), JwtError> {
    if claims.get("iss").and_then(Value::as_str) != Some(expect.issuer) {
        return Err(JwtError::Issuer);
    }
    let sub = claims.get("sub").and_then(Value::as_str).unwrap_or("");
    if sub.is_empty() {
        return Err(JwtError::Claim("sub"));
    }
    match claims.get("aud") {
        Some(Value::String(a)) if a == expect.audience => {}
        Some(Value::Array(list))
            if list.iter().all(Value::is_string)
                && list.iter().any(|a| a.as_str() == Some(expect.audience)) =>
        {
            // OIDC Core §3.1.3.7: several audiences need an authorized party,
            // and it must be this client.
            if list.len() > 1 && claims.get("azp").and_then(Value::as_str) != Some(expect.audience)
            {
                return Err(JwtError::Audience);
            }
        }
        _ => return Err(JwtError::Audience),
    }
    if let Some(azp) = claims.get("azp") {
        if azp.as_str() != Some(expect.audience) {
            return Err(JwtError::Audience);
        }
    }
    let exp = num(claims, "exp")?.ok_or(JwtError::Claim("exp"))?;
    if expect.now >= exp.saturating_add(LEEWAY_SECS) {
        return Err(JwtError::Expired);
    }
    let iat = num(claims, "iat")?.ok_or(JwtError::Claim("iat"))?;
    if iat > expect.now.saturating_add(LEEWAY_SECS) {
        return Err(JwtError::NotYetValid);
    }
    if let Some(nbf) = num(claims, "nbf")? {
        if nbf > expect.now.saturating_add(LEEWAY_SECS) {
            return Err(JwtError::NotYetValid);
        }
    }
    if let Some(want) = expect.nonce {
        if claims.get("nonce").and_then(Value::as_str) != Some(want) {
            return Err(JwtError::Nonce);
        }
    }
    Ok(())
}

/// An ES256 signing key: PKCS#8 in, compact JWS out.
pub struct Es256Signer {
    pair: EcdsaKeyPair,
    kid: String,
}

impl Es256Signer {
    /// A fresh P-256 key as PKCS#8 DER, for the caller to persist.
    pub fn generate_pkcs8() -> Result<Vec<u8>, JwtError> {
        let rng = aws_lc_rs::rand::SystemRandom::new();
        let doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .map_err(|_| JwtError::Key)?;
        Ok(doc.as_ref().to_vec())
    }

    pub fn from_pkcs8(der: &[u8]) -> Result<Self, JwtError> {
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der)
            .map_err(|_| JwtError::Key)?;
        let mut signer = Self {
            pair,
            kid: String::new(),
        };
        signer.kid = signer.thumbprint();
        Ok(signer)
    }

    fn xy(&self) -> (&[u8], &[u8]) {
        let point = self.pair.public_key().as_ref();
        (&point[1..33], &point[33..65])
    }

    /// RFC 7638 JWK thumbprint: the key's stable `kid`.
    fn thumbprint(&self) -> String {
        let (x, y) = self.xy();
        let canonical = format!(
            r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#,
            b64url(x),
            b64url(y)
        );
        let digest = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, canonical.as_bytes());
        b64url(digest.as_ref())
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The public half, as a JWK for a JWK Set.
    pub fn jwk(&self) -> Value {
        let (x, y) = self.xy();
        json!({
            "kty": "EC",
            "crv": "P-256",
            "x": b64url(x),
            "y": b64url(y),
            "use": "sig",
            "alg": "ES256",
            "kid": self.kid,
        })
    }

    pub fn sign(&self, claims: &Value) -> Result<String, JwtError> {
        let header = json!({ "alg": "ES256", "typ": "JWT", "kid": self.kid });
        let input = format!(
            "{}.{}",
            b64url(header.to_string().as_bytes()),
            b64url(claims.to_string().as_bytes())
        );
        let rng = aws_lc_rs::rand::SystemRandom::new();
        let sig = self
            .pair
            .sign(&rng, input.as_bytes())
            .map_err(|_| JwtError::Key)?;
        Ok(format!("{input}.{}", b64url(sig.as_ref())))
    }
}

/// OIDC at_hash for the supported SHA-256 signing algorithms.
pub fn token_hash(token: &str) -> String {
    let hash = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, token.as_bytes());
    b64url(&hash.as_ref()[..16])
}

/// RFC 7636 S256 code challenge for a PKCE verifier.
pub fn pkce_challenge(verifier: &str) -> String {
    b64url(aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, verifier.as_bytes()).as_ref())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    const ISS: &str = "https://secd.test";
    const AUD: &str = "secd-cli";
    const NOW: i64 = 1_800_000_000;

    fn signer() -> Es256Signer {
        Es256Signer::from_pkcs8(&Es256Signer::generate_pkcs8().unwrap()).unwrap()
    }

    fn claims() -> Value {
        json!({"iss": ISS, "sub": "u-1", "aud": AUD, "exp": NOW + 300, "iat": NOW})
    }

    fn expect() -> Expect<'static> {
        Expect {
            issuer: ISS,
            audience: AUD,
            nonce: None,
            now: NOW,
        }
    }

    fn jwks(s: &Es256Signer) -> Value {
        json!({ "keys": [s.jwk()] })
    }

    fn with(mut c: Value, k: &str, v: Value) -> Value {
        c[k] = v;
        c
    }

    #[test]
    fn T_JWT_ROUNDTRIP() {
        let s = signer();
        let t = s.sign(&claims()).unwrap();
        let got = verify(&t, &jwks(&s), &expect()).unwrap();
        assert_eq!(got["sub"], "u-1");
        assert_eq!(s.kid().len(), 43, "a SHA-256 thumbprint");
    }

    #[test]
    fn T_JWT_REJECTS() {
        let s = signer();
        let keys = jwks(&s);
        let cases: Vec<(Value, JwtError)> = vec![
            (
                with(claims(), "iss", json!("https://evil.test")),
                JwtError::Issuer,
            ),
            (with(claims(), "aud", json!("other")), JwtError::Audience),
            (
                with(claims(), "aud", json!([AUD, "other"])),
                JwtError::Audience,
            ),
            (with(claims(), "azp", json!("other")), JwtError::Audience),
            (
                with(claims(), "exp", json!(NOW - LEEWAY_SECS)),
                JwtError::Expired,
            ),
            (
                with(claims(), "iat", json!(NOW + LEEWAY_SECS + 1)),
                JwtError::NotYetValid,
            ),
            (
                with(claims(), "nbf", json!(NOW + LEEWAY_SECS + 1)),
                JwtError::NotYetValid,
            ),
            (with(claims(), "sub", json!("")), JwtError::Claim("sub")),
            (with(claims(), "exp", json!("soon")), JwtError::Claim("exp")),
        ];
        for (c, want) in cases {
            let t = s.sign(&c).unwrap();
            assert_eq!(verify(&t, &keys, &expect()), Err(want), "{c}");
        }
        let both = with(
            with(claims(), "aud", json!([AUD, "other"])),
            "azp",
            json!(AUD),
        );
        assert!(verify(&s.sign(&both).unwrap(), &keys, &expect()).is_ok());
    }

    #[test]
    fn T_JWT_NONCE() {
        let s = signer();
        let t = s.sign(&with(claims(), "nonce", json!("n-1"))).unwrap();
        let mut e = expect();
        e.nonce = Some("n-1");
        assert!(verify(&t, &jwks(&s), &e).is_ok());
        e.nonce = Some("n-2");
        assert_eq!(verify(&t, &jwks(&s), &e), Err(JwtError::Nonce));
        let bare = s.sign(&claims()).unwrap();
        assert_eq!(verify(&bare, &jwks(&s), &e), Err(JwtError::Nonce));
    }

    #[test]
    fn T_JWT_FORGERY() {
        let s = signer();
        let other = signer();
        let t = other.sign(&claims()).unwrap();
        // Another key's signature under this key's kid: the signature decides.
        let parts: Vec<&str> = t.split('.').collect();
        let hdr = b64url(json!({"alg":"ES256","kid":s.kid()}).to_string().as_bytes());
        let swapped = format!("{hdr}.{}.{}", parts[1], parts[2]);
        assert_eq!(
            verify(&swapped, &jwks(&s), &expect()),
            Err(JwtError::Signature)
        );
        assert_eq!(verify(&t, &jwks(&s), &expect()), Err(JwtError::UnknownKey));

        // A tampered payload under a genuine signature.
        let good = s.sign(&claims()).unwrap();
        let p: Vec<&str> = good.split('.').collect();
        let evil = b64url(with(claims(), "sub", json!("admin")).to_string().as_bytes());
        let tampered = format!("{}.{evil}.{}", p[0], p[2]);
        assert_eq!(
            verify(&tampered, &jwks(&s), &expect()),
            Err(JwtError::Signature)
        );

        // alg none, HMAC, and crit all fail closed.
        for alg in [
            json!({"alg":"none"}),
            json!({"alg":"HS256","kid":s.kid()}),
            json!({"alg":"ES256","kid":s.kid(),"crit":["exp"]}),
        ] {
            let h = b64url(alg.to_string().as_bytes());
            let t = format!("{h}.{}.{}", p[1], p[2]);
            assert_eq!(
                verify(&t, &jwks(&s), &expect()),
                Err(JwtError::Algorithm),
                "{alg}"
            );
        }
        for bad in ["", "a.b", "a.b.c.d", "!!.!!.!!"] {
            assert!(verify(bad, &jwks(&s), &expect()).is_err(), "{bad}");
        }
    }

    #[test]
    fn T_JWT_RS256() {
        use aws_lc_rs::rsa::KeySize;
        use aws_lc_rs::signature::{KeyPair as _, RsaKeyPair, RSA_PKCS1_SHA256};
        let pair = RsaKeyPair::generate(KeySize::Rsa2048).unwrap();
        // RSAPublicKey ::= SEQUENCE { modulus INTEGER, publicExponent INTEGER }
        fn der_take(b: &[u8]) -> (u8, &[u8], &[u8]) {
            let (tag, mut i) = (b[0], 2);
            let mut len = b[1] as usize;
            if len & 0x80 != 0 {
                let n = len & 0x7f;
                len = b[2..2 + n].iter().fold(0, |a, x| (a << 8) | *x as usize);
                i += n;
            }
            (tag, &b[i..i + len], &b[i + len..])
        }
        let (_, seq, _) = der_take(pair.public_key().as_ref());
        let (_, n, rest) = der_take(seq);
        let (_, e, _) = der_take(rest);
        let strip = |x: &[u8]| {
            x.iter()
                .skip_while(|b| **b == 0)
                .copied()
                .collect::<Vec<u8>>()
        };
        struct Pc {
            n: Vec<u8>,
            e: Vec<u8>,
        }
        let pc = Pc {
            n: strip(n),
            e: strip(e),
        };
        let jwks = json!({"keys": [
            {"kty": "RSA", "kid": "r1", "alg": "RS256", "use": "sig", "n": b64url(&pc.n), "e": b64url(&pc.e)},
            signer().jwk(),
        ]});
        let input = format!(
            "{}.{}",
            b64url(json!({"alg": "RS256", "kid": "r1"}).to_string().as_bytes()),
            b64url(claims().to_string().as_bytes())
        );
        let mut sig = vec![0; pair.public_modulus_len()];
        pair.sign(
            &RSA_PKCS1_SHA256,
            &aws_lc_rs::rand::SystemRandom::new(),
            input.as_bytes(),
            &mut sig,
        )
        .unwrap();
        let token = format!("{input}.{}", b64url(&sig));
        assert!(verify(&token, &jwks, &expect()).is_ok());
        // An RS256 header cannot be pointed at the EC key.
        let ec_kid = jwks["keys"][1]["kid"].as_str().unwrap().to_string();
        let p: Vec<&str> = token.split('.').collect();
        let h = b64url(
            json!({"alg": "RS256", "kid": ec_kid})
                .to_string()
                .as_bytes(),
        );
        assert_eq!(
            verify(&format!("{h}.{}.{}", p[1], p[2]), &jwks, &expect()),
            Err(JwtError::UnknownKey)
        );
    }

    #[test]
    fn T_PKCE_S256() {
        // RFC 7636 Appendix B.
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
