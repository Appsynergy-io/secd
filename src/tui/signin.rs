use std::time::{Duration, Instant};

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::login::{self, DeviceFlow, Poll, Unlocked};

use super::model::Event;
use super::theme::{color, ACCENT, DIM, OK, WARN};

pub struct SignIn {
    flow: Option<DeviceFlow>,
    pub user_code: String,
    pub verification_uri: String,
    status: String,
    pub quit: bool,
    unlocked: Option<Unlocked>,
    last_poll: Instant,
    interval: Duration,
}

impl SignIn {
    pub fn from_flow(flow: DeviceFlow) -> Self {
        let user_code = flow.user_code.clone();
        let verification_uri = flow.verification_uri.clone();
        let interval = Duration::from_secs(flow.interval.max(1));
        Self {
            flow: Some(flow),
            user_code,
            verification_uri,
            status: "waiting for approval".to_string(),
            quit: false,
            unlocked: None,
            last_poll: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
            interval,
        }
    }

    pub fn take_unlocked(&mut self) -> Option<Unlocked> {
        self.unlocked.take()
    }

    pub fn handle(&mut self, ev: Event) {
        match ev {
            Event::Esc | Event::Quit | Event::Key('q') | Event::Key('Q') => self.quit = true,
            Event::Tick => self.tick(),
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        if self.unlocked.is_some() || self.quit {
            return;
        }
        if self.last_poll.elapsed() < self.interval {
            return;
        }
        self.last_poll = Instant::now();
        let Some(flow) = self.flow.as_ref() else {
            return;
        };
        match login::poll_once(flow) {
            Ok(Poll::Pending) => {}
            Ok(Poll::Expired) => self.status = "expired — run secd again".to_string(),
            Ok(Poll::Ready { token, sealed }) => {
                let Some(flow) = self.flow.take() else {
                    return;
                };
                match login::finish(flow, token, sealed) {
                    Ok(unlocked) => {
                        self.status = "approved".to_string();
                        self.unlocked = Some(unlocked);
                    }
                    Err(e) => self.status = format!("handoff failed: {e}"),
                }
            }
            Err(e) => self.status = format!("poll: {e}"),
        }
    }
}

pub fn draw(frame: &mut Frame, signin: &SignIn) {
    let area = frame.area();
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  secd",
            Style::default()
                .fg(color(ACCENT))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Approve this machine",
            Style::default().fg(color(DIM)),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                signin.user_code.as_str(),
                Style::default()
                    .fg(color(ACCENT))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", signin.verification_uri),
            Style::default().fg(color(DIM)),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", signin.status),
            Style::default().fg(if signin.status == "approved" {
                color(OK)
            } else {
                color(WARN)
            }),
        )),
        Line::from(""),
        Line::from(Span::styled("  Esc quit", Style::default().fg(color(DIM)))),
    ];
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .title("sign-in")
                .border_style(Style::default().fg(color(ACCENT))),
        ),
        area,
    );
}
