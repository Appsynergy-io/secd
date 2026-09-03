#![allow(non_snake_case)]

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::BTreeMap;

use secd::policy;
use secd::tui::{
    action_bar, draw, draw_action_bar, hit_key, mask, regions, Action, Event, Modal, Mode, Model,
    Rect, Spot, SpotHit, ACCENT, DANGER, DIM, HIGHLIGHT, OK, ON_ACCENT, WARN,
};
use secd::Secret;
use secd_core::{build_payload, providers};

fn button_text(action: Action) -> String {
    format!("[{}] {}", action.key, action.label)
}

fn row_text(buf: &ratatui::buffer::Buffer, y: u16, width: u16) -> String {
    let mut out = String::new();
    for x in 0..width {
        out.push_str(buf[(x, y)].symbol());
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn contains_ident(src: &str, ident: &str) -> bool {
    let hay = src.as_bytes();
    let needle = ident.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    for i in 0..=hay.len() - needle.len() {
        if &hay[i..i + needle.len()] != needle {
            continue;
        }
        let left_ok = i == 0 || !is_ident_byte(hay[i - 1]);
        let right = i + needle.len();
        let right_ok = right == hay.len() || !is_ident_byte(hay[right]);
        if left_ok && right_ok {
            return true;
        }
    }
    false
}

#[test]
fn T_TUI_REGIONS() {
    for (w, h) in [(80u16, 24u16), (100, 24), (160, 40)] {
        let area = Rect::new(2, 3, w, h);
        let r = regions(area);
        let list_w = w.saturating_mul(42) / 100;
        let mid_h = h.saturating_sub(1).saturating_sub(8);
        assert_eq!(r.header, Rect::new(area.x, area.y, w, 1), "header 1 w={w}");
        assert_eq!(
            r.activity,
            Rect::new(area.x, area.y + 1 + mid_h, w, 8),
            "activity 8 w={w}"
        );
        assert_eq!(
            r.list,
            Rect::new(area.x, area.y + 1, list_w, mid_h),
            "list 42 w={w}"
        );
        assert_eq!(
            r.detail,
            Rect::new(area.x + list_w, area.y + 1, w - list_w, mid_h),
            "detail 58 w={w}"
        );
        assert_eq!(r.list.width + r.detail.width, w);
    }
    let even = regions(Rect::new(0, 0, 100, 24));
    assert_eq!(even.list.width, 42);
    assert_eq!(even.detail.width, 58);
}

#[test]
fn T_TUI_HIT_EQ_DRAW() {
    let actions = Model::new().actions().to_vec();
    let bar = Rect::new(0, 0, 80, 1);
    let hits = action_bar(bar, &actions);
    assert!(!hits.is_empty(), "idle action bar places buttons");

    let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("term");
    terminal
        .draw(|f| draw_action_bar(f, bar, &actions))
        .expect("draw");
    let buf = terminal.backend().buffer();
    let row = row_text(buf, 0, 80);

    let mut drawn_at = Vec::new();
    for hit in &hits {
        let action = actions
            .iter()
            .copied()
            .find(|a| a.key == hit.key)
            .expect("hit key is an action");
        let label = button_text(action);
        assert_eq!(
            hit.rect.width as usize,
            label.chars().count(),
            "rect width is the full button, not a clip"
        );
        let start = hit.rect.x as usize;
        let end = start + label.len();
        assert_eq!(&row[start..end], label, "drawn text at action_bar rect");
        drawn_at.push(start);
    }

    let starts: Vec<usize> = row
        .match_indices('[')
        .filter(|(i, _)| row.as_bytes().get(i + 2) == Some(&b']'))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(starts, drawn_at, "action_bar rects == drawn buttons");
}

#[test]
fn T_TUI_CLIP() {
    let actions = [
        Action {
            key: 'a',
            label: "one-button-that-takes-space-here",
        },
        Action {
            key: 'b',
            label: "two-button-that-takes-space-here",
        },
        Action {
            key: 'c',
            label: "clip-me-because-I-do-not-fit-80",
        },
    ];
    let full_a = button_text(actions[0]);
    let full_b = button_text(actions[1]);
    let full_c = button_text(actions[2]);
    let wa = u16::try_from(full_a.chars().count()).unwrap();
    let wb = u16::try_from(full_b.chars().count()).unwrap();
    let wc = u16::try_from(full_c.chars().count()).unwrap();
    let used = wa + 2 + wb;
    assert!(used < 80, "first two buttons fit at width 80");
    assert!(used + 2 + wc > 80, "third button would clip at width 80");
    assert!(
        80 - used >= 3,
        "leftover would fit a truncated [c] if clip shortened"
    );

    let bar = Rect::new(0, 0, 80, 1);
    let hits = action_bar(bar, &actions);
    assert_eq!(hits.len(), 2, "button that would clip is omitted");
    assert_eq!(hits[0].key, 'a');
    assert_eq!(hits[1].key, 'b');
    assert_eq!(hits[0].rect.width, wa);
    assert_eq!(hits[1].rect.width, wb);
    assert!(hits.iter().all(|h| h.rect.right() <= 80));
    assert!(hits.iter().all(|h| h.key != 'c'));

    let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("term");
    terminal
        .draw(|f| draw_action_bar(f, bar, &actions))
        .expect("draw");
    let row = row_text(terminal.backend().buffer(), 0, 80);
    assert!(row.contains(&full_a));
    assert!(row.contains(&full_b));
    assert!(
        !row.contains("[c]"),
        "clipped button is omitted, not shortened: {row:?}"
    );
}

#[test]
fn T_TUI_CLICK_IS_KEY() {
    let mut by_click = Model::new();
    let mut by_key = Model::new();
    let actions = by_click.actions().to_vec();
    let bar = Rect::new(0, 12, 80, 1);
    let hits = action_bar(bar, &actions);
    let a = *hits.iter().find(|h| h.key == 'a').expect("button a");
    assert_eq!(hit_key(&hits, a.rect.x, a.rect.y), Some('a'));
    assert_eq!(
        hit_key(&hits, a.rect.x + a.rect.width / 2, a.rect.y),
        Some('a')
    );

    by_click.set_hits(hits);
    by_click.handle(Event::Click {
        column: a.rect.x + a.rect.width / 2,
        row: a.rect.y,
    });
    by_key.handle(Event::Key('a'));
    assert_eq!(by_click.mode(), by_key.mode());
    assert!(matches!(by_click.mode(), Mode::Modal(Modal::Add { .. })));
}

#[test]
fn T_TUI_ESC_IDLE() {
    let mut model = Model::new();
    assert!(model.is_idle());
    assert_eq!(model.title(), "register");
    assert!(!model.quit());
    model.handle(Event::Esc);
    assert!(model.quit(), "Esc on idle register sets quit");
    assert!(model.is_idle());
}

#[test]
fn T_TUI_ESC_MODAL() {
    let mut model = Model::new();
    model.handle(Event::Key('a'));
    assert!(matches!(model.mode(), Mode::Modal(Modal::Add { .. })));
    model.handle(Event::Key('x'));
    model.handle(Event::Esc);
    assert!(model.is_idle(), "Esc on modal cancels");
    assert!(!model.quit(), "Esc on modal does not quit");
    assert!(model.names().is_empty());
}

#[test]
fn T_TUI_VIEW_NO_GET() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui/view.rs");
    let src = std::fs::read_to_string(&path).expect("src/tui/view.rs");
    for ident in ["vault", "get", "decrypt", "open"] {
        assert!(
            !contains_ident(&src, ident),
            "view.rs must not contain {ident}"
        );
    }
}

#[test]
fn T_TUI_THEME() {
    assert_eq!(ACCENT, "#A78BFA");
    assert_eq!(HIGHLIGHT, "#F48FCD");
    assert_eq!(OK, "#6ED6BA");
    assert_eq!(WARN, "#F0BE6E");
    assert_eq!(DANGER, "#F87185");
    assert_eq!(DIM, "#7A7A94");
    assert_eq!(ON_ACCENT, "#181825");
}

/// Cells `x..x+n` of row `y`, by cell index rather than byte offset: a border
/// glyph is one cell and three bytes.
fn cell_text(buf: &ratatui::buffer::Buffer, x: u16, y: u16, n: u16) -> String {
    (0..n).map(|i| buf[(x + i, y)].symbol()).collect()
}

fn screen_text(buf: &ratatui::buffer::Buffer, w: u16, h: u16) -> String {
    (0..h)
        .map(|y| row_text(buf, y, w))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Paint `model` into a `w` by `h` terminal, and hand back what a click can
/// land on and what was drawn.
fn render(model: &Model, w: u16, h: u16) -> (Vec<SpotHit>, String) {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("term");
    let mut spots = Vec::new();
    terminal
        .draw(|f| {
            spots = draw(f, model);
        })
        .expect("draw");
    let text = screen_text(terminal.backend().buffer(), w, h);
    (spots, text)
}

fn provider_form(name: &str) -> Model {
    let mut m = Model::new();
    let i = m
        .schemas()
        .iter()
        .position(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} schema"));
    m.handle(Event::Key('p'));
    for _ in 0..i {
        m.handle(Event::Down);
    }
    m.handle(Event::Enter);
    assert!(m.form().is_some(), "the schema form is open");
    m
}

fn bundle_register() -> Model {
    let mut m = Model::new();
    m.apply_loaded(policy::VaultLoad {
        entries: vec![
            policy::Entry {
                name: "kv/plain".into(),
                value: Secret::new(b"PLAINVALUE".to_vec()),
                meta: serde_json::json!({}),
            },
            policy::Entry {
                name: "prod/cf".into(),
                value: Secret::new(
                    br#"{"account_id":"ACCTVISIBLE","api_token":"TOKENSECRET"}"#.to_vec(),
                ),
                meta: serde_json::json!({
                    "provider": "cloudflare",
                    "fields": ["account_id", "api_token"],
                }),
            },
        ],
        raw: 2,
        body: String::new(),
        before: BTreeMap::new(),
    });
    m
}

#[test]
fn T_TUI_PAYLOAD() {
    let cf = providers()
        .iter()
        .find(|p| p.name == "cloudflare")
        .expect("cloudflare");
    let vals = vec![
        ("api_token".to_string(), "  tok  ".to_string()),
        ("account_id".to_string(), "acct".to_string()),
        ("zone_id".to_string(), "   ".to_string()),
        ("not_in_the_schema".to_string(), "x".to_string()),
    ];
    let got = build_payload(&cf.fields, &vals).expect("both required fields are filled");
    assert_eq!(
        got,
        vec![
            ("account_id".to_string(), "acct".to_string()),
            ("api_token".to_string(), "tok".to_string()),
        ],
        "schema order, trimmed, and only what the schema names"
    );
    assert!(
        got.iter().all(|(k, _)| k != "zone_id"),
        "an empty optional is absent, not empty"
    );

    assert!(
        build_payload(&cf.fields, &[("api_token".into(), "tok".into())]).is_none(),
        "a missing required field refuses"
    );
    assert!(
        build_payload(
            &cf.fields,
            &[
                ("account_id".into(), "   ".into()),
                ("api_token".into(), "tok".into()),
            ]
        )
        .is_none(),
        "a whitespace-only required field refuses"
    );

    // The order is the schema's, not the sorted one. The widest schema is
    // where the two visibly differ.
    let wide = providers()
        .iter()
        .max_by_key(|p| p.fields.len())
        .expect("a widest schema");
    let filled: Vec<(String, String)> = wide
        .fields
        .iter()
        .map(|f| (f.key.clone(), "v".to_string()))
        .collect();
    let keys: Vec<String> = build_payload(&wide.fields, &filled)
        .expect("all filled")
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    let want: Vec<String> = wide.fields.iter().map(|f| f.key.clone()).collect();
    assert_eq!(keys, want);
    let mut sorted = keys.clone();
    sorted.sort();
    assert_ne!(keys, sorted, "the schema order is not the sorted order");
}

#[test]
fn T_TUI_PROVIDER_META() {
    let pairs = vec![
        ("account_id".to_string(), "acct".to_string()),
        ("api_token".to_string(), "tok".to_string()),
    ];
    let keys: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
    let meta = policy::provider_meta("cloudflare", &keys);
    assert_eq!(meta["provider"], serde_json::json!("cloudflare"));
    assert_eq!(
        meta["fields"],
        serde_json::json!(["account_id", "api_token"]),
        "the field list is in schema order"
    );
    let obj = meta.as_object().expect("an object");
    assert_eq!(obj.len(), 2);
    for k in ["value", "plaintext", "dek"] {
        assert!(!obj.contains_key(k), "the server refuses meta carrying {k}");
    }

    assert_eq!(
        policy::payload_json(&pairs),
        r#"{"account_id":"acct","api_token":"tok"}"#
    );
    assert_eq!(
        policy::payload_json(&[
            ("b".to_string(), "1".to_string()),
            ("a".to_string(), "2".to_string()),
        ]),
        r#"{"b":"1","a":"2"}"#,
        "the order given survives, where a serde map would sort"
    );
    let odd = vec![("k".to_string(), "a\"b\\c\nd\te\u{1}f".to_string())];
    let back: serde_json::Value =
        serde_json::from_str(&policy::payload_json(&odd)).expect("valid JSON");
    assert_eq!(back["k"], serde_json::json!("a\"b\\c\nd\te\u{1}f"));

    // What the register writes is what the CLI's bundle resolution reads back.
    let entry = policy::Entry {
        name: "prod/cloudflare".to_string(),
        value: Secret::new(policy::payload_json(&pairs).into_bytes()),
        meta,
    };
    let bundles = policy::discover_bundles(std::slice::from_ref(&entry));
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].provider, "cloudflare");
    assert_eq!(bundles[0].name, "prod/cloudflare");
    assert_eq!(
        bundles[0].fields.get("api_token").map(String::as_str),
        Some("tok")
    );
}

