use std::io::{self, stdout, IsTerminal};
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as TermEvent, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;

use crate::login;
use crate::policy::Schema;

mod form;
mod model;
mod signin;
mod theme;
mod tree;
mod view;

pub use form::{FieldTag, Form, FormField, Input};
pub use model::{DetailRow, Event, Modal, Mode, Model, Picker, KEYS};
pub use signin::SignIn;
pub use theme::{color, ACCENT, DANGER, DIM, HIGHLIGHT, OK, ON_ACCENT, WARN};
pub use tree::Row;
pub use view::{
    action_bar, detail_cols, field_rects, form_cols, hit_key, inset, mask, modal_box,
    modal_buttons, regions, reveal_rect, rows_at, spot_at, spot_text, toggle_text, window_start,
    Action, ActionHit, DetailCols, FormCols, Painted, Rect, Regions, Spot, SpotHit,
};

/// `secd` with no args: sign-in wait, then the register.
pub fn run() -> anyhow::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("secd: need a terminal");
    }

    // The session and its DEK outlive the terminal that made them, so a second
    // `secd` reuses them instead of asking a human to approve a device again.
    // Probed before raw mode, so a dead token still reaches the sign-in below.
    let resumed = login::resume();
    let mut signin = match resumed {
        Some(_) => None,
        None => {
            let flow = login::start().context("device start")?;
            login::open_browser(&flow.open_url);
            Some(SignIn::from_flow(flow))
        }
    };

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;
    let _restore = Restore;

    let unlocked = match resumed {
        Some(unlocked) => unlocked,
        None => {
            let signin = signin.as_mut().expect("invariant: no resume means a flow");
            loop {
                terminal.draw(|f| signin::draw(f, signin))?;
                if event::poll(Duration::from_millis(250))? {
                    signin.handle(map_term(event::read()?));
                } else {
                    signin.handle(Event::Tick);
                }
                if signin.quit {
                    return Ok(());
                }
                if let Some(unlocked) = signin.take_unlocked() {
                    break unlocked;
                }
            }
        }
    };

    let mut model = Model::from_unlocked(unlocked);
    // Paint before the first round trip, not after it.
    terminal.draw(|f| {
        draw(f, &model);
    })?;
    model.load();
    loop {
        terminal.draw(|f| {
            let area = Rect::from(f.area());
            let r = regions(area);
            let bar = view::action_bar_row(area, &r);
            model.set_hits(action_bar(bar, model.actions()));
            let painted = draw(f, &model);
            model.set_painted(&painted);
        })?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        model.handle(map_term(event::read()?));
        if model.quit() {
            break;
        }
    }
    Ok(())
}

fn map_term(ev: TermEvent) -> Event {
    match ev {
        TermEvent::Key(k) if k.kind != KeyEventKind::Release => map_key(k),
        TermEvent::Mouse(m) => match m.kind {
            MouseEventKind::Down(MouseButton::Left) => Event::Click {
                column: m.column,
                row: m.row,
            },
            // The obvious thing in every mode, and no new state to hold.
            MouseEventKind::ScrollUp => Event::Up,
            MouseEventKind::ScrollDown => Event::Down,
            _ => Event::Tick,
        },
        TermEvent::Resize(_, _) => Event::Resize,
        _ => Event::Tick,
    }
}

fn map_key(k: KeyEvent) -> Event {
    // Every Ctrl combo is swallowed here. Falling through would make Ctrl-A
    // open the add modal and Ctrl-D the delete one.
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        return match k.code {
            KeyCode::Char('c') => Event::Quit,
            KeyCode::Char('r' | 'R') => Event::Reveal,
            _ => Event::Tick,
        };
    }
    match k.code {
        KeyCode::Esc => Event::Esc,
        KeyCode::Enter => Event::Enter,
        KeyCode::Up => Event::Up,
        KeyCode::Down => Event::Down,
        KeyCode::Left => Event::Left,
        KeyCode::Right => Event::Right,
        KeyCode::Home => Event::Home,
        KeyCode::End => Event::End,
        KeyCode::PageUp => Event::PageUp,
        KeyCode::PageDown => Event::PageDown,
        KeyCode::Backspace => Event::Backspace,
        KeyCode::Delete => Event::Delete,
        KeyCode::BackTab => Event::BackTab,
        // Some terminals send Shift-Tab as Tab with the modifier.
        KeyCode::Tab if k.modifiers.contains(KeyModifiers::SHIFT) => Event::BackTab,
        KeyCode::Tab => Event::Tab,
        KeyCode::Char(c) => Event::Key(c),
        _ => Event::Tick,
    }
}

struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut out = stdout();
        let _ = execute!(out, LeaveAlternateScreen, DisableMouseCapture);
        login::clipboard_clear();
    }
}

/// Paint the register and return everything a click can land on. The hit table
/// is built by the same code that draws, so the two cannot disagree.
pub fn draw(frame: &mut Frame, model: &Model) -> Painted {
    let area = Rect::from(frame.area());
    let r = regions(area);
    draw_header(frame, r.header, model);
    let (rows, list) = draw_list(frame, r.list, model);
    draw_detail(frame, r.detail, model);
    draw_activity(frame, r.activity, model);
    let bar = view::action_bar_row(area, &r);
    draw_action_bar(frame, bar, model.actions());
    let mut painted = Painted {
        spots: rows,
        list,
        modal: 0,
    };
    if model.helping() {
        // The overlay covers the register, so nothing under it can be clicked.
        painted.spots = draw_help(frame, area);
        return painted;
    }
    match model.mode() {
        Mode::Idle => {}
        Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => {
            let (spots, start) = draw_form(frame, area, form, model.modal_window());
            painted.spots = spots;
            painted.modal = start;
        }
        Mode::Modal(Modal::Pick { pick }) => {
            let (spots, start) =
                draw_picker(frame, area, model.schemas(), pick, model.modal_window());
            painted.spots = spots;
            painted.modal = start;
        }
        Mode::Modal(Modal::Delete { label, names }) => {
            painted.spots = draw_confirm(frame, area, label, names.len());
        }
    }
    painted
}

/// Every key, from the one table the action bar is built from. Three sources
/// that can disagree is three chances to tell the human something untrue.
fn draw_help(frame: &mut Frame, area: Rect) -> Vec<SpotHit> {
    let keys = model::KEYS;
    let (box_r, list) = modal_frame(frame, area, keys.len(), "keys");
    let w = keys.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    if list.width > 0 && list.height > 0 {
        let lines: Vec<Line> = keys
            .iter()
            .map(|(k, what)| {
                Line::from(vec![
                    Span::styled(
                        format!("{} ", fit(k, w)),
                        Style::default()
                            .fg(color(ACCENT))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(*what),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), ratatui::layout::Rect::from(list));
    }
    draw_status(frame, box_r, None, "? or Esc closes");
    Vec::new()
}

pub fn draw_action_bar(frame: &mut Frame, area: Rect, actions: &[Action]) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(
        Block::default().style(Style::default().bg(color(ACCENT)).fg(color(ON_ACCENT))),
        ratatui::layout::Rect::from(area),
    );
    for (action, rect) in view::layout_actions(area, actions) {
        frame.render_widget(
            Paragraph::new(Span::styled(
                view::button_text(action),
                Style::default()
                    .fg(color(ON_ACCENT))
                    .bg(color(ACCENT))
                    .add_modifier(Modifier::BOLD),
            )),
            ratatui::layout::Rect::from(rect),
        );
    }
}

fn draw_header(frame: &mut Frame, area: Rect, model: &Model) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " secd ",
                Style::default()
                    .fg(color(ON_ACCENT))
                    .bg(color(ACCENT))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(model.title(), Style::default().fg(color(DIM))),
            // The one key that names every other key. The action bar has no
            // room for it, and a key nobody can find is a key nobody has.
            Span::styled("  ? keys", Style::default().fg(color(DIM))),
        ])),
        ratatui::layout::Rect::from(area),
    );
}

