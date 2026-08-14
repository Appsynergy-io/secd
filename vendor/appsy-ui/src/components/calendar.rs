//! Calendar — port of `components/ui/calendar.tsx` (react-day-picker v10
//! wrapped with the site's utility classNames and Remix chevrons; rdp ships
//! no stylesheet here, so its `rdp-*` classes are vocabulary only). Machine
//! (rdp v10, `mode="single" | "range"`, `showOutsideDays`):
//!
//! ```text
//! Single: click day --> selected = day; click selected again --> None
//! Range:  click 1 --> from = to = day
//!         click 2 --> extends (earlier click becomes `from`)
//!         click on a complete range --> new from = to = day
//! Nav: prev/next buttons shift the displayed month
//! Keyboard (on a day button): ArrowLeft/Right ±1 day, ArrowUp/Down ±1 week,
//! Home/End week bounds, PageUp/Down ±1 month, Shift+PageUp/Down ±1 year —
//! focus follows, the grid re-renders when the month changes;
//! Enter/Space select (native button activation)
//! ```
//!
//! One day button holds `tabindex="0"`: the selection start if visible,
//! else today if visible, else the first day of the month. `aria-selected`
//! lives on the gridcell `<td>`, exactly as rdp stamps it (the reference's
//! `aria-selected:` button utilities never match and are reproduced by
//! omission). Determinism: the crate never reads the clock — `today` is a
//! prop (rdp exposes the same prop; the consumer owns the clock).

use leptos::prelude::*;

use crate::icons::{Icon, RI_ARROW_LEFT_S_LINE, RI_ARROW_RIGHT_S_LINE};

pub const CAL: &str = "asy-cal";
pub const CAL_MONTHS: &str = "asy-cal__months";
pub const CAL_MONTH: &str = "asy-cal__month";
pub const CAL_CAPTION: &str = "asy-cal__caption";
pub const CAL_CAPTION_LABEL: &str = "asy-cal__caption-label";
pub const CAL_NAV: &str = "asy-cal__nav";
pub const CAL_NAV_BTN: &str = "asy-cal__nav-btn";
pub const CAL_CHEVRON: &str = "asy-cal__chevron";
pub const CAL_GRID: &str = "asy-cal__grid";
pub const CAL_WEEKDAYS: &str = "asy-cal__weekdays";
pub const CAL_WEEKDAY: &str = "asy-cal__weekday";
pub const CAL_WEEK: &str = "asy-cal__week";
pub const CAL_DAY: &str = "asy-cal__day";
pub const CAL_DAY_TODAY: &str = "asy-cal__day--today";
pub const CAL_DAY_OUTSIDE: &str = "asy-cal__day--outside";
pub const CAL_DAY_BTN: &str = "asy-cal__day-btn";

const WEEKDAY_SHORT: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
const WEEKDAY_LONG: [&str; 7] =
    ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];

/// A civil date. Ordinary calendar arithmetic, no clock access anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalendarDate {
    pub year: i32,
    /// 1–12.
    pub month: u32,
    /// 1–31.
    pub day: u32,
}