#[test]
fn T_TUI_FORM_EDIT() {
    let mut m = Model::new();
    m.handle(Event::Key('a'));
    let text = |m: &Model| m.form().expect("form").fields[0].input.text();
    let caret = |m: &Model| m.form().expect("form").fields[0].input.caret();

    for c in "abc".chars() {
        m.handle(Event::Key(c));
    }
    m.handle(Event::Left);
    m.handle(Event::Left);
    m.handle(Event::Key('X'));
    assert_eq!(text(&m), "aXbc");
    assert_eq!(caret(&m), 2);
    m.handle(Event::Backspace);
    assert_eq!(text(&m), "abc");
    m.handle(Event::Delete);
    assert_eq!(text(&m), "ac");
    m.handle(Event::Home);
    m.handle(Event::Delete);
    assert_eq!(text(&m), "c");
    m.handle(Event::End);
    m.handle(Event::Backspace);
    assert_eq!(text(&m), "");

    m.handle(Event::Key('é'));
    m.handle(Event::Key('字'));
    assert_eq!(text(&m), "é字");
    m.handle(Event::Backspace);
    assert_eq!(
        text(&m),
        "é",
        "one Backspace is one character, not one byte"
    );

    assert_eq!(m.form().expect("form").focus, 0);
    m.handle(Event::Tab);
    assert_eq!(m.form().expect("form").focus, 1);
    m.handle(Event::Tab);
    assert_eq!(m.form().expect("form").focus, 0, "Tab wraps");
    m.handle(Event::BackTab);
    assert_eq!(m.form().expect("form").focus, 1, "BackTab wraps");
}