fn draw_list(frame: &mut Frame, area: Rect, model: &Model) -> (Vec<SpotHit>, usize) {
    if area.height == 0 || area.width == 0 {
        return (Vec::new(), 0);
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title(list_title(model))
        .border_style(Style::default().fg(color(DIM)));
    frame.render_widget(block, ratatui::layout::Rect::from(area));
    let inner = view::inset(area);
    if inner.height == 0 || inner.width == 0 {
        return (Vec::new(), 0);
    }
    let rows = model.rows();
    if rows.is_empty() {
        // An empty list that says nothing leaves the human guessing which key
        // fills it.
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                empty_hint(model),
                Style::default().fg(color(DIM)),
            ))),
            ratatui::layout::Rect::from(inner),
        );
        return (Vec::new(), 0);
    }
    let h = inner.height as usize;
    let sel = model.selected();
    let start = view::window_start(model.list_window(), sel, rows.len(), h);
    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .skip(start)
        .take(h)
        .map(|(i, row)| {
            let shared = row.shared(i.checked_sub(1).and_then(|j| rows.get(j)));
            ListItem::new(Line::from(row_spans(row, shared, i == sel)))
        })
        .collect();
    frame.render_widget(List::new(items), ratatui::layout::Rect::from(inner));
    (view::rows_at(inner, start, rows.len(), Spot::Row), start)
}

/// The path, with the part shared with the row above dimmed. A group reads as
/// a group without costing the keystroke a directory level would.
fn row_spans(row: &tree::Row, shared: usize, selected: bool) -> Vec<Span<'static>> {
    let leaf = if selected {
        Style::default()
            .fg(color(HIGHLIGHT))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let mut spans = Vec::new();
    if shared > 0 {
        spans.push(Span::styled(
            row.label[..shared].to_string(),
            Style::default().fg(color(DIM)),
        ));
    }
    spans.push(Span::styled(row.label[shared..].to_string(), leaf));
    if row.descends() {
        spans.push(Span::styled(
            format!("  {} {}", row.members.len(), plural(row.members.len())),
            Style::default().fg(color(DIM)),
        ));
    }
    spans
}

fn list_title(model: &Model) -> String {
    if !model.open().is_empty() {
        return format!("{} · Esc back", model.open());
    }
    let shown = model.rows().len();
    let total = model.total();
    if model.filtering() || !model.filter().is_empty() {
        return format!("/{}  {shown}/{total}", model.filter());
    }
    format!("names  {total}")
}

fn empty_hint(model: &Model) -> &'static str {
    if model.filtering() || !model.filter().is_empty() {
        "nothing matches — Esc clears the filter"
    } else {
        "no secrets yet — [a] adds one, [p] adds a provider bundle"
    }
}

fn draw_detail(frame: &mut Frame, area: Rect, model: &Model) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title("detail")
        .border_style(Style::default().fg(color(DIM)));
    frame.render_widget(block, ratatui::layout::Rect::from(area));
    let inner = view::inset(area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let rows = model.detail_rows();
    let widest = |f: fn(&DetailRow) -> &String| {
        rows.iter()
            .map(|r| u16::try_from(f(r).chars().count()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0)
    };
    // Key left, env right, and the value fed before either: it is what you
    // came to read.
    let cols = view::detail_cols(inner.width, widest(|r| &r.key), widest(|r| &r.env));
    let key_w = cols.key as usize;
    let env_w = cols.env as usize;
    let value_w = cols.value as usize;
    let mut lines: Vec<Line> = Vec::new();
    if !model.detail_title().is_empty() {
        lines.push(Line::from(Span::styled(
            model.detail_title().to_string(),
            Style::default().fg(color(DIM)),
        )));
    }
    for row in rows {
        let shown = !row.secret || model.revealed();
        let value = if shown {
            row.value.clone()
        } else {
            view::mask(row.value.chars().count())
        };
        let value_style = if shown {
            Style::default()
        } else {
            Style::default().fg(color(DIM))
        };
        if row.key.is_empty() {
            lines.push(Line::from(Span::styled(value, value_style)));
            continue;
        }
        let mut spans = vec![
            Span::styled(
                format!("{} ", fit(&row.key, key_w)),
                Style::default().fg(color(ACCENT)),
            ),
            Span::styled(fit(&value, value_w), value_style),
        ];
        if env_w > 0 {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                row.env.clone(),
                Style::default().fg(color(DIM)),
            ));
        }
        lines.push(Line::from(spans));
    }
    let h = inner.height as usize;
    if lines.len() > h {
        let more = lines.len() - (h - 1);
        lines.truncate(h - 1);
        lines.push(Line::from(Span::styled(
            format!("… {more} more"),
            Style::default().fg(color(DIM)),
        )));
    }
    frame.render_widget(Paragraph::new(lines), ratatui::layout::Rect::from(inner));
}

