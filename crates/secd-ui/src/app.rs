//! Shell: Register / Activity / Account.

use leptos::prelude::*;

use crate::account::{AccountPage, AccountView};
use crate::activity::{ActivityPage, ActivityView};
use crate::gate::{DevicePage, GatePage, GateView};
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

    pub fn href(self) -> &'static str {
        match self {
            Self::Register => "/register",
            Self::Activity => "/activity",
            Self::Account => "/account",
            Self::Device => "/device",
            _ => "/",
        }
    }
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
        Screen::Boot => view! { <div class="app" data-page="boot"></div> }.into_any(),
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
            let mut v = state.register.clone();
            v.width_px = width;
            view! { <RegisterPage view=v /> }.into_any()
        }
        Screen::Activity => {
            let v = state.activity.clone();
            view! { <ActivityPage view=v /> }.into_any()
        }
        Screen::Account => {
            let v = state.account.clone();
            view! { <AccountPage view=v /> }.into_any()
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn App() -> impl IntoView {
    view! { <div class="app" data-page="boot"></div> }
}

#[cfg(target_arch = "wasm32")]
pub use crate::live::App;