#[test]
fn T_TUI_FORM_CLICK() {
    let mut m = provider_form("cloudflare");
    let (spots, _) = render(&m, 80, 24);
    let field3 = spots
        .iter()
        .find(|h| h.spot == Spot::Field(3))
        .copied()
        .expect("a fourth row");

    // The label is drawn where the hit table says the field is.
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("term");
    terminal
        .draw(|f| {
            draw(f, &m);
        })
        .expect("draw");
    let key = m.form().expect("form").fields[3].key.clone();
    let width = u16::try_from(key.chars().count()).expect("a short key");
    assert_eq!(
        cell_text(
            terminal.backend().buffer(),
            field3.rect.x,
            field3.rect.y,
            width
        ),
        key,
        "field 3's label sits at field 3's hit rect"
    );

    m.set_spots(spots.clone());
    m.handle(Event::Click {
        column: field3.rect.x + field3.rect.width / 2,
        row: field3.rect.y,
    });
    assert_eq!(
        m.form().expect("form").focus,
        3,
        "a click focuses the field"
    );

    // The focused secret field carries a clickable reveal.
    let secret = m
        .form()
        .expect("form")
        .fields
        .iter()
        .position(|f| f.secret)
        .expect("a secret field");
    let hit = spots
        .iter()
        .find(|h| h.spot == Spot::Field(secret))
        .copied()
        .expect("a rect for it");
    m.handle(Event::Click {
        column: hit.rect.x,
        row: hit.rect.y,
    });
    assert_eq!(m.form().expect("form").focus, secret);
    let (spots, _) = render(&m, 80, 24);
    let toggle = spots
        .iter()
        .find(|h| h.spot == Spot::Reveal(secret))
        .copied()
        .expect("the focused secret field carries a toggle");
    m.set_spots(spots);
    m.handle(Event::Click {
        column: toggle.rect.x,
        row: toggle.rect.y,
    });
    assert!(
        m.form().expect("form").fields[secret].shown,
        "clicking the toggle reveals that field"
    );
}

