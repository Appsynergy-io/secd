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
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;

use crate::login;

mod model;
mod signin;
mod theme;
mod view;

pub use model::{AddField, Event, Modal, Mode, Model};
pub use signin::SignIn;
pub use theme::{color, ACCENT, DANGER, DIM, HIGHLIGHT, OK, ON_ACCENT, WARN};
pub use view::{action_bar, hit_key, regions, Action, ActionHit, Rect, Regions};

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
    loop {
        terminal.draw(|f| {
            let area = Rect::from(f.area());
            let r = regions(area);
            let bar = view::action_bar_row(area, &r);
            model.set_hits(action_bar(bar, model.actions()));
            draw(f, &model);
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
        TermEvent::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => Event::Click {
            column: m.column,
            row: m.row,
        },
        TermEvent::Resize(_, _) => Event::Resize,
        _ => Event::Tick,
    }
}

fn map_key(k: KeyEvent) -> Event {
    if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c')) {
        return Event::Quit;
    }
    match k.code {
        KeyCode::Esc => Event::Esc,
        KeyCode::Enter => Event::Enter,
        KeyCode::Up => Event::Up,
        KeyCode::Down => Event::Down,
        KeyCode::Backspace => Event::Backspace,
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

pub fn draw(frame: &mut Frame, model: &Model) {
    let area = Rect::from(frame.area());
    let r = regions(area);
    draw_header(frame, r.header, model);
    draw_list(frame, r.list, model);
    draw_detail(frame, r.detail, model);
    draw_activity(frame, r.activity, model);
    let bar = view::action_bar_row(area, &r);
    draw_action_bar(frame, bar, model.actions());
    if let Mode::Modal(modal) = model.mode() {
        draw_modal(frame, area, modal);
    }
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
        ])),
        ratatui::layout::Rect::from(area),
    );
}

fn draw_list(frame: &mut Frame, area: Rect, model: &Model) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title("names")
        .border_style(Style::default().fg(color(DIM)));
    let inner = Rect::from(block.inner(ratatui::layout::Rect::from(area)));
    frame.render_widget(block, ratatui::layout::Rect::from(area));
    if inner.height == 0 {
        return;
    }
    let names = model.names();
    let h = inner.height as usize;
    let sel = model.selected();
    let start = window_start(sel, names.len(), h);
    let items: Vec<ListItem> = names
        .iter()
        .enumerate()
        .skip(start)
        .take(h)
        .map(|(i, name)| {
            let style = if i == sel {
                Style::default()
                    .fg(color(HIGHLIGHT))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(name.as_str()).style(style)
        })
        .collect();
    frame.render_widget(List::new(items), ratatui::layout::Rect::from(inner));
}

fn draw_detail(frame: &mut Frame, area: Rect, model: &Model) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title("detail")
        .border_style(Style::default().fg(color(DIM)));
    let inner = block.inner(ratatui::layout::Rect::from(area));
    frame.render_widget(block, ratatui::layout::Rect::from(area));
    frame.render_widget(Paragraph::new(model.detail_text()), inner);
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

fn draw_modal(frame: &mut Frame, area: Rect, modal: &Modal) {
    let w = area.width.min(56).max(area.width.min(20));
    let h = 8u16
        .min(area.height.saturating_sub(2))
        .max(area.height.min(5));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let box_r = ratatui::layout::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, box_r);
    let (title, body) = match modal {
        Modal::Add { name, value, focus } => {
            let nmark = if *focus == AddField::Name { ">" } else { " " };
            let vmark = if *focus == AddField::Value { ">" } else { " " };
            (
                "add",
                format!("{nmark} name  {name}\n{vmark} value {value}\n\nEnter save · Esc cancel"),
            )
        }
        Modal::Delete { name } => ("delete", format!("{name}\n\nEnter confirm · Esc cancel")),
    };
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(color(ACCENT))),
        ),
        box_r,
    );
}

fn window_start(sel: usize, len: usize, h: usize) -> usize {
    if h == 0 || len <= h {
        return 0;
    }
    if sel < h {
        0
    } else {
        sel + 1 - h
    }
}
