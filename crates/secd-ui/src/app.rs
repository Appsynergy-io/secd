//! Shell: desktop top utility nav, mobile bottom nav. Register / Activity / Account.

use leptos::prelude::*;

use crate::account::{AccountPage, AccountView};
use crate::activity::{ActivityPage, ActivityView};
use crate::gate::{DevicePage, GatePage, GateView};
use crate::layout::layout_mode;
use crate::register::{RegisterPage, RegisterView};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Screen {
    #[default]
    Boot,
    Gate,
    Device,
    Register,
    Activity,
    Account,
}

impl Screen {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boot => "boot",
            Self::Gate => "gate",
            Self::Device => "device",
            Self::Register => "register",
            Self::Activity => "activity",
            Self::Account => "account",
        }
    }
}

#[component]
pub fn Nav(screen: Screen) -> impl IntoView {
    let mark = |want: Screen| (screen == want).then_some("true");
    view! {
        <a href="#register" data-nav-item="register" data-current=mark(Screen::Register)>"Register"</a>
        <a href="#activity" data-nav-item="activity" data-current=mark(Screen::Activity)>"Activity"</a>
        <a href="#account" data-nav-item="account" data-current=mark(Screen::Account)>"Account"</a>
    }
}

#[component]
pub fn Chrome(screen: Screen, width_px: u32, children: Children) -> impl IntoView {
    let layout = layout_mode(width_px);
    view! {
        <div class="app" data-screen=screen.as_str() data-layout=layout.as_str()>
            <nav class="nav-top" data-nav="utility">
                <Nav screen=screen />
            </nav>
            <main class="main">{children()}</main>
            <nav class="nav-bottom" data-nav="bottom">
                <Nav screen=screen />
            </nav>
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! { <div class="app" data-page="boot"></div> }
}

#[derive(Clone, Default)]
pub struct ConsoleState {
    pub screen: Screen,
    pub width_px: u32,
    pub gate: Option<GateView>,
    pub register: RegisterView,
    pub activity: ActivityView,
    pub account: AccountView,
}

pub fn render_console(state: &ConsoleState) -> String {
    let width = state.width_px;
    let screen = state.screen;
    crate::html(move || match screen {
        Screen::Boot => view! { <App /> }.into_any(),
        Screen::Gate => {
            let g = state
                .gate
                .clone()
                .unwrap_or_else(|| crate::gate::resolve_gate(&crate::gate::GateQuery::default()));
            view! { <GatePage view=g /> }.into_any()
        }
        Screen::Device => {
            let g = state.gate.clone().unwrap_or_else(|| {
                crate::gate::resolve_gate(&crate::gate::GateQuery {
                    session: Some(crate::gate::SessionInfo {
                        email: String::new(),
                        has_passkey: false,
                        has_password: false,
                        session_id: String::new(),
                    }),
                    ..crate::gate::GateQuery::default()
                })
            });
            view! { <DevicePage view=g /> }.into_any()
        }
        Screen::Register => {
            let v = state.register.clone();
            view! {
                <Chrome screen=Screen::Register width_px=width>
                    <RegisterPage view=v />
                </Chrome>
            }
            .into_any()
        }
        Screen::Activity => {
            let v = state.activity.clone();
            view! {
                <Chrome screen=Screen::Activity width_px=width>
                    <ActivityPage view=v />
                </Chrome>
            }
            .into_any()
        }
        Screen::Account => {
            let v = state.account.clone();
            view! {
                <Chrome screen=Screen::Account width_px=width>
                    <AccountPage view=v />
                </Chrome>
            }
            .into_any()
        }
    })
}