#[test]
fn T_TUI_MASK() {
    assert_eq!(
        mask(3).chars().count(),
        8,
        "a short value is not advertised"
    );
    assert_eq!(mask(11).chars().count(), 11);
    assert_eq!(
        mask(9999).chars().count(),
        24,
        "the mask does not report the length"
    );

    let mut m = bundle_register();
    let (_, screen) = render(&m, 100, 24);
    assert!(
        !screen.contains("PLAINVALUE"),
        "a plain value is masked until it is asked for"
    );

    m.handle(Event::Down);
    let (_, screen) = render(&m, 100, 24);
    assert!(screen.contains("Cloudflare"), "the schema names the entry");
    assert!(
        !screen.contains("TOKENSECRET"),
        "a field the schema marks secret is masked: {screen}"
    );
    assert!(
        screen.contains("ACCTVISIBLE"),
        "a field it does not mark secret is shown"
    );
    assert!(screen.contains('\u{2022}'), "the mask is drawn");

    m.handle(Event::Reveal);
    let (_, screen) = render(&m, 100, 24);
    assert!(screen.contains("TOKENSECRET"), "Ctrl-R reveals");
    m.handle(Event::Key('r'));
    let (_, screen) = render(&m, 100, 24);
    assert!(!screen.contains("TOKENSECRET"), "and hides again");

    m.handle(Event::Reveal);
    m.handle(Event::Up);
    m.handle(Event::Down);
    let (_, screen) = render(&m, 100, 24);
    assert!(
        !screen.contains("TOKENSECRET"),
        "a move does not carry the reveal with it"
    );
}

