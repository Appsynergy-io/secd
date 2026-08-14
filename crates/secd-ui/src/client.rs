//! Browser I/O. Rust calling web-sys. Not application JavaScript.

use js_sys::{Array, Object, Promise, Reflect, Uint8Array};
use serde_json::{json, Value};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCredentials, RequestInit, RequestMode, Response, Window};
use zeroize::Zeroizing;

use crate::api;
use crate::tokens::{FAIL_SENTENCE, LAST_KEY, PRF_SALT};

pub struct Http {
    pub status: u16,
    pub data: Value,
}

fn window() -> Window {
    web_sys::window().expect("invariant: window")
}

pub async fn req(method: &str, url: &str, body: Option<&Value>) -> Result<Http, String> {
    let opts = RequestInit::new();
    opts.set_method(method);
    opts.set_mode(RequestMode::SameOrigin);
    opts.set_credentials(RequestCredentials::SameOrigin);
    if let Some(v) = body {
        let headers = web_sys::Headers::new().map_err(|_| FAIL_SENTENCE.to_string())?;
        headers
            .set("Content-Type", "application/json")
            .map_err(|_| FAIL_SENTENCE.to_string())?;
        opts.set_headers(&headers);
        opts.set_body(&JsValue::from_str(&v.to_string()));
    }
    let request =
        Request::new_with_str_and_init(url, &opts).map_err(|_| FAIL_SENTENCE.to_string())?;
    let resp_val = JsFuture::from(window().fetch_with_request(&request))
        .await
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    let resp: Response = resp_val.dyn_into().map_err(|_| FAIL_SENTENCE.to_string())?;
    let status = resp.status();
    let text = match resp.text() {
        Ok(p) => JsFuture::from(p)
            .await
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    };
    let data = serde_json::from_str(&text).unwrap_or(json!({}));
    Ok(Http { status, data })
}

pub fn load_remember() -> Option<crate::Remembered> {
    let store = window().local_storage().ok().flatten()?;
    let raw = store.get_item(LAST_KEY).ok().flatten()?;
    crate::parse_remembered(&raw)
}

pub fn save_remember(email: &str, has_passkey: bool) {
    let Some(store) = window().local_storage().ok().flatten() else {
        return;
    };
    let rec = crate::Remembered {
        email: email.to_string(),
        has_passkey,
        at: crate::remember::now_rfc3339(),
    };
    let _ = store.set_item(LAST_KEY, &crate::remember::encode_remembered(&rec));
}

pub fn clear_remember() {
    if let Some(store) = window().local_storage().ok().flatten() {
        let _ = store.remove_item(LAST_KEY);
    }
}

pub fn query_user_code() -> (String, String) {
    let search = window().location().search().unwrap_or_default();
    crate::api::device_query(&search)
}

fn b64url_to_u8(s: &str) -> Vec<u8> {
    let mut t = s.replace('-', "+").replace('_', "/");
    while t.len() % 4 != 0 {
        t.push('=');
    }
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = t.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < bytes.len() {
        let (a, b, c, d) = (
            val(bytes[i]),
            val(bytes[i + 1]),
            val(bytes[i + 2]),
            val(bytes[i + 3]),
        );
        if let (Some(a), Some(b)) = (a, b) {
            out.push((a << 2) | (b >> 4));
            if let Some(c) = c {
                out.push((b << 4) | (c >> 2));
                if let Some(d) = d {
                    out.push((c << 6) | d);
                }
            }
        }
        i += 4;
    }
    out
}

fn u8_to_b64url(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = bytes.get(i + 1).copied().unwrap_or(0);
        let b2 = bytes.get(i + 2).copied().unwrap_or(0);
        s.push(T[(b0 >> 2) as usize] as char);
        s.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < bytes.len() {
            s.push(T[(((b1 & 15) << 2) | (b2 >> 6)) as usize] as char);
        }
        if i + 2 < bytes.len() {
            s.push(T[(b2 & 63) as usize] as char);
        }
        i += 3;
    }
    s.replace('+', "-").replace('/', "_")
}

fn set_buf(obj: &Object, key: &str, s: &str) {
    let buf = Uint8Array::from(b64url_to_u8(s).as_slice());
    let _ = Reflect::set(obj, &JsValue::from_str(key), &buf.buffer());
}