impl CalendarDate {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Days since 1970-01-01 (negative before), Howard Hinnant's civil
    /// algorithm.
    pub fn to_days(self) -> i64 {
        let y = i64::from(if self.month <= 2 { self.year - 1 } else { self.year });
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = i64::from(if self.month > 2 { self.month - 3 } else { self.month + 9 });
        let doy = (153 * mp + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    /// Inverse of [`to_days`](Self::to_days).
    pub fn from_days(days: i64) -> Self {
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        Self { year: (if m <= 2 { y + 1 } else { y }) as i32, month: m, day: d }
    }

    /// 0 = Sunday … 6 = Saturday.
    pub fn weekday(self) -> u32 {
        (self.to_days() + 4).rem_euclid(7) as u32
    }

    pub fn plus_days(self, n: i64) -> Self {
        Self::from_days(self.to_days() + n)
    }

    /// Same day `n` months later (negative allowed), clamped to the target
    /// month's length — rdp's PageUp/PageDown behavior.
    pub fn plus_months(self, n: i32) -> Self {
        let total = self.year * 12 + self.month as i32 - 1 + n;
        let year = total.div_euclid(12);
        let month = (total.rem_euclid(12) + 1) as u32;
        let day = self.day.min(days_in_month(year, month));
        Self { year, month, day }
    }

    /// `YYYY-MM-DD`, rdp's `data-day` format.
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// "1st" / "2nd" / "3rd" / "4th" … (11th–13th are `th`).
fn ordinal(day: u32) -> String {
    let suffix = match (day % 10, day % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{day}{suffix}")
}

/// rdp's day button label: `"Thursday, August 6th, 2026"`, prefixed
/// `"Today, "` and suffixed `", selected"` as applicable.
fn day_label(date: CalendarDate, is_today: bool, is_selected: bool) -> String {
    let mut label = String::new();
    if is_today {
        label.push_str("Today, ");
    }
    label.push_str(WEEKDAY_LONG[date.weekday() as usize]);
    label.push_str(", ");
    label.push_str(MONTHS[(date.month - 1) as usize]);
    label.push(' ');
    label.push_str(&ordinal(date.day));
    label.push_str(", ");
    label.push_str(&date.year.to_string());
    if is_selected {
        label.push_str(", selected");
    }
    label
}

fn caption(year: i32, month: u32) -> String {
    format!("{} {year}", MONTHS[(month - 1) as usize])
}

/// The Sunday-started grid for a month: every visible day, in as many
/// whole weeks as the month spans (no fixed-week padding).
pub fn month_grid(year: i32, month: u32) -> Vec<Vec<CalendarDate>> {
    let first = CalendarDate::new(year, month, 1);
    let lead = first.weekday();
    let start = first.plus_days(-i64::from(lead));
    let weeks = (lead + days_in_month(year, month)).div_ceil(7);
    (0..weeks)
        .map(|w| (0..7).map(|d| start.plus_days(i64::from(w * 7 + d))).collect())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalendarMode {
    #[default]
    Single,
    Range,
}

/// An inclusive range; a first click yields `from == to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarRange {
    pub from: CalendarDate,
    pub to: CalendarDate,
}

#[component]
pub fn Calendar(
    /// The consumer's clock — the crate never reads one (rdp's `today`
    /// prop; determinism invariant).
    today: CalendarDate,
    #[prop(optional)] mode: CalendarMode,
    #[prop(optional, into)] selected: Signal<Option<CalendarDate>>,
    #[prop(optional, into)] selected_range: Signal<Option<CalendarRange>>,
    #[prop(optional)] on_select: Option<Callback<Option<CalendarDate>>>,
    #[prop(optional)] on_select_range: Option<Callback<Option<CalendarRange>>>,
    /// Initial displayed month; defaults to the selection start, else today.
    #[prop(optional)] default_month: Option<(i32, u32)>,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let initial = default_month
        .or_else(|| {
            let sel = match mode {
                CalendarMode::Single => selected.get_untracked(),
                CalendarMode::Range => selected_range.get_untracked().map(|r| r.from),
            };
            sel.map(|d| (d.year, d.month))
        })
        .unwrap_or((today.year, today.month));
    let month = RwSignal::new(initial);
    // `focused`: the day whose button holds DOM focus (rdp's `focused`
    // modifier). `last_focused`: rdp's focus memory, written by the day
    // button's blur handler from whatever `focused` holds at that moment —
    // event ordering makes a click leave the *previous* day here while a
    // keyboard move (which updates `focused` before refocusing) leaves the
    // *new* day.
    let focused: RwSignal<Option<CalendarDate>> = RwSignal::new(None);
    let last_focused: RwSignal<Option<CalendarDate>> = RwSignal::new(None);
    // React's day-keyed lists silently detach the focused button on a month
    // change (no focusout reaches its root listener), so its focus memory
    // never records the transition. Leptos morphs the grid in place — the
    // old button survives as a different day and *does* blur when the rAF
    // refocus lands — so that one blur is suppressed to keep the streams
    // identical.
    let suppress_blur: StoredValue<bool> = StoredValue::new(false);

    let mut root_class = CAL.to_owned();
    if let Some(extra) = class {
        root_class.push(' ');
        root_class.push_str(&extra);
    }
    let mode_attr = match mode {
        CalendarMode::Single => "single",
        CalendarMode::Range => "range",
    };

    // Selection and focus changes land as attribute flips on stable day
    // nodes — recreating the clicked button would invalidate the browser's
    // click-set sequential-focus starting point (React keeps the node; so
    // must we).
    let select_day = move |day: CalendarDate| {
        match mode {
            CalendarMode::Single => {
                if let Some(cb) = on_select {
                    let next = if selected.get_untracked() == Some(day) { None } else { Some(day) };
                    cb.run(next);
                }
            }
            CalendarMode::Range => {
                if let Some(cb) = on_select_range {
                    let next = match selected_range.get_untracked() {
                        None => CalendarRange { from: day, to: day },
                        Some(r) if r.from == r.to => {
                            if day < r.from {
                                CalendarRange { from: day, to: r.from }
                            } else {
                                CalendarRange { from: r.from, to: day }
                            }
                        }
                        Some(_) => CalendarRange { from: day, to: day },
                    };
                    cb.run(Some(next));
                }
            }
        }
    };

    // The single tab stop — rdp's `calculateFocusTarget`: last-focused day,
    // else the first selected day in grid order, else today, else the first
    // focusable day; outside days are never focusable.
    let tab_target = Memo::new(move |_| {
        let (y, m) = month.get();
        let grid = month_grid(y, m);
        let focusable =
            |d: CalendarDate| (d.year, d.month) == (y, m) && grid.iter().flatten().any(|g| *g == d);
        let first_selected = match mode {
            CalendarMode::Single => selected.get().filter(|d| focusable(*d)),
            CalendarMode::Range => selected_range.get().and_then(|r| {
                grid.iter().flatten().copied().find(|d| {
                    *d >= r.from && *d <= r.to && focusable(*d)
                })
            }),
        };
        last_focused
            .get()
            .filter(|d| focusable(*d))
            .or(first_selected)
            .or_else(|| Some(today).filter(|d| focusable(*d)))
            .unwrap_or(CalendarDate::new(y, m, 1))
    });

    // Keyboard focus movement; changing month re-renders the grid, then the
    // target button (present by construction) takes focus.
    let move_focus = move |target: CalendarDate| {
        let (y, m) = month.get_untracked();
        let month_changed = (target.year, target.month) != (y, m);
        if month_changed {
            month.set((target.year, target.month));
            suppress_blur.set_value(true);
        }
        focused.set(Some(target));
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        {
            let iso = target.iso();
            request_animation_frame(move || {
                let document = leptos::tachys::dom::document();
                if let Ok(Some(btn)) =
                    document.query_selector(&format!("[data-day=\"{iso}\"] button"))
                {
                    use wasm_bindgen::JsCast;
                    if let Ok(btn) = btn.dyn_into::<web_sys::HtmlElement>() {
                        let _ = btn.focus();
                    }
                }
                suppress_blur.set_value(false);
            });
        }
    };

    // Month-keyed table (the <For> below): a month change replaces the whole
    // grid (React's day-keyed lists drop the nodes too), so the previously
    // focused button detaches silently instead of morphing in place and
    // emitting a spurious blur. Within a month, selection and focus land as
    // reactive attrs on stable nodes. The keyed boundary sits outside the
    // table because hydration markers inside <tbody> get foster-parented by
    // the HTML parser.
    view! {
        <div class=root_class lang="en-US" data-mode=mode_attr>
            <div class=CAL_MONTHS>
                <nav class=CAL_NAV aria-label="Navigation bar">
                    <button
                        type="button"
                        class=CAL_NAV_BTN
                        aria-label="Go to the Previous Month"
                        on:click=move |_| month.update(|m| {
                            let prev = CalendarDate::new(m.0, m.1, 1).plus_months(-1);
                            *m = (prev.year, prev.month);
                        })
                    >
                        <Icon d=RI_ARROW_LEFT_S_LINE class=CAL_CHEVRON />
                    </button>
                    <button
                        type="button"
                        class=CAL_NAV_BTN
                        aria-label="Go to the Next Month"
                        on:click=move |_| month.update(|m| {
                            let next = CalendarDate::new(m.0, m.1, 1).plus_months(1);
                            *m = (next.year, next.month);
                        })
                    >
                        <Icon d=RI_ARROW_RIGHT_S_LINE class=CAL_CHEVRON />
                    </button>
                </nav>
                <div class=CAL_MONTH>
                    <div class=CAL_CAPTION>
                        <span class=CAL_CAPTION_LABEL role="status" aria-live="polite">
                            {move || {
                                let (y, m) = month.get();
                                caption(y, m)
                            }}
                        </span>
                    </div>
                    <table
                        role="grid"
                        aria-multiselectable=match mode {
                            CalendarMode::Single => "false",
                            CalendarMode::Range => "true",
                        }
                        class=CAL_GRID
                        aria-label=move || {
                            let (y, m) = month.get();
                            caption(y, m)
                        }
                    >
                        <thead aria-hidden="true">
                            <tr class=CAL_WEEKDAYS>
                                {(0..7)
                                    .map(|i| {
                                        view! {
                                            <th
                                                aria-label=WEEKDAY_LONG[i]
                                                class=CAL_WEEKDAY
                                                scope="col"
                                            >
                                                {WEEKDAY_SHORT[i]}
                                            </th>
                                        }
                                    })
                                    .collect_view()}
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                let (y, m) = month.get();
                                month_grid(y, m)
                                    .into_iter()
                                    .map(|week| {
                                        let cells = week
                                            .into_iter()
                                            .map(|day| {
                                                day_cell(DayCell {
                                                    day,
                                                    displayed: (y, m),
                                                    today,
                                                    mode,
                                                    selected,
                                                    selected_range,
                                                    focused,
                                                    last_focused,
                                                    suppress_blur,
                                                    tab_target,
                                                    select_day,
                                                    move_focus,
                                                })
                                            })
                                            .collect_view();
                                        view! { <tr class=CAL_WEEK>{cells}</tr> }
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

struct DayCell<S, F> {
    day: CalendarDate,
    displayed: (i32, u32),
    today: CalendarDate,
    mode: CalendarMode,
    selected: Signal<Option<CalendarDate>>,
    selected_range: Signal<Option<CalendarRange>>,
    focused: RwSignal<Option<CalendarDate>>,
    last_focused: RwSignal<Option<CalendarDate>>,
    suppress_blur: StoredValue<bool>,
    tab_target: Memo<CalendarDate>,
    select_day: S,
    move_focus: F,
}

fn day_cell<S, F>(cell: DayCell<S, F>) -> impl IntoView
where
    S: Fn(CalendarDate) + Copy + 'static,
    F: Fn(CalendarDate) + Copy + 'static,
{
    let DayCell {
        day,
        displayed,
        today,
        mode,
        selected,
        selected_range,
        focused,
        last_focused,
        suppress_blur,
        tab_target,
        select_day,
        move_focus,
    } = cell;
    let outside = (day.year, day.month) != displayed;
    let is_today = day == today;
    // Range membership marks `aria-selected`; rdp's `rdp-range_*` classes
    // carry no styles upstream (no rdp stylesheet ships), so position
    // classes are reproduced by omission.
    let is_selected = move || match mode {
        CalendarMode::Single => selected.get() == Some(day),
        CalendarMode::Range => {
            selected_range.get().is_some_and(|r| day >= r.from && day <= r.to)
        }
    };
    let mut cls = CAL_DAY.to_owned();
    if is_today {
        cls.push(' ');
        cls.push_str(CAL_DAY_TODAY);
    }
    if outside {
        cls.push(' ');
        cls.push_str(CAL_DAY_OUTSIDE);
    }
    let keydown = move |ev: web_sys::KeyboardEvent| {
        let shift = ev.shift_key();
        let target = match ev.key().as_str() {
            "ArrowLeft" => {
                Some(if shift { day.plus_months(-1) } else { day.plus_days(-1) })
            }
            "ArrowRight" => {
                Some(if shift { day.plus_months(1) } else { day.plus_days(1) })
            }
            "ArrowUp" => {
                Some(if shift { day.plus_months(-12) } else { day.plus_days(-7) })
            }
            "ArrowDown" => {
                Some(if shift { day.plus_months(12) } else { day.plus_days(7) })
            }
            "Home" => Some(day.plus_days(-i64::from(day.weekday()))),
            "End" => Some(day.plus_days(i64::from(6 - day.weekday()))),
            "PageUp" => Some(day.plus_months(if shift { -12 } else { -1 })),
            "PageDown" => Some(day.plus_months(if shift { 12 } else { 1 })),
            _ => None,
        };
        if let Some(target) = target {
            ev.prevent_default();
            move_focus(target);
        }
    };
    view! {
        <td
            class=cls
            role="gridcell"
            aria-selected=move || is_selected().then_some("true")
            data-day=day.iso()
            data-month=outside.then(|| format!("{:04}-{:02}", day.year, day.month))
            data-outside=outside.then_some("true")
            data-selected=move || is_selected().then_some("true")
            data-today=is_today.then_some("true")
            data-focused=move || (focused.get() == Some(day)).then_some("true")
        >
            <button
                class=CAL_DAY_BTN
                type="button"
                tabindex=move || if day == tab_target.get() { "0" } else { "-1" }
                aria-label=move || day_label(day, is_today, is_selected())
                on:click=move |_| select_day(day)
                on:keydown=keydown
                on:focus=move |_| focused.set(Some(day))
                on:blur=move |ev: web_sys::FocusEvent| {
                    // rdp's blur(): the memory takes whatever `focused`
                    // holds right now, then focus state clears. Blur fired
                    // for a *detached* button (month-change rebuild) must be
                    // ignored — React's root-delegated focusout never sees
                    // it, so neither may we.
                    use wasm_bindgen::JsCast;
                    let detached = ev
                        .target()
                        .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                        .is_some_and(|n| !n.is_connected());
                    if detached || suppress_blur.get_value() {
                        return;
                    }
                    last_focused.set(focused.get_untracked());
                    focused.set(None);
                }
            >
                {day.day.to_string()}
            </button>
        </td>
    }
}

/// Root `p-3 text-sm` plus the site's classNames translated; rdp's own
/// stylesheet is absent upstream, so only the utility classes style
/// anything.
pub fn css() -> String {
    format!(
        ".{CAL}{{padding:.75rem;font-size:.875rem;line-height:calc(1.25/.875)}}\
.{CAL_MONTHS}{{display:flex;flex-direction:column;gap:1rem;\
max-width:100%;overflow-x:auto}}\
@media (min-width:640px){{.{CAL_MONTHS}{{flex-direction:row}}}}\
.{CAL_MONTH}{{display:flex;flex-direction:column;gap:.75rem}}\
.{CAL_CAPTION}{{display:flex;justify-content:center;padding-top:.25rem;\
font-size:.875rem;line-height:calc(1.25/.875);font-weight:500}}\
.{CAL_CAPTION_LABEL}{{display:block}}\
.{CAL_NAV}{{display:flex;align-items:center;justify-content:space-between}}\
.{CAL_NAV_BTN}{{width:1.75rem;height:1.75rem;display:inline-flex;\
align-items:center;justify-content:center;border-radius:var(--radius-sm);\
color:var(--color-text-muted)}}\
@media (hover:hover){{.{CAL_NAV_BTN}:hover{{\
background-color:var(--color-surface-2);color:var(--color-text)}}}}\
.{CAL_CHEVRON}{{width:1rem;height:1rem}}\
.{CAL_GRID}{{text-indent:0;border-color:inherit;border-collapse:collapse}}\
.{CAL_WEEKDAYS}{{display:flex}}\
.{CAL_WEEKDAY}{{color:var(--color-text-dim);width:2.25rem;\
text-align:center;font-size:11.5px;font-weight:500;\
text-transform:uppercase;letter-spacing:.04em}}\
.{CAL_WEEK}{{display:flex;width:100%;margin-top:.375rem}}\
.{CAL_DAY}{{width:2.25rem;height:2.25rem;padding:0;text-align:center}}\
.{CAL_DAY_TODAY}{{color:var(--color-accent);font-weight:600}}\
.{CAL_DAY_OUTSIDE}{{color:var(--color-text-dim);opacity:.5}}\
.{CAL_DAY_BTN}{{width:2.25rem;height:2.25rem;display:inline-flex;\
align-items:center;justify-content:center;\
border-radius:var(--radius-sm);font-size:.875rem;\
line-height:calc(1.25/.875)}}\
@media (hover:hover){{.{CAL_DAY_BTN}:hover{{\
background-color:var(--color-surface-2)}}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip_and_weekday() {
        let d = CalendarDate::new(2026, 8, 6);
        assert_eq!(d.to_days(), 20671);
        assert_eq!(CalendarDate::from_days(20671), d);
        assert_eq!(d.weekday(), 4); // Thursday
        assert_eq!(CalendarDate::new(1970, 1, 1).weekday(), 4);
        assert_eq!(CalendarDate::new(2026, 8, 1).weekday(), 6); // Saturday
    }

    #[test]
    fn grids_span_only_needed_weeks() {
        let aug = month_grid(2026, 8);
        assert_eq!(aug.len(), 6);
        assert_eq!(aug[0][0], CalendarDate::new(2026, 7, 26));
        assert_eq!(aug[5][6], CalendarDate::new(2026, 9, 5));
        let sep = month_grid(2026, 9);
        assert_eq!(sep.len(), 5);
        let feb = month_grid(2026, 2); // Feb 2026 starts Sunday, 28 days
        assert_eq!(feb.len(), 4);
    }

    #[test]
    fn plus_months_clamps() {
        assert_eq!(
            CalendarDate::new(2026, 1, 31).plus_months(1),
            CalendarDate::new(2026, 2, 28)
        );
        assert_eq!(
            CalendarDate::new(2026, 8, 6).plus_months(-12),
            CalendarDate::new(2025, 8, 6)
        );
    }

    #[test]
    fn labels_match_rdp() {
        assert_eq!(
            day_label(CalendarDate::new(2026, 8, 6), true, true),
            "Today, Thursday, August 6th, 2026, selected"
        );
        assert_eq!(
            day_label(CalendarDate::new(2026, 7, 26), false, false),
            "Sunday, July 26th, 2026"
        );
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(13), "13th");
        assert_eq!(ordinal(21), "21st");
        assert_eq!(ordinal(22), "22nd");
        assert_eq!(caption(2026, 8), "August 2026");
    }

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            CAL,
            CAL_MONTHS,
            CAL_MONTH,
            CAL_CAPTION,
            CAL_CAPTION_LABEL,
            CAL_NAV,
            CAL_NAV_BTN,
            CAL_CHEVRON,
            CAL_GRID,
            CAL_WEEKDAYS,
            CAL_WEEKDAY,
            CAL_WEEK,
            CAL_DAY,
            CAL_DAY_TODAY,
            CAL_DAY_OUTSIDE,
            CAL_DAY_BTN,
        ] {
            assert!(css.contains(&format!(".{class}")), "no rule for .{class}");
        }
    }
}
