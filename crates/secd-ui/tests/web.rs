#![allow(non_snake_case)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use secd_ui::register::FieldView;
use secd_ui::{
    layout_mode, parse_remembered, primary_field_action, remember_is_fresh_unix,
    remove_passkey_enabled, render_account, render_gate, render_register, session_revoke_path,
    AccountView, AuthMethod, FieldAction, GateKind, GateQuery, LayoutMode, PasskeyRow,
    RegisterView, Remembered, SecretItem, SessionInfo, SessionRow, APP_JS, BREAKPOINT_PX,
    EMAIL_AUTOCOMPLETE, LAST_KEY,
};

const VIEW_MOBILE: u32 = 375;
const VIEW_DESKTOP: u32 = 1280;
const EMAIL: &str = "op@imabee.com";
const XSS_NAME: &str = "<script>alert(1)</script>";

static SEQ: AtomicU64 = AtomicU64::new(1);

fn resolve(q: GateQuery) -> secd_ui::GateView {
    secd_ui::resolve_gate(&q)
}

fn attr(html: &str, name: &str, value: &str) -> bool {
    html.contains(&format!("{name}=\"{value}\"")) || html.contains(&format!("{name}='{value}'"))
}

fn has_input_type(html: &str, ty: &str) -> bool {
    attr(html, "type", ty)
}

fn has_named(html: &str, name: &str) -> bool {
    attr(html, "name", name)
}

fn has_email_field(html: &str) -> bool {
    has_named(html, "email") || has_input_type(html, "email")
}

fn has_password_input(html: &str) -> bool {
    has_input_type(html, "password") || has_named(html, "password")
}

fn has_script_element(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    lower.contains("<script>") || lower.contains("<script ") || lower.contains("<script/")
}

fn count(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}

fn fresh_at() -> String {
    secd_ui::remember::now_rfc3339()
}

fn stale_at() -> String {
    let now = fresh_at();
    assert!(
        now.len() >= 5 && now.as_bytes()[4] == b'-',
        "rfc3339: {now}"
    );
    format!("1999-{}", &now[5..])
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("invariant: wall clock after epoch")
        .as_secs() as i64
}

fn passkey_only_view() -> secd_ui::GateView {
    resolve(GateQuery {
        method: Some(AuthMethod::Passkey),
        ..GateQuery::default()
    })
}

fn remembered(has_passkey: bool, at: String) -> Remembered {
    Remembered {
        email: EMAIL.into(),
        has_passkey,
        at,
    }
}

fn secret(name: &str, field: &str) -> SecretItem {
    SecretItem {
        name: name.into(),
        fields: vec![FieldView {
            key: field.into(),
            secret: true,
            value: String::new(),
        }],
    }
}

fn register_at(width_px: u32, selected: bool) -> String {
    let item = secret("kv/gitea/token", "token");
    let name = item.name.clone();
    render_register(&RegisterView {
        width_px,
        items: vec![item],
        selected: selected.then_some(name),
        wizard_open: false,
    })
}

fn sessions_two() -> Vec<SessionRow> {
    vec![
        SessionRow {
            id: "sess-this".into(),
            kind: "console".into(),
            label: "This browser".into(),
            created: "2026-01-01T00:00:00Z".into(),
            last_seen: "2026-01-02T00:00:00Z".into(),
            current: true,
        },
        SessionRow {
            id: "sess-nuc".into(),
            kind: "device".into(),
            label: "nuc".into(),
            created: "2026-01-01T00:00:00Z".into(),
            last_seen: "2026-01-02T00:00:00Z".into(),
            current: false,
        },
    ]
}

fn passkeys_two() -> Vec<PasskeyRow> {
    vec![
        PasskeyRow {
            id: "pk-1".into(),
            created: "2026-01-01T00:00:00Z".into(),
        },
        PasskeyRow {
            id: "pk-2".into(),
            created: "2026-01-02T00:00:00Z".into(),
        },
    ]
}

fn browser_bin() -> Option<PathBuf> {
    const NAMES: &[&str] = &[
        "thorium-browser",
        "thorium",
        "brave",
        "brave-browser",
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
    ];
    for name in NAMES {
        let p = PathBuf::from(format!("/usr/bin/{name}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn dump_dom(fragment: &str, width_px: u32) -> Option<String> {
    let bin = browser_bin()?;
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "secd-t6-{}-{}-{n}.html",
        std::process::id(),
        width_px
    ));
    let page = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>{fragment}</body></html>"
    );
    std::fs::write(&path, page).expect("write dump-dom fixture");
    let url = format!("file://{}", path.display());
    let out = Command::new(bin)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-default-browser-check",
            &format!("--window-size={width_px},812"),
            "--dump-dom",
            &url,
        ])
        .output()
        .expect("spawn headless dump-dom");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "dump-dom {width_px}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn assert_no_password_node(html: &str) {
    assert!(
        !has_password_input(html),
        "passkey-only gate must not contain a password input: {html}"
    );
}