fn coerce_pk(data: &Value) -> Result<JsValue, String> {
    let pk = data.get("publicKey").unwrap_or(data);
    let obj = js_sys::JSON::parse(&pk.to_string()).map_err(|_| FAIL_SENTENCE.to_string())?;
    let obj = obj
        .dyn_into::<Object>()
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    if let Some(ch) = pk.get("challenge").and_then(Value::as_str) {
        set_buf(&obj, "challenge", ch);
    }
    if let Some(user) = Reflect::get(&obj, &JsValue::from_str("user"))
        .ok()
        .and_then(|v| v.dyn_into::<Object>().ok())
    {
        if let Some(id) = pk
            .get("user")
            .and_then(|u| u.get("id"))
            .and_then(Value::as_str)
        {
            set_buf(&user, "id", id);
        }
    }
    for list_key in ["excludeCredentials", "allowCredentials"] {
        if let Ok(list) = Reflect::get(&obj, &JsValue::from_str(list_key)) {
            if let Ok(arr) = list.dyn_into::<Array>() {
                for i in 0..arr.length() {
                    if let Ok(cred) = arr.get(i).dyn_into::<Object>() {
                        if let Some(id) = pk
                            .get(list_key)
                            .and_then(Value::as_array)
                            .and_then(|a| a.get(i as usize))
                            .and_then(|c| c.get("id"))
                            .and_then(Value::as_str)
                        {
                            set_buf(&cred, "id", id);
                        }
                    }
                }
            }
        }
    }
    let ext = Object::new();
    if let Ok(existing) = Reflect::get(&obj, &JsValue::from_str("extensions")) {
        if let Ok(e) = existing.dyn_into::<Object>() {
            for key in js_sys::Object::keys(&e).iter() {
                let _ = Reflect::set(&ext, &key, &Reflect::get(&e, &key).unwrap_or(JsValue::NULL));
            }
        }
    }
    let prf = Object::new();
    let eval = Object::new();
    let salt = Uint8Array::from(PRF_SALT.as_slice());
    let _ = Reflect::set(&eval, &JsValue::from_str("first"), &salt);
    let _ = Reflect::set(&prf, &JsValue::from_str("eval"), &eval);
    let _ = Reflect::set(&ext, &JsValue::from_str("prf"), &prf);
    let _ = Reflect::set(&obj, &JsValue::from_str("extensions"), &ext);
    Ok(obj.into())
}

