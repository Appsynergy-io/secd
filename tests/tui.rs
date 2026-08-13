#![allow(non_snake_case)]

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use secd::tui::{
    action_bar, draw_action_bar, hit_key, regions, Action, Event, Modal, Mode, Model, Rect, ACCENT,
    DANGER, DIM, HIGHLIGHT, OK, ON_ACCENT, WARN,
};

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
