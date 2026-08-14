//! NotificationPrefsDialog — port of
//! `dashboard/notification-prefs-dialog.tsx` (T5).
//!
//! Props/callbacks split: `useNotificationPreferences` → `prefs` +
//! `loading`/`load_error`; `useUpdateNotificationPreferences` →
//! `on_toggle(KindPref)` (the sparse single-kind patch — the consumer
//! wraps it in the one-element list the PATCH takes) + `saving`/
//! `save_error`. The dialog's open state is presentation and stays here
//! (the reference's `useState`, via the Dialog primitive's own state).

use crate::components::button::{ButtonSize, ButtonVariant};
use crate::components::dialog::{
    Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger,
};
use crate::components::switch::Switch;
use leptos::prelude::*;

pub const NPREF_DIALOG: &str = "asy-npref";
pub const NPREF_SCROLL: &str = "asy-npref__scroll";
pub const NPREF_TABLE: &str = "asy-npref__table";
pub const NPREF_HEADROW: &str = "asy-npref__headrow";
pub const NPREF_TH_EVENT: &str = "asy-npref__th-event";
pub const NPREF_TH_CH: &str = "asy-npref__th-ch";
pub const NPREF_ROW_RULED: &str = "asy-npref__row--ruled";
pub const NPREF_TD_EVENT: &str = "asy-npref__td-event";
pub const NPREF_TD_CH: &str = "asy-npref__td-ch";
pub const NPREF_LOADING: &str = "asy-npref__loading";
pub const NPREF_LOAD_ERR: &str = "asy-npref__load-err";
pub const NPREF_SAVE_ERR: &str = "asy-npref__save-err";

/// One kind's channel preferences (`KindPref` in the OpenAPI schema).
#[derive(Clone, PartialEq, Debug)]
pub struct KindPref {
    /// The colon slug (e.g. `payment:failed`).
    pub kind: String,
    pub email: bool,
    pub in_app: bool,
}