fn serialize_cred(cred: &JsValue) -> Result<Value, String> {
    let raw_id =
        Reflect::get(cred, &JsValue::from_str("rawId")).map_err(|_| FAIL_SENTENCE.to_string())?;
    let raw = Uint8Array::new(&raw_id);
    let mut raw_bytes = vec![0u8; raw.length() as usize];
    raw.copy_to(&mut raw_bytes);
    let resp = Reflect::get(cred, &JsValue::from_str("response"))
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    let id = Reflect::get(cred, &JsValue::from_str("id"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    let mut response = serde_json::Map::new();
    for (name, getter) in [
        ("attestationObject", "attestationObject"),
        ("clientDataJSON", "clientDataJSON"),
        ("authenticatorData", "authenticatorData"),
        ("signature", "signature"),
        ("userHandle", "userHandle"),
    ] {
        if let Ok(v) = Reflect::get(&resp, &JsValue::from_str(getter)) {
            if !v.is_undefined() && !v.is_null() {
                let arr = Uint8Array::new(&v);
                let mut bytes = vec![0u8; arr.length() as usize];
                arr.copy_to(&mut bytes);
                response.insert(name.to_string(), json!(u8_to_b64url(&bytes)));
            }
        }
    }
    Ok(json!({
        "id": id,
        "rawId": u8_to_b64url(&raw_bytes),
        "type": "public-key",
        "response": Value::Object(response),
    }))
}

/// The PRF secret is the passkey KEK; it never leaves this process.
fn prf_bytes(cred: &JsValue) -> Option<Zeroizing<Vec<u8>>> {
    let f = Reflect::get(cred, &JsValue::from_str("getClientExtensionResults")).ok()?;
    let f: js_sys::Function = f.dyn_into().ok()?;
    let ext = f.call0(cred).ok()?;
    let prf = Reflect::get(&ext, &JsValue::from_str("prf")).ok()?;
    let results = Reflect::get(&prf, &JsValue::from_str("results")).ok()?;
    let first = Reflect::get(&results, &JsValue::from_str("first")).ok()?;
    let arr = Uint8Array::new(&first);
    if arr.length() < 32 {
        return None;
    }
    let mut bytes = vec![0u8; arr.length() as usize];
    arr.copy_to(&mut bytes);
    let out = Zeroizing::new(bytes[..32].to_vec());
    crate::crypto::zeroize_bytes(&mut bytes);
    Some(out)
}

fn cred_id_hex(cred: &JsValue) -> String {
    let Ok(raw_id) = Reflect::get(cred, &JsValue::from_str("rawId")) else {
        return String::new();
    };
    let raw = Uint8Array::new(&raw_id);
    let mut bytes = vec![0u8; raw.length() as usize];
    raw.copy_to(&mut bytes);
    crate::crypto::to_hex(&bytes)
}

async fn cred_create(pk: &JsValue) -> Result<web_sys::PublicKeyCredential, String> {
    let nav = window().navigator();
    let creds = nav.credentials();
    let opts = web_sys::CredentialCreationOptions::new();
    let _ = Reflect::set(&opts, &JsValue::from_str("publicKey"), pk);
    let p = creds
        .create_with_options(&opts)
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    let got = JsFuture::from(p)
        .await
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    got.dyn_into().map_err(|_| FAIL_SENTENCE.to_string())
}

async fn cred_get(pk: &JsValue, conditional: bool) -> Result<web_sys::PublicKeyCredential, String> {
    let nav = window().navigator();
    let creds = nav.credentials();
    let opts = web_sys::CredentialRequestOptions::new();
    let _ = Reflect::set(&opts, &JsValue::from_str("publicKey"), pk);
    if conditional {
        let _ = Reflect::set(
            &opts,
            &JsValue::from_str("mediation"),
            &JsValue::from_str("conditional"),
        );
    }
    let p = creds
        .get_with_options(&opts)
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    let got = JsFuture::from(p)
        .await
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    got.dyn_into().map_err(|_| FAIL_SENTENCE.to_string())
}

/// Registers a passkey and wraps the DEK to its PRF secret client-side.
/// `dek` is the in-memory vault key when adding a factor; a fresh mint on
/// first registration. Returns the DEK on success so the caller holds it.
pub async fn passkey_create(
    email: &str,
    dek: Option<&[u8]>,
) -> Result<(Http, Option<Zeroizing<Vec<u8>>>), String> {
    let start = req(
        "POST",
        api::passkey_register_start_url(),
        Some(&json!({ "email": email })),
    )
    .await?;
    if start.status != 200 {
        return Ok((start, None));
    }
    let pk = coerce_pk(&start.data)?;
    let cred: JsValue = cred_create(&pk).await?.into();
    let Some(prf) = prf_bytes(&cred) else {
        return Ok((
            Http {
                status: 400,
                data: json!({ "error": "prf" }),
            },
            None,
        ));
    };
    let dek_bytes: Zeroizing<Vec<u8>> = match dek {
        Some(d) => Zeroizing::new(d.to_vec()),
        None => {
            let mut fresh = crate::crypto::mint_dek();
            let z = Zeroizing::new(fresh.to_vec());
            crate::crypto::zeroize_bytes(&mut fresh);
            z
        }
    };
    let wrap = crate::crypto::wrap_passkey(&dek_bytes, &prf, &cred_id_hex(&cred))
        .map_err(|_| FAIL_SENTENCE.to_string())?;
    let handle = start
        .data
        .get("handle")
        .and_then(Value::as_str)
        .unwrap_or("");
    let res = req(
        "POST",
        api::passkey_register_finish_url(),
        Some(&json!({
            "handle": handle,
            "credential": serialize_cred(&cred)?,
            "wrap": crate::crypto::wrap_to_json(&wrap),
            "email": email,
        })),
    )
    .await?;
    let dek_out = (res.status == 200).then_some(dek_bytes);
    Ok((res, dek_out))
}

/// Signs in with a passkey and unwraps the DEK from the returned wraps.
/// A stale account whose wraps predate client-side minting yields no DEK.
pub async fn passkey_get(
    email: Option<&str>,
    conditional: bool,
) -> Result<(Http, Option<Zeroizing<Vec<u8>>>), String> {
    let body = match email {
        Some(e) if !e.is_empty() => json!({ "email": e }),
        _ => json!({}),
    };
    let start = req("POST", api::passkey_login_start_url(), Some(&body)).await?;
    if start.status != 200 {
        return Ok((start, None));
    }
    let pk = coerce_pk(&start.data)?;
    let cred: JsValue = cred_get(&pk, conditional).await?.into();
    let Some(prf) = prf_bytes(&cred) else {
        return Ok((
            Http {
                status: 400,
                data: json!({ "error": "prf" }),
            },
            None,
        ));
    };
    let handle = start
        .data
        .get("handle")
        .and_then(Value::as_str)
        .unwrap_or("");
    let res = req(
        "POST",
        api::passkey_login_finish_url(),
        Some(&json!({
            "handle": handle,
            "credential": serialize_cred(&cred)?,
        })),
    )
    .await?;
    let dek = if res.status == 200 {
        crate::crypto::unwrap_any(&crate::crypto::wraps_from_json(&res.data), None, Some(&prf))
            .map(Zeroizing::new)
    } else {
        None
    };
    Ok((res, dek))
}

pub async fn copy_text(text: &str) {
    let c = window().navigator().clipboard();
    let _ = JsFuture::from(c.write_text(text)).await;
}

pub fn width_px() -> u32 {
    window()
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0) as u32
}

pub fn path() -> String {
    window()
        .location()
        .pathname()
        .unwrap_or_else(|_| "/".into())
}

pub fn push_path(to: &str) {
    if let Ok(hist) = window().history() {
        let _ = hist.push_state_with_url(&JsValue::NULL, "", Some(to));
    }
}

/// Runs `f` on the next macrotask. Handlers that unmount their own subtree
/// must defer the state write past the current event dispatch.
pub fn after_delay_ms(ms: i32, f: impl FnOnce() + 'static) {
    let cb = wasm_bindgen::closure::Closure::once_into_js(f);
    let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms);
}

#[allow(dead_code)]
fn _keep_promise(_: Promise) {}