fn assert_list_only(html: &str) {
    assert!(attr(html, "data-layout", "list-only"), "{html}");
    assert!(attr(html, "data-pane", "list"), "{html}");
    assert!(
        !attr(html, "data-pane", "inspector"),
        "375/list-only must not include inspector: {html}"
    );
}

fn assert_list_inspector(html: &str) {
    assert!(attr(html, "data-layout", "list-inspector"), "{html}");
    assert!(attr(html, "data-pane", "list"), "{html}");
    assert!(
        attr(html, "data-pane", "inspector"),
        "1280 must include inspector: {html}"
    );
    assert!(
        !attr(html, "data-pane", "sheet"),
        "1280 must not use the sheet: {html}"
    );
}

#[test]
fn T_WEB_PK_ONLY_NO_PASSWORD_NODE() {
    let view = passkey_only_view();
    assert_eq!(view.kind, GateKind::Identity);
    assert!(view.show_passkey);
    assert!(!view.show_password);
    let html = render_gate(&view);
    assert_no_password_node(&html);
    for width in [VIEW_MOBILE, VIEW_DESKTOP] {
        if let Some(dom) = dump_dom(&html, width) {
            assert_no_password_node(&dom);
        }
    }
}

#[test]
fn T_WEB_REMEMBERED_NO_EMAIL() {
    let at = fresh_at();
    let raw = format!(r#"{{"email":"{EMAIL}","has_passkey":true,"at":"{at}"}}"#);
    let last = parse_remembered(&raw).expect("secd.last");
    assert_eq!(LAST_KEY, "secd.last");
    assert!(last.has_passkey);
    assert!(remember_is_fresh_unix(&last.at, unix_now()));
    let view = resolve(GateQuery {
        remember: Some(last),
        ..GateQuery::default()
    });
    assert_eq!(view.kind, GateKind::RememberedPasskey);
    assert!(!view.show_email);
    let html = render_gate(&view);
    assert!(
        !has_email_field(&html),
        "fresh secd.last + has_passkey must omit the email field: {html}"
    );
}

#[test]
fn T_WEB_LIVE_SESSION_APPROVE_ONLY() {
    let view = resolve(GateQuery {
        session: Some(SessionInfo {
            email: EMAIL.into(),
            has_passkey: true,
            has_password: false,
            session_id: "sid-live".into(),
        }),
        user_code: Some("ABCD-EFGH".into()),
        ..GateQuery::default()
    });
    assert_eq!(view.kind, GateKind::ApproveOnly);
    assert!(view.show_approve);
    assert!(!view.show_email);
    assert!(!view.show_password);
    assert!(!view.show_passkey);
    let html = render_gate(&view);
    assert!(
        html.contains("Approve") && attr(&html, "data-action", "approve"),
        "live session must show Approve: {html}"
    );
    assert!(
        attr(&html, "data-page", "device") || html.contains("Approve this machine"),
        "live session is the device page: {html}"
    );
    assert!(!has_email_field(&html), "no email on approve-only: {html}");
    assert!(
        !has_password_input(&html),
        "no password on approve-only: {html}"
    );
}

#[test]
fn T_WEB_STALE_REMEMBER() {
    let at = stale_at();
    assert!(!remember_is_fresh_unix(&at, unix_now()));
    let view = resolve(GateQuery {
        remember: Some(remembered(true, at)),
        ..GateQuery::default()
    });
    assert!(
        view.show_email,
        "at older than 30d must show the email field"
    );
    let html = render_gate(&view);
    assert!(
        has_email_field(&html),
        "stale secd.last must show email: {html}"
    );
}

#[test]
fn T_WEB_CONDITIONAL() {
    let view = resolve(GateQuery::default());
    assert!(view.show_email);
    assert_eq!(view.email_autocomplete, Some(EMAIL_AUTOCOMPLETE));
    assert!(
        EMAIL_AUTOCOMPLETE.contains("webauthn"),
        "{EMAIL_AUTOCOMPLETE}"
    );
    let html = render_gate(&view);
    assert!(has_email_field(&html), "cold gate shows email: {html}");
    assert!(
        html.contains("webauthn"),
        "email autocomplete must include webauthn: {html}"
    );
}

#[test]
fn T_WEB_XSS_NAME() {
    let html = render_register(&RegisterView {
        width_px: VIEW_DESKTOP,
        items: vec![SecretItem {
            name: XSS_NAME.into(),
            fields: vec![],
        }],
        selected: Some(XSS_NAME.into()),
        wizard_open: false,
    });
    assert!(
        !has_script_element(&html),
        "name must render as text, not a script node: {html}"
    );
    assert!(
        html.contains("&lt;script&gt;") || html.contains("&lt;script"),
        "name must be escaped text: {html}"
    );
}

#[test]
fn T_WEB_MOBILE_BREAKPOINT() {
    assert_eq!(BREAKPOINT_PX, 900);
    assert_eq!(layout_mode(VIEW_MOBILE), LayoutMode::ListOnly);
    assert_eq!(layout_mode(899), LayoutMode::ListOnly);
    assert_eq!(layout_mode(900), LayoutMode::ListInspector);
    assert_eq!(layout_mode(VIEW_DESKTOP), LayoutMode::ListInspector);
    let mobile = register_at(VIEW_MOBILE, true);
    let desktop = register_at(VIEW_DESKTOP, true);
    assert_list_only(&mobile);
    assert_list_inspector(&desktop);
    if let Some(dom) = dump_dom(&mobile, VIEW_MOBILE) {
        assert_list_only(&dom);
    }
    if let Some(dom) = dump_dom(&desktop, VIEW_DESKTOP) {
        assert_list_inspector(&dom);
    }
}

#[test]
fn T_WEB_COPY_DEFAULT() {
    assert_eq!(primary_field_action(), FieldAction::Copy);
    assert_ne!(primary_field_action(), FieldAction::Show);
    let html = register_at(VIEW_DESKTOP, true);
    let copy = html
        .find(r#"data-action="copy""#)
        .expect("copy action on field");
    let show = html
        .find(r#"data-action="show""#)
        .expect("show action on field");
    assert!(copy < show, "copy is the primary field action: {html}");
    assert!(
        html[copy.saturating_sub(32)..copy].contains("primary"),
        "copy button is the primary action: {html}"
    );
    assert!(
        attr(&html, "data-hold", "1"),
        "reveal is hold-to-show, not primary: {html}"
    );
}

#[test]
fn T_WEB_ACCOUNT_SESSIONS() {
    let view = AccountView {
        email: EMAIL.into(),
        sessions: sessions_two(),
        passkeys: vec![],
        has_password: true,
    };
    let html = render_account(&view);
    assert!(attr(&html, "data-list", "sessions"), "{html}");
    assert!(html.contains("sess-this"), "{html}");
    assert!(html.contains("sess-nuc"), "{html}");
    assert_eq!(
        count(&html, r#"data-action="revoke""#),
        view.sessions.len(),
        "each session row has Revoke: {html}"
    );
    assert!(html.contains("Revoke"), "{html}");
}

#[test]
fn T_WEB_ACCOUNT_REVOKE() {
    let view = AccountView {
        email: EMAIL.into(),
        sessions: sessions_two(),
        passkeys: vec![],
        has_password: true,
    };
    let html = render_account(&view);
    assert!(
        html.contains(r#"data-session-id="sess-nuc""#),
        "non-current row is listed: {html}"
    );
    assert!(
        html.contains(r#"data-action="revoke""#),
        "non-current row offers Revoke: {html}"
    );
    assert_eq!(session_revoke_path("sess-nuc"), "/api/v1/sessions/sess-nuc");
    assert!(
        APP_JS.contains(r#"req("DELETE", "/api/v1/sessions/" + encodeURIComponent(id))"#),
        "Revoke on a row must DELETE /api/v1/sessions/:id"
    );
}

#[test]
fn T_WEB_ACCOUNT_PASSKEYS() {
    let view = AccountView {
        email: EMAIL.into(),
        sessions: vec![],
        passkeys: passkeys_two(),
        has_password: true,
    };
    let html = render_account(&view);
    assert!(attr(&html, "data-list", "passkeys"), "{html}");
    assert!(html.contains("pk-1") && html.contains("pk-2"), "{html}");
    assert!(
        attr(&html, "data-action", "add-passkey") && html.contains("Add passkey"),
        "Add passkey present: {html}"
    );
    assert!(
        attr(&html, "data-action", "remove") && html.contains("Remove"),
        "Remove present: {html}"
    );
}

#[test]
fn T_WEB_ACCOUNT_REMOVE_LAST_DISABLED() {
    assert!(!remove_passkey_enabled(1, false));
    let view = AccountView {
        email: EMAIL.into(),
        sessions: vec![],
        passkeys: vec![PasskeyRow {
            id: "pk-only".into(),
            created: "2026-01-01T00:00:00Z".into(),
        }],
        has_password: false,
    };
    assert!(!view.remove_enabled());
    let html = render_account(&view);
    let offered = attr(&html, "data-action", "remove");
    if offered {
        assert!(
            html.contains("disabled"),
            "sole passkey and no password: Remove is disabled: {html}"
        );
    }
}
