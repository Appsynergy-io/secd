/// Pixel-less terminal rectangle. Same fields as a ratatui rect.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, column: u16, row: u16) -> bool {
        column >= self.x
            && row >= self.y
            && column < self.x.saturating_add(self.width)
            && row < self.y.saturating_add(self.height)
    }

    pub fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }
}

impl From<ratatui::layout::Rect> for Rect {
    fn from(r: ratatui::layout::Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

impl From<Rect> for ratatui::layout::Rect {
    fn from(r: Rect) -> Self {
        ratatui::layout::Rect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// Header 1, activity 8, list|detail 42/58 of the leftover width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Regions {
    pub header: Rect,
    pub list: Rect,
    pub detail: Rect,
    pub activity: Rect,
}

pub fn regions(area: Rect) -> Regions {
    let header_h = area.height.min(1);
    let rest = area.height.saturating_sub(header_h);
    let activity_h = rest.min(8);
    let mid_h = rest.saturating_sub(activity_h);
    let list_w = area.width.saturating_mul(42) / 100;
    let detail_w = area.width.saturating_sub(list_w);
    let mid_y = area.y.saturating_add(header_h);
    Regions {
        header: Rect::new(area.x, area.y, area.width, header_h),
        list: Rect::new(area.x, mid_y, list_w, mid_h),
        detail: Rect::new(area.x.saturating_add(list_w), mid_y, detail_w, mid_h),
        activity: Rect::new(area.x, mid_y.saturating_add(mid_h), area.width, activity_h),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Action {
    pub key: char,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionHit {
    pub key: char,
    pub rect: Rect,
}

/// Whole buttons only: a button that would clip is omitted, not shortened.
pub fn action_bar(area: Rect, actions: &[Action]) -> Vec<ActionHit> {
    layout_actions(area, actions)
        .into_iter()
        .map(|(a, r)| ActionHit {
            key: a.key,
            rect: r,
        })
        .collect()
}

pub fn hit_key(hits: &[ActionHit], column: u16, row: u16) -> Option<char> {
    hits.iter()
        .find(|h| h.rect.contains(column, row))
        .map(|h| h.key)
}

pub fn action_bar_row(area: Rect, placed: &Regions) -> Rect {
    let _ = area;
    let a = placed.activity;
    if a.height == 0 {
        return Rect::new(a.x, a.y, a.width, 0);
    }
    Rect::new(
        a.x,
        a.y.saturating_add(a.height.saturating_sub(1)),
        a.width,
        1,
    )
}

pub fn button_text(action: Action) -> String {
    format!("[{}] {}", action.key, action.label)
}

pub(crate) fn layout_actions(area: Rect, actions: &[Action]) -> Vec<(Action, Rect)> {
    let mut x = area.x;
    let mut out = Vec::new();
    for action in actions {
        let w = button_width(*action);
        let gap = if out.is_empty() { 0 } else { 2 };
        let next = x.saturating_add(gap);
        if next.saturating_add(w) > area.right() || w > area.width {
            break;
        }
        x = next;
        out.push((*action, Rect::new(x, area.y, w, area.height.min(1))));
        x = x.saturating_add(w);
    }
    out
}

fn button_width(action: Action) -> u16 {
    u16::try_from(button_text(action).chars().count()).unwrap_or(u16::MAX)
}

/// What a click landed on. Not a key: inside a form every letter types text,
/// so a modal button cannot be keyed like an action-bar button.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Spot {
    /// Row of the names list.
    Row(usize),
    /// Input of a form field.
    Field(usize),
    /// The show|hide toggle of a form field.
    Reveal(usize),
    /// Row of the schema picker.
    Choice(usize),
    Save,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpotHit {
    pub spot: Spot,
    pub rect: Rect,
}

pub fn spot_at(hits: &[SpotHit], column: u16, row: u16) -> Option<Spot> {
    hits.iter()
        .find(|h| h.rect.contains(column, row))
        .map(|h| h.spot)
}

/// The interior of a one-cell border.
pub fn inset(r: Rect) -> Rect {
    Rect::new(
        r.x.saturating_add(1),
        r.y.saturating_add(1),
        r.width.saturating_sub(2),
        r.height.saturating_sub(2),
    )
}

/// What one paint decided, so the next one continues from it rather than
/// recomputing a window from the selection alone.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Painted {
    pub spots: Vec<SpotHit>,
    /// First row of the register list.
    pub list: usize,
    /// First row of the modal on screen.
    pub modal: usize,
}

/// Rows kept off the top and bottom edge, so the next row is always visible.
const SCROLLOFF: usize = 2;

/// First row drawn, so a selection below the fold scrolls into view. The draw
/// and the hit table both window through here, so they cannot disagree.
///
/// `prev` is where the pane was, and the window only moves when the selection
/// would leave it. Recomputing from the selection alone pins it to the bottom
/// edge, so moving from row 50 to row 2 scrolls 38 times before the cursor
/// appears to move at all, then changes behaviour halfway.
pub fn window_start(prev: usize, sel: usize, len: usize, h: usize) -> usize {
    if h == 0 || len <= h {
        return 0;
    }
    let last = len - h;
    let pad = SCROLLOFF.min(h.saturating_sub(1) / 2);
    let mut start = prev.min(last);
    if sel < start + pad {
        start = sel.saturating_sub(pad);
    }
    if sel + pad >= start + h {
        start = (sel + pad + 1).saturating_sub(h);
    }
    start.min(last)
}

/// One rect per visible row of a list drawn into `body`, from `start`.
pub fn rows_at(body: Rect, start: usize, count: usize, spot: fn(usize) -> Spot) -> Vec<SpotHit> {
    let h = body.height as usize;
    let n = count.saturating_sub(start).min(h);
    (0..n)
        .map(|i| SpotHit {
            spot: spot(start + i),
            rect: Rect::new(
                body.x,
                body.y.saturating_add(u16::try_from(i).unwrap_or(u16::MAX)),
                body.width,
                1,
            ),
        })
        .collect()
}

/// A modal box centred in `area`, tall enough for `rows` body lines plus its
/// border. Never larger than the area: rows that do not fit scroll instead.
pub fn modal_box(area: Rect, rows: u16) -> Rect {
    let w = area.width.saturating_sub(4).min(76).max(area.width.min(24));
    let want = rows.saturating_add(2);
    let h = want.min(area.height).max(area.height.min(5));
    let x = area.x.saturating_add(area.width.saturating_sub(w) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(h) / 2);
    Rect::new(x, y, w, h)
}

/// Column plan for a detail row: key, value, env, one space between each.
///
/// The value is fed first. Letting the key and env columns take their full
/// natural width and giving the value what is left starves the one column that
/// carries the thing you came to read: a full Cloudflare bundle at 80 columns
/// leaves it five cells, and under 73 columns, none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetailCols {
    pub key: u16,
    pub value: u16,
    pub env: u16,
}

pub fn detail_cols(width: u16, key_w: u16, env_w: u16) -> DetailCols {
    if key_w == 0 {
        return DetailCols {
            key: 0,
            value: width,
            env: 0,
        };
    }
    let mut key = key_w;
    let mut env = env_w;
    // A gap before each column that is present.
    let need = |k: u16, e: u16| {
        k.saturating_add(1)
            .saturating_add(if e > 0 { e + 1 } else { 0 })
    };
    while width.saturating_sub(need(key, env)) < GOOD_VALUE {
        if env > 0 {
            env = 0;
        } else if key > MIN_KEY {
            key = key.saturating_sub(1);
        } else {
            break;
        }
    }
    let value = width.saturating_sub(need(key, env));
    DetailCols { key, value, env }
}

/// A value narrower than this is not worth the columns that describe it.
const GOOD_VALUE: u16 = 16;

/// A key cut below this stops naming its field.
const MIN_KEY: u16 = 6;

/// Column plan for a form row: label, input, tag, env, one space between each.
/// The env column is dropped first when the row is narrow, then the tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormCols {
    pub label: u16,
    pub input: u16,
    pub tag: u16,
    pub env: u16,
}

/// Width of the widest tag word, `required`.
pub const TAG_W: u16 = 8;
/// Below this an input is not worth a column beside it at all.
const MIN_INPUT: u16 = 8;
/// An input worth describing: the env column is granted only above it.
const GOOD_INPUT: u16 = 24;

pub fn form_cols(width: u16, label_w: u16, env_w: u16) -> FormCols {
    let label = label_w.min(18).min(width);
    let rest = width.saturating_sub(label).saturating_sub(1);
    let mut tag = TAG_W;
    let mut env = env_w.min(24);
    // The input is what a human types into, so it is fed before the columns
    // that only describe it.
    if rest < GOOD_INPUT + 1 + tag + 1 + env {
        env = 0;
    }
    if rest < MIN_INPUT + 1 + tag {
        tag = 0;
    }
    let mut used = 0u16;
    if tag > 0 {
        used = used.saturating_add(1).saturating_add(tag);
    }
    if env > 0 {
        used = used.saturating_add(1).saturating_add(env);
    }
    FormCols {
        label,
        input: rest.saturating_sub(used),
        tag,
        env,
    }
}

/// One rect per visible form row, from `start`. The rect spans label and
/// input, so clicking the label focuses the field.
pub fn field_rects(body: Rect, cols: FormCols, start: usize, count: usize) -> Vec<SpotHit> {
    let w = cols
        .label
        .saturating_add(1)
        .saturating_add(cols.input)
        .min(body.width);
    rows_at(body, start, count, Spot::Field)
        .into_iter()
        .map(|h| SpotHit {
            spot: h.spot,
            rect: Rect::new(h.rect.x, h.rect.y, w, 1),
        })
        .collect()
}

pub fn toggle_text(shown: bool) -> &'static str {
    if shown {
        "[hide]"
    } else {
        "[show]"
    }
}

/// The show|hide toggle for a secret row, right-aligned inside its input
/// column. `None` when the row is too narrow: a control that would clip is
/// omitted, never shortened.
pub fn reveal_rect(row: Rect, cols: FormCols, shown: bool) -> Option<Rect> {
    let w = u16::try_from(toggle_text(shown).chars().count()).ok()?;
    if cols.input < w.saturating_add(MIN_INPUT) {
        return None;
    }
    let x = row
        .x
        .saturating_add(cols.label)
        .saturating_add(1)
        .saturating_add(cols.input)
        .saturating_sub(w);
    Some(Rect::new(x, row.y, w, 1))
}

pub fn spot_text(label: &str) -> String {
    format!("[{label}]")
}

/// `[cancel]` and the commit button on the box's last interior row, right
/// aligned. Whole buttons only.
pub fn modal_buttons(box_r: Rect, commit: &str) -> Vec<SpotHit> {
    let body = inset(box_r);
    if body.height == 0 || body.width == 0 {
        return Vec::new();
    }
    let y = body.y.saturating_add(body.height.saturating_sub(1));
    let cw = u16::try_from(spot_text(commit).chars().count()).unwrap_or(u16::MAX);
    let xw = u16::try_from(spot_text("cancel").chars().count()).unwrap_or(u16::MAX);
    let total = cw.saturating_add(2).saturating_add(xw);
    if total > body.width {
        return Vec::new();
    }
    let x = body.x.saturating_add(body.width).saturating_sub(total);
    vec![
        SpotHit {
            spot: Spot::Cancel,
            rect: Rect::new(x, y, xw, 1),
        },
        SpotHit {
            spot: Spot::Save,
            rect: Rect::new(x.saturating_add(xw).saturating_add(2), y, cw, 1),
        },
    ]
}

/// The console's mask: 8 to 24 bullets, so its width does not report the
/// length of what it hides.
pub fn mask(len: usize) -> String {
    "\u{2022}".repeat(len.clamp(8, 24))
}
