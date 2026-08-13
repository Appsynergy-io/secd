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