fn draw_activity(frame: &mut Frame, area: Rect, model: &Model) {
    if area.height == 0 {
        return;
    }
    let log_h = area.height.saturating_sub(1);
    let log = Rect::new(area.x, area.y, area.width, log_h);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("activity")
        .border_style(Style::default().fg(color(DIM)));
    let inner = block.inner(ratatui::layout::Rect::from(log));
    frame.render_widget(block, ratatui::layout::Rect::from(log));
    let notes = model.activity_lines();
    let lines: Vec<Line> = notes
        .iter()
        .rev()
        .take(inner.height as usize)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|s| Line::from(Span::styled(s.as_str(), Style::default().fg(color(DIM)))))
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The box, its title, and the interior split into a list area, a status row
/// and a button row.
fn modal_frame(frame: &mut Frame, area: Rect, rows: usize, title: &str) -> (Rect, Rect) {
    let want = u16::try_from(rows).unwrap_or(u16::MAX).saturating_add(2);
    let box_r = view::modal_box(area, want);
    frame.render_widget(Clear, ratatui::layout::Rect::from(box_r));
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_string())
            .border_style(Style::default().fg(color(ACCENT))),
        ratatui::layout::Rect::from(box_r),
    );
    let body = view::inset(box_r);
    let list = Rect::new(body.x, body.y, body.width, body.height.saturating_sub(2));
    (box_r, list)
}

fn draw_status(frame: &mut Frame, box_r: Rect, error: Option<&str>, hint: &str) {
    let body = view::inset(box_r);
    if body.height < 2 || body.width == 0 {
        return;
    }
    let y = body.y.saturating_add(body.height.saturating_sub(2));
    let (text, style) = match error {
        Some(e) => (e.to_string(), Style::default().fg(color(DANGER))),
        None => (hint.to_string(), Style::default().fg(color(DIM))),
    };
    frame.render_widget(
        Paragraph::new(Span::styled(text, style)),
        ratatui::layout::Rect::from(Rect::new(body.x, y, body.width, 1)),
    );
}

fn draw_buttons(frame: &mut Frame, box_r: Rect, commit: &str) -> Vec<SpotHit> {
    let hits = view::modal_buttons(box_r, commit);
    for hit in &hits {
        let label = match hit.spot {
            Spot::Cancel => view::spot_text("cancel"),
            _ => view::spot_text(commit),
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                label,
                Style::default()
                    .fg(color(ON_ACCENT))
                    .bg(color(ACCENT))
                    .add_modifier(Modifier::BOLD),
            )),
            ratatui::layout::Rect::from(hit.rect),
        );
    }
    hits
}

fn draw_form(frame: &mut Frame, area: Rect, form: &Form, prev: usize) -> (Vec<SpotHit>, usize) {
    let (box_r, list) = modal_frame(frame, area, form.fields.len(), &form.title);
    let mut spots = Vec::new();
    let mut window = 0;
    if list.width > 0 && list.height > 0 {
        let (label_w, env_w) = form.widths();
        let cols = view::form_cols(list.width, label_w, env_w);
        let start = view::window_start(prev, form.focus, form.fields.len(), list.height as usize);
        window = start;
        let hits = view::field_rects(list, cols, start, form.fields.len());
        // Reveal sits inside the input column, so it is tested first.
        let mut fields = Vec::with_capacity(hits.len());
        for hit in hits {
            let Spot::Field(i) = hit.spot else {
                continue;
            };
            let Some(f) = form.fields.get(i) else {
                continue;
            };
            let row = Rect::new(list.x, hit.rect.y, list.width, 1);
            let focused = i == form.focus;
            let toggle = if focused && f.secret {
                view::reveal_rect(row, cols, f.shown)
            } else {
                None
            };
            draw_form_row(frame, row, cols, f, focused, toggle);
            if let Some(rect) = toggle {
                spots.push(SpotHit {
                    spot: Spot::Reveal(i),
                    rect,
                });
            }
            fields.push(hit);
        }
        spots.extend(fields);
    }
    draw_status(
        frame,
        box_r,
        form.error.as_deref(),
        "Tab field · Ctrl-R reveal · Enter save · Esc cancel",
    );
    spots.extend(draw_buttons(frame, box_r, "save"));
    (spots, window)
}

