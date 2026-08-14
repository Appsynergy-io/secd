//! Floating position — place a floating element (tooltip, popover, menu,
//! select content) relative to an anchor, with collision handling. Replaces
//! `@radix-ui/react-popper`'s default middleware stack: offset → flip
//! (primary axis) → shift (cross axis, clamped to the viewport).
//!
//! # Spec
//!
//! Inputs: anchor rect, floating size, viewport size, preferred `Side`,
//! `Align`, offset (px, gap between anchor and floating element).
//!
//! Algorithm (pure, tested):
//! 1. Primary coordinate from `Side` at `offset` from the anchor edge.
//! 2. Cross coordinate from `Align`: `Start` flush with the anchor's
//!    start edge, `Center` centered on the anchor, `End` flush with its
//!    end edge.
//! 3. **Flip**: if the floating element overflows the viewport on the
//!    primary axis and the opposite side fits, recompute on the opposite
//!    side. If neither side fits, keep the preferred side.
//! 4. **Shift**: clamp the cross-axis coordinate into `[0, viewport - size]`.
//!
//! The returned `side` is the side actually used (consumers set
//! `data-side`-equivalent styling from it).
//!
//! Keyboard map: none (geometry primitive; interaction lives in the overlay
//! that uses it).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    fn opposite(self) -> Side {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub side: Side,
}

fn place(anchor: Rect, size: (f64, f64), side: Side, align: Align, offset: f64) -> (f64, f64) {
    let (w, h) = size;
    let cross_x = match align {
        Align::Start => anchor.x,
        Align::Center => anchor.x + (anchor.width - w) / 2.0,
        Align::End => anchor.x + anchor.width - w,
    };
    let cross_y = match align {
        Align::Start => anchor.y,
        Align::Center => anchor.y + (anchor.height - h) / 2.0,
        Align::End => anchor.y + anchor.height - h,
    };
    match side {
        Side::Top => (cross_x, anchor.y - h - offset),
        Side::Bottom => (cross_x, anchor.y + anchor.height + offset),
        Side::Left => (anchor.x - w - offset, cross_y),
        Side::Right => (anchor.x + anchor.width + offset, cross_y),
    }
}

fn overflows_primary(pos: (f64, f64), size: (f64, f64), viewport: (f64, f64), side: Side) -> bool {
    let (x, y) = pos;
    let (w, h) = size;
    match side {
        Side::Top => y < 0.0,
        Side::Bottom => y + h > viewport.1,
        Side::Left => x < 0.0,
        Side::Right => x + w > viewport.0,
    }
}

/// The tested entry point: offset → flip → shift (no collision padding).
pub fn compute(
    anchor: Rect,
    size: (f64, f64),
    viewport: (f64, f64),
    side: Side,
    align: Align,
    offset: f64,
) -> Placement {
    compute_padded(anchor, size, viewport, side, align, offset, 0.0)
}

/// [`compute`] with a collision padding: the shift clamp keeps the floating
/// element `padding` px inside the viewport (Radix `collisionPadding` —
/// Select uses 10, the other poppers 0).
pub fn compute_padded(
    anchor: Rect,
    size: (f64, f64),
    viewport: (f64, f64),
    side: Side,
    align: Align,
    offset: f64,
    padding: f64,
) -> Placement {
    let preferred = place(anchor, size, side, align, offset);
    let (mut pos, mut used) = (preferred, side);
    if overflows_primary(preferred, size, viewport, side) {
        let flipped = place(anchor, size, side.opposite(), align, offset);
        if !overflows_primary(flipped, size, viewport, side.opposite()) {
            pos = flipped;
            used = side.opposite();
        }
    }
    // Shift: clamp the cross axis into the padded viewport.
    let (x, y) = pos;
    let clamped = match used {
        Side::Top | Side::Bottom => {
            (x.clamp(padding, (viewport.0 - size.0 - padding).max(padding)), y)
        }
        Side::Left | Side::Right => {
            (x, y.clamp(padding, (viewport.1 - size.1 - padding).max(padding)))
        }
    };
    Placement { x: clamped.0, y: clamped.1, side: used }
}

/// Clamp a floating element's measured size so it cannot exceed the
/// padded viewport (Radix size middleware / collision-box bound).
pub fn clamp_size_to_viewport(size: (f64, f64), viewport: (f64, f64), padding: f64) -> (f64, f64) {
    let max_w = (viewport.0 - 2.0 * padding).max(0.0);
    let max_h = (viewport.1 - 2.0 * padding).max(0.0);
    (size.0.min(max_w), size.1.min(max_h))
}

/// Remaining vertical space below a placed floating `y`, for `max-height`.
/// Floors at 80px so a near-bottom placement still yields a usable scrollport.
pub fn max_height_for_y(y: f64, viewport_h: f64, padding: f64) -> f64 {
    (viewport_h - y - padding).max(80.0)
}