#[test]
fn T_TUI_MODAL_FITS() {
    let wide = providers()
        .iter()
        .max_by_key(|p| p.fields.len())
        .expect("a widest schema");
    let want = wide.fields.len() + 1;
    let mut m = provider_form(&wide.name);
    assert_eq!(
        m.form().expect("form").fields.len(),
        want,
        "every schema field, plus the name row"
    );

    let (spots, _) = render(&m, 80, 24);
    let fields: Vec<SpotHit> = spots
        .iter()
        .filter(|h| matches!(h.spot, Spot::Field(_)))
        .copied()
        .collect();
    assert_eq!(fields.len(), want, "every field is placed at 80x24");
    for h in &spots {
        assert!(
            h.rect.right() <= 80 && h.rect.y < 24,
            "every target is on screen: {:?}",
            h.rect
        );
    }
    let mut ys: Vec<u16> = fields.iter().map(|h| h.rect.y).collect();
    ys.sort_unstable();
    ys.dedup();
    assert_eq!(ys.len(), fields.len(), "no two field rows overlap");
    assert!(
        spots.iter().any(|h| h.spot == Spot::Save) && spots.iter().any(|h| h.spot == Spot::Cancel),
        "save and cancel are clickable"
    );

    // A short terminal clamps the box and scrolls to the focused row.
    m.handle(Event::PageDown);
    let last = want - 1;
    assert_eq!(m.form().expect("form").focus, last);
    let (spots, _) = render(&m, 80, 12);
    let fields: Vec<SpotHit> = spots
        .iter()
        .filter(|h| matches!(h.spot, Spot::Field(_)))
        .copied()
        .collect();
    assert!(
        fields.len() < want && !fields.is_empty(),
        "the box clamps rather than growing past the screen"
    );
    for h in &spots {
        assert!(
            h.rect.right() <= 80 && h.rect.y < 12,
            "still on screen: {:?}",
            h.rect
        );
    }
    assert!(
        fields.iter().any(|h| h.spot == Spot::Field(last)),
        "the focused row is inside the window"
    );
}