fn draw_form_row(
    frame: &mut Frame,
    row: Rect,
    cols: FormCols,
    f: &FormField,
    focused: bool,
    toggle: Option<Rect>,
) {
    let label_style = if focused {
        Style::default()
            .fg(color(HIGHLIGHT))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color(DIM))
    };
    frame.render_widget(
        Paragraph::new(Span::styled(fit(&f.key, cols.label as usize), label_style)),
        ratatui::layout::Rect::from(Rect::new(row.x, row.y, cols.label, 1)),
    );

    let input_x = row.x.saturating_add(cols.label).saturating_add(1);
    let room = match toggle {
        Some(t) => cols.input.saturating_sub(t.width.saturating_add(1)),
        None => cols.input,
    };
    let shown = f.shown || !f.secret;
    let (text, caret) = f.input.window(room as usize, shown);
    let span = if f.input.is_empty() && !f.hint.is_empty() {
        Span::styled(fit(&f.hint, room as usize), Style::default().fg(color(DIM)))
    } else {
        Span::raw(text)
    };
    frame.render_widget(
        Paragraph::new(span),
        ratatui::layout::Rect::from(Rect::new(input_x, row.y, room, 1)),
    );
    if focused {
        frame.set_cursor_position(Position::new(
            input_x.saturating_add(u16::try_from(caret).unwrap_or(u16::MAX)),
            row.y,
        ));
    }
    if let Some(rect) = toggle {
        frame.render_widget(
            Paragraph::new(Span::styled(
                view::toggle_text(f.shown),
                Style::default().fg(color(ACCENT)),
            )),
            ratatui::layout::Rect::from(rect),
        );
    }

    let mut x = input_x.saturating_add(cols.input).saturating_add(1);
    if cols.tag > 0 {
        let style = match f.tag {
            FieldTag::Required => Style::default().fg(color(WARN)),
            _ => Style::default().fg(color(DIM)),
        };
        frame.render_widget(
            Paragraph::new(Span::styled(f.tag.label(), style)),
            ratatui::layout::Rect::from(Rect::new(x, row.y, cols.tag, 1)),
        );
        x = x.saturating_add(cols.tag).saturating_add(1);
    }
    if cols.env > 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                f.env.as_str(),
                Style::default().fg(color(DIM)),
            )),
            ratatui::layout::Rect::from(Rect::new(x, row.y, cols.env, 1)),
        );
    }
}

fn draw_picker(
    frame: &mut Frame,
    area: Rect,
    schemas: &[Schema],
    pick: &Picker,
    prev: usize,
) -> (Vec<SpotHit>, usize) {
    let (box_r, list) = modal_frame(frame, area, schemas.len(), "provider");
    let mut spots = Vec::new();
    let mut window = 0;
    if list.width > 0 && list.height > 0 {
        let h = list.height as usize;
        let start = view::window_start(prev, pick.selected, schemas.len(), h);
        window = start;
        let name_w = schemas
            .iter()
            .map(|s| s.name.chars().count())
            .max()
            .unwrap_or(0);
        let items: Vec<ListItem> = schemas
            .iter()
            .enumerate()
            .skip(start)
            .take(h)
            .map(|(i, s)| {
                let n = s.fields.len();
                let tail = if s.builtin {
                    format!("{n} {}", plural(n))
                } else {
                    format!("{n} {} · custom", plural(n))
                };
                let style = if i == pick.selected {
                    Style::default()
                        .fg(color(HIGHLIGHT))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", fit(&s.name, name_w)), style),
                    Span::styled(s.title.clone(), style),
                    Span::raw(" "),
                    Span::styled(tail, Style::default().fg(color(DIM))),
                ]))
            })
            .collect();
        frame.render_widget(List::new(items), ratatui::layout::Rect::from(list));
        spots = view::rows_at(list, start, schemas.len(), Spot::Choice);
    }
    draw_status(frame, box_r, None, "Enter choose · Esc cancel");
    spots.extend(draw_buttons(frame, box_r, "choose"));
    (spots, window)
}

