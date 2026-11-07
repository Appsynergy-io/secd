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
    worker: Option<std::thread::JoinHandle<(DeviceFlow, anyhow::Result<Poll>)>>,
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
            worker: None,
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
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            return;
        }
        let Some(worker) = self.worker.take() else {
            if self.last_poll.elapsed() < self.interval {
                return;
            }
            let Some(flow) = self.flow.take() else {
                return;
            };
            self.worker = Some(std::thread::spawn(move || {
                let result = login::poll_once(&flow);
                (flow, result)
            }));
            return;
        };
        let (flow, result) = match worker.join() {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        };
        self.flow = Some(flow);
        self.last_poll = Instant::now();
        match result {
            Ok(Poll::Pending) => {}
            Ok(Poll::SlowDown(secs)) => {
                self.interval = Duration::from_secs(secs);
                if let Some(flow) = self.flow.as_mut() {
                    flow.interval = secs;
                }
            }
            Ok(Poll::Denied) => {
                self.flow = None;
                self.status = "denied in the console — run secd again".to_string();
            }
            Ok(Poll::Expired) => {
                self.flow = None;
                self.status = "expired — run secd again".to_string();
            }
            Ok(Poll::Ready {
                token,
                sealed,
                email,
            }) => {
                let Some(flow) = self.flow.take() else {
                    return;
                };
                match login::finish(flow, token, sealed) {
                    Ok(unlocked) => {
                        self.status = format!("signed in as {email}");
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_network_does_not_block_escape_or_resize() {
        let flow = DeviceFlow::from_parts(
            x25519_dalek::StaticSecret::random(),
            "ABCD-EFGH".into(),
            5,
            "https://secd.test/device".into(),
            String::new(),
        );
        let mut signin = SignIn::from_flow(flow);
        let flow = signin.flow.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        signin.worker = Some(std::thread::spawn(move || {
            rx.recv().unwrap();
            (flow, Ok(Poll::Pending))
        }));
        signin.handle(Event::Tick);
        signin.handle(Event::Resize);
        signin.handle(Event::Esc);
        assert!(signin.quit);
        tx.send(()).unwrap();
        assert!(signin.worker.take().unwrap().join().is_ok());
    }
}