/// DOM adapter: an element's viewport-relative rect.
pub fn rect_of(element: &web_sys::Element) -> Rect {
    let r = element.get_bounding_client_rect();
    Rect { x: r.x(), y: r.y(), width: r.width(), height: r.height() }
}

/// DOM adapter: apply a placement as fixed positioning.
pub fn apply(floating: &web_sys::HtmlElement, placement: Placement) {
    let style = floating.style();
    let _ = style.set_property("position", "fixed");
    let _ = style.set_property("left", &format!("{}px", placement.x));
    let _ = style.set_property("top", &format!("{}px", placement.y));
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANCHOR: Rect = Rect { x: 100.0, y: 100.0, width: 50.0, height: 20.0 };
    const VIEWPORT: (f64, f64) = (800.0, 600.0);

    #[test]
    fn bottom_center_positions_below_and_centered() {
        let p = compute(ANCHOR, (150.0, 40.0), VIEWPORT, Side::Bottom, Align::Center, 6.0);
        assert_eq!(p.side, Side::Bottom);
        assert_eq!(p.y, 126.0); // 100 + 20 + 6
        assert_eq!(p.x, 50.0); // 100 + (50-150)/2
    }

    #[test]
    fn start_and_end_align_flush_with_anchor_edges() {
        let start = compute(ANCHOR, (150.0, 40.0), VIEWPORT, Side::Bottom, Align::Start, 0.0);
        assert_eq!(start.x, 100.0);
        let end = compute(ANCHOR, (150.0, 40.0), VIEWPORT, Side::Bottom, Align::End, 0.0);
        assert_eq!(end.x, 0.0); // 100 + 50 - 150
    }

    #[test]
    fn top_flips_to_bottom_when_no_space_above() {
        let near_top = Rect { x: 100.0, y: 10.0, width: 50.0, height: 20.0 };
        let p = compute(near_top, (100.0, 40.0), VIEWPORT, Side::Top, Align::Start, 6.0);
        assert_eq!(p.side, Side::Bottom);
        assert_eq!(p.y, 36.0); // 10 + 20 + 6
    }

    #[test]
    fn no_flip_when_neither_side_fits() {
        let anchor = Rect { x: 100.0, y: 290.0, width: 50.0, height: 20.0 };
        let p = compute(anchor, (100.0, 500.0), VIEWPORT, Side::Top, Align::Start, 0.0);
        assert_eq!(p.side, Side::Top); // keeps preferred
    }

    #[test]
    fn shift_clamps_cross_axis_to_viewport() {
        let left_edge = Rect { x: 5.0, y: 100.0, width: 20.0, height: 20.0 };
        let p = compute(left_edge, (200.0, 40.0), VIEWPORT, Side::Bottom, Align::Center, 0.0);
        assert_eq!(p.x, 0.0);
        let right_edge = Rect { x: 750.0, y: 100.0, width: 40.0, height: 20.0 };
        let p2 = compute(right_edge, (200.0, 40.0), VIEWPORT, Side::Bottom, Align::Center, 0.0);
        assert_eq!(p2.x, 600.0); // 800 - 200
    }

    #[test]
    fn collision_padding_clamps_inside_the_padded_viewport() {
        // Anchor flush with the viewport's left edge: align-start wants
        // x = 0; a 10px padding clamps to 10 (the Radix Select case).
        let anchor = Rect { x: 0.0, y: 0.0, width: 192.0, height: 32.0 };
        let p = compute_padded(anchor, (128.0, 240.0), (1440.0, 900.0), Side::Bottom, Align::Start, 0.0, 10.0);
        assert_eq!(p.side, Side::Bottom);
        assert_eq!((p.x, p.y), (10.0, 32.0));
        // Zero padding keeps the old clamp.
        let p0 = compute(anchor, (128.0, 240.0), (1440.0, 900.0), Side::Bottom, Align::Start, 0.0);
        assert_eq!((p0.x, p0.y), (0.0, 32.0));
    }

    #[test]
    fn left_and_right_sides_place_on_the_x_axis() {
        let p = compute(ANCHOR, (60.0, 30.0), VIEWPORT, Side::Right, Align::Start, 8.0);
        assert_eq!(p.side, Side::Right);
        assert_eq!(p.x, 158.0); // 100 + 50 + 8
        assert_eq!(p.y, 100.0);
        let p2 = compute(ANCHOR, (60.0, 30.0), VIEWPORT, Side::Left, Align::End, 8.0);
        assert_eq!(p2.x, 32.0); // 100 - 60 - 8
        assert_eq!(p2.y, 90.0); // 100 + 20 - 30
    }
}