fn draw_confirm(frame: &mut Frame, area: Rect, label: &str, count: usize) -> Vec<SpotHit> {
    let (box_r, list) = modal_frame(frame, area, 2, "delete");
    if list.width > 0 && list.height > 0 {
        // A bundle is one row and several entries. Deleting it takes them all,
        // and saying how many is the difference between a confirmed action and
        // a surprised one.
        let mut lines = vec![Line::from(Span::styled(
            label,
            Style::default()
                .fg(color(DANGER))
                .add_modifier(Modifier::BOLD),
        ))];
        if count > 1 {
            lines.push(Line::from(Span::styled(
                format!("{count} entries, all of them"),
                Style::default().fg(color(DANGER)),
            )));
        }
        frame.render_widget(Paragraph::new(lines), ratatui::layout::Rect::from(list));
    }
    draw_status(frame, box_r, None, "Enter confirm · Esc cancel");
    draw_buttons(frame, box_r, "confirm")
}

/// `field` or `fields`.
fn plural(n: usize) -> &'static str {
    if n == 1 {
        "field"
    } else {
        "fields"
    }
}

/// `s` in exactly `w` chars: padded, or cut.
fn fit(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        return s.chars().take(w).collect();
    }
    let mut out = String::with_capacity(w);
    out.push_str(s);
    out.push_str(&" ".repeat(w - n));
    out
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use super::*;
    use crossterm::event::{MouseEvent, MouseEventKind};

    fn key(code: KeyCode) -> Event {
        map_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(c: char) -> Event {
        map_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
    }

    fn wheel(kind: MouseEventKind) -> Event {
        map_term(TermEvent::Mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }))
    }

    #[test]
    fn T_TUI_KEYS() {
        assert_eq!(key(KeyCode::Left), Event::Left);
        assert_eq!(key(KeyCode::Right), Event::Right);
        assert_eq!(key(KeyCode::Home), Event::Home);
        assert_eq!(key(KeyCode::End), Event::End);
        assert_eq!(key(KeyCode::Delete), Event::Delete);
        assert_eq!(key(KeyCode::BackTab), Event::BackTab);
        assert_eq!(key(KeyCode::PageUp), Event::PageUp);
        assert_eq!(key(KeyCode::PageDown), Event::PageDown);
        assert_eq!(key(KeyCode::Tab), Event::Tab);
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT)),
            Event::BackTab,
            "a terminal that sends Shift-Tab as Tab still cycles back"
        );
        assert_eq!(key(KeyCode::Char('a')), Event::Key('a'));

        assert_eq!(ctrl('c'), Event::Quit);
        assert_eq!(ctrl('r'), Event::Reveal);
        assert_eq!(ctrl('R'), Event::Reveal);
        // The regression: a Ctrl combo must never reach the register as the
        // bare letter, or Ctrl-A opens add and Ctrl-D opens delete.
        assert_eq!(ctrl('a'), Event::Tick);
        assert_eq!(ctrl('d'), Event::Tick);
        assert_eq!(ctrl('q'), Event::Tick);

        assert_eq!(wheel(MouseEventKind::ScrollUp), Event::Up);
        assert_eq!(wheel(MouseEventKind::ScrollDown), Event::Down);
        assert_eq!(
            wheel(MouseEventKind::Down(MouseButton::Left)),
            Event::Click { column: 0, row: 0 }
        );
    }
}