/// The reference's `titleForKind`: humanise a `kind` slug
/// (`path_tier_near_cap` → `Path tier near cap`); colons pass through.
pub fn title_for_kind(kind: &str) -> String {
    let mut spaced = String::new();
    let mut in_sep = false;
    for c in kind.chars() {
        if c == '_' || c == '-' {
            if !in_sep {
                spaced.push(' ');
            }
            in_sep = true;
        } else {
            spaced.push(c);
            in_sep = false;
        }
    }
    let spaced = spaced.trim();
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The per-kind notification-preference matrix in a dialog opened from
/// the inbox's Settings button. Each kind has an Email and an In-app
/// toggle; flipping one emits that single updated kind.
#[component]
pub fn NotificationPrefsDialog(
    /// The loaded preference rows.
    #[prop(into)]
    prefs: Signal<Vec<KindPref>>,
    /// The preferences query is still pending.
    #[prop(optional, into)]
    loading: Signal<bool>,
    /// The preferences query failed (`error.message`).
    #[prop(optional, into)]
    load_error: Signal<Option<String>>,
    /// A toggle flipped: the updated single kind (sparse update).
    #[prop(into)]
    on_toggle: Callback<KindPref>,
    /// The update mutation is in flight — disables every switch.
    #[prop(optional, into)]
    saving: Signal<bool>,
    /// The update mutation failed (`error.message`).
    #[prop(optional, into)]
    save_error: Signal<Option<String>>,
    /// Trigger styling (the site passes a ghost/sm Button as the trigger).
    #[prop(optional)]
    trigger_variant: ButtonVariant,
    #[prop(optional)] trigger_size: ButtonSize,
    /// Trigger content.
    children: Children,
) -> impl IntoView {
    view! {
        <Dialog>
            <DialogTrigger variant=trigger_variant size=trigger_size>{children()}</DialogTrigger>
            <DialogContent class=NPREF_DIALOG>
                <DialogHeader>
                    <DialogTitle>"Notification preferences"</DialogTitle>
                    <DialogDescription>
                        "Choose how each kind of event reaches you. Changes save automatically."
                    </DialogDescription>
                </DialogHeader>
                {move || {
                    if let Some(err) = load_error.get() {
                        view! {
                            <p class=NPREF_LOAD_ERR>
                                "Could not load preferences: "{err}
                            </p>
                        }
                            .into_any()
                    } else if loading.get() {
                        view! { <p class=NPREF_LOADING>"Loading preferences…"</p> }.into_any()
                    } else {
                        let rows = prefs.get();
                        let last = rows.len().saturating_sub(1);
                        view! {
                            <div class=NPREF_SCROLL>
                                <table class=NPREF_TABLE>
                                    <thead>
                                        <tr class=NPREF_HEADROW>
                                            <th class=NPREF_TH_EVENT>"Event"</th>
                                            <th class=NPREF_TH_CH>"Email"</th>
                                            <th class=NPREF_TH_CH>"In-app"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {rows
                                            .into_iter()
                                            .enumerate()
                                            .map(|(i, pref)| {
                                                let row_cls = if i < last {
                                                    NPREF_ROW_RULED
                                                } else {
                                                    ""
                                                };
                                                let email_pref = pref.clone();
                                                let inapp_pref = pref.clone();
                                                view! {
                                                    <tr class=row_cls>
                                                        <td class=NPREF_TD_EVENT>
                                                            {title_for_kind(&pref.kind)}
                                                        </td>
                                                        <td class=NPREF_TD_CH>
                                                            {
                                                                let p = email_pref.clone();
                                                                move || {
                                                                    let p2 = p.clone();
                                                                    view! {
                                                                        <Switch
                                                                            attr:aria-label=format!("{} email", p.kind)
                                                                            checked=Signal::from(p.email)
                                                                            disabled=saving.get()
                                                                            on_checked_change=Callback::new(move |v: bool| {
                                                                                on_toggle
                                                                                    .run(KindPref {
                                                                                        email: v,
                                                                                        ..p2.clone()
                                                                                    })
                                                                            })
                                                                        />
                                                                    }
                                                                }
                                                            }
                                                        </td>
                                                        <td class=NPREF_TD_CH>
                                                            {
                                                                let p = inapp_pref.clone();
                                                                move || {
                                                                    let p2 = p.clone();
                                                                    view! {
                                                                        <Switch
                                                                            attr:aria-label=format!("{} in-app", p.kind)
                                                                            checked=Signal::from(p.in_app)
                                                                            disabled=saving.get()
                                                                            on_checked_change=Callback::new(move |v: bool| {
                                                                                on_toggle
                                                                                    .run(KindPref {
                                                                                        in_app: v,
                                                                                        ..p2.clone()
                                                                                    })
                                                                            })
                                                                        />
                                                                    }
                                                                }
                                                            }
                                                        </td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        }
                            .into_any()
                    }
                }}
                {move || {
                    save_error
                        .get()
                        .map(|err| {
                            view! { <p class=NPREF_SAVE_ERR>"Save failed: "{err}</p> }
                        })
                }}
            </DialogContent>
        </Dialog>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{dialog}{{max-width:32rem}}",
            ".{scroll}{{max-height:60vh;overflow-x:auto;overflow-y:auto}}",
            ".{table}{{width:100%;font-size:12.5px}}",
            ".{headrow}{{text-align:left;font-size:11px;",
            "text-transform:uppercase;letter-spacing:.04em;",
            "color:var(--color-text-muted)}}",
            ".{th_event}{{padding-block:.5rem;padding-right:.75rem;",
            "font-weight:500}}",
            ".{th_ch}{{padding-inline:.75rem;padding-block:.5rem;",
            "text-align:center;font-weight:500}}",
            // border-b border-[var(--color-border-soft)] on all rows but
            // the last (color applies to all sides).
            ".{row_ruled}{{border:0 solid var(--color-border-soft);",
            "border-bottom-width:1px}}",
            ".{td_event}{{padding-block:.625rem;padding-right:.75rem}}",
            ".{td_ch}{{padding-inline:.75rem;padding-block:.625rem;",
            "text-align:center}}",
            ".{loading}{{font-size:12.5px;color:var(--color-text-muted)}}",
            ".{load_err}{{font-size:12.5px;color:var(--color-danger)}}",
            ".{save_err}{{font-size:11.5px;color:var(--color-danger)}}",
        ),
        dialog = NPREF_DIALOG,
        scroll = NPREF_SCROLL,
        table = NPREF_TABLE,
        headrow = NPREF_HEADROW,
        th_event = NPREF_TH_EVENT,
        th_ch = NPREF_TH_CH,
        row_ruled = NPREF_ROW_RULED,
        td_event = NPREF_TD_EVENT,
        td_ch = NPREF_TD_CH,
        loading = NPREF_LOADING,
        load_err = NPREF_LOAD_ERR,
        save_err = NPREF_SAVE_ERR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_for_kind_humanises_underscore_slugs() {
        assert_eq!(title_for_kind("path_tier_near_cap"), "Path tier near cap");
        assert_eq!(title_for_kind("device_new_country"), "Device new country");
    }

    /// Colons pass through untouched — the reference's regex only spaces
    /// `[_-]+` runs.
    #[test]
    fn title_for_kind_keeps_colons() {
        assert_eq!(title_for_kind("payment:failed"), "Payment:failed");
    }

    #[test]
    fn title_for_kind_collapses_runs_and_trims() {
        assert_eq!(title_for_kind("a__b--c"), "A b c");
        assert_eq!(title_for_kind("_leading_"), "Leading");
        assert_eq!(title_for_kind(""), "");
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            NPREF_DIALOG, NPREF_SCROLL, NPREF_TABLE, NPREF_HEADROW, NPREF_TH_EVENT, NPREF_TH_CH,
            NPREF_ROW_RULED, NPREF_TD_EVENT, NPREF_TD_CH, NPREF_LOADING, NPREF_LOAD_ERR,
            NPREF_SAVE_ERR,
        ] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
