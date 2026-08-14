//! CancelSubscriptionDialog / UpgradeDialog / CryptoCheckoutDialog — port
//! of `dashboard/billing-actions.tsx` (T6).
//!
//! Props/callbacks splits: `useCancelSubscription` → `on_cancel` +
//! `cancelling`/`error` (the subscription id never renders and stays
//! consumer-side); `useStripeCheckout` → `on_checkout(CheckoutSelection)`
//! + `checking_out`/`error` (the consumer performs the Stripe redirect);
//! `useCryptoCheckout` → `on_quote(CryptoQuoteRequest)` + `quote`/
//! `quoting`/`error` (the consumer owns the mutation and resets its
//! quote when it re-opens the dialog); `useBillingOffering`'s
//! `?? ["monthly","annual"]` fallback becomes the `offered_cycles` prop
//! default. Selection state (plan/cycle/currency) is presentation and
//! stays here, reset on open like the reference's effects.
//!
//! Quote expiry renders via the browser's own `Date.parse` +
//! `toLocaleString` (exact reference behavior, locale/TZ included). On
//! the server (ssr) the raw RFC3339 string renders instead — a minted
//! quote only exists after client-side interaction, so a server-rendered
//! quote pane is unreachable on the real site.

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::dialog::{
    DialogContent, DialogControlled, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
};
use leptos::prelude::*;

pub const BILL_COL: &str = "asy-billact__col";
pub const BILL_GRID2: &str = "asy-billact__grid2";
pub const BILL_LABEL: &str = "asy-billact__label";
pub const BILL_FIELD: &str = "asy-billact__field";
pub const BILL_PRICE: &str = "asy-billact__price";
pub const BILL_PRICE_NUM: &str = "asy-billact__price-num";
pub const BILL_ERR: &str = "asy-billact__err";
pub const BILL_QUOTE: &str = "asy-billact__quote";
pub const BILL_QUOTE_ROW: &str = "asy-billact__quote-row";
pub const BILL_QUOTE_KEY: &str = "asy-billact__quote-key";
pub const BILL_QUOTE_AMT: &str = "asy-billact__quote-amt";
pub const BILL_QUOTE_ADDR_COL: &str = "asy-billact__quote-addr-col";
pub const BILL_QUOTE_ADDR: &str = "asy-billact__quote-addr";
pub const BILL_QUOTE_EXP: &str = "asy-billact__quote-exp";

/// A catalog plan — the fields the dialogs render.
#[derive(Clone, PartialEq, Debug)]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub price_monthly: f64,
    pub price_biweekly: f64,
    pub price_annual: f64,
}

/// A minted crypto payment quote — the fields the quote pane renders.
#[derive(Clone, PartialEq, Debug)]
pub struct CryptoPayment {
    /// Arbitrary-precision amount string (BTC has 8 decimals etc.).
    pub amount_crypto: String,
    pub currency: String,
    pub amount_usd: f64,
    pub wallet_address: String,
    /// RFC3339 UTC expiry.
    pub expires_at: String,
}

/// The Stripe checkout selection emitted by [`UpgradeDialog`].
#[derive(Clone, PartialEq, Debug)]
pub struct CheckoutSelection {
    pub plan_id: String,
    pub billing_cycle: String,
}

/// The quote request emitted by [`CryptoCheckoutDialog`].
#[derive(Clone, PartialEq, Debug)]
pub struct CryptoQuoteRequest {
    pub plan_id: String,
    pub billing_cycle: String,
    pub currency: String,
}

/// The reference's `planPriceForCycle`.
pub fn plan_price_for_cycle(plan: &Plan, cycle: &str) -> f64 {
    if cycle == "annual" {
        plan.price_annual
    } else if cycle == "biweekly" {
        plan.price_biweekly
    } else {
        plan.price_monthly
    }
}

/// The reference's `cycleSuffix` (`annual` → `"/ year"`).
pub fn cycle_suffix(cycle: &str) -> &'static str {
    if cycle == "annual" {
        "/ year"
    } else if cycle == "biweekly" {
        "/ 2 weeks"
    } else {
        "/ mo"
    }
}

/// The reference's `cycleLabel` (`annual` → `"Annual"`).
pub fn cycle_label(cycle: &str) -> &'static str {
    if cycle == "annual" {
        "Annual"
    } else if cycle == "biweekly" {
        "Biweekly"
    } else {
        "Monthly"
    }
}

/// The reference's `formatPlanPrice` (`230` → `"$230"`, `12.5` → `"$12.50"`).
pub fn format_plan_price(amount: f64) -> String {
    if amount.is_finite() && amount.fract() == 0.0 {
        format!("${}", amount as i64)
    } else {
        format!("${amount:.2}")
    }
}

/// The reference's quote-expiry formatting: `Date.parse` → invalid keeps
/// the raw string, valid renders `new Date(t).toLocaleString()`.
fn format_expires(expires_at: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let t = js_sys::Date::parse(expires_at);
        if t.is_nan() {
            return expires_at.to_owned();
        }
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(t));
        // No-arg `toLocaleString()` — browser default locale + timezone,
        // exactly what the reference calls.
        if let Ok(f) = js_sys::Reflect::get(date.as_ref(), &"toLocaleString".into()) {
            let f: js_sys::Function = f.into();
            if let Ok(s) = f.call0(date.as_ref()) {
                if let Some(s) = s.as_string() {
                    return s;
                }
            }
        }
        expires_at.to_owned()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        expires_at.to_owned()
    }
}

fn default_cycles() -> Vec<String> {
    vec!["monthly".to_owned(), "annual".to_owned()]
}

/// Confirm cancelling the current subscription at period end.
#[component]
pub fn CancelSubscriptionDialog(
    /// Pre-formatted renewal date, or `None` for the generic period copy.
    renews_at: Option<String>,
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// The consumer's cancel mutation; it closes the dialog on success.
    #[prop(into)]
    on_cancel: Callback<()>,
    #[prop(optional, into)] cancelling: Signal<bool>,
    #[prop(optional, into)] error: Signal<Option<String>>,
) -> impl IntoView {
    let until = StoredValue::new(
        renews_at
            .map(|r| format!(" {r}"))
            .unwrap_or_else(|| " the end of the current period".to_owned()),
    );
    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Cancel subscription"</DialogTitle>
                    <DialogDescription>
                        "Your plan stays active until"{until.get_value()}
                        ". You keep access until then, and you can reactivate any time before it ends."
                    </DialogDescription>
                </DialogHeader>
                {move || {
                    error
                        .get()
                        .map(|msg| {
                            view! {
                                <p class=BILL_ERR style="color: var(--color-danger)">
                                    "Could not cancel: "
                                    {msg}
                                </p>
                            }
                        })
                }}
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || cancelling.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Keep plan"
                    </Button>
                    <Button
                        variant=ButtonVariant::Danger
                        size=ButtonSize::Sm
                        attr:disabled=move || cancelling.get().then_some("")
                        on:click=move |_| on_cancel.run(())
                    >
                        {move || {
                            if cancelling.get() { "Cancelling…" } else { "Cancel at period end" }
                        }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Pick a plan + cycle and hand the selection to the consumer's Stripe
/// checkout. Used for both "Upgrade plan" (full catalog) and "Switch to
/// monthly" (pre-pinned via `lock_plan_id` / `default_cycle`).
#[component]
pub fn UpgradeDialog(
    plans: Vec<Plan>,
    current_plan_id: Option<String>,
    #[prop(optional, into, default = "monthly".into())] default_cycle: String,
    lock_plan_id: Option<String>,
    #[prop(optional, into, default = "Upgrade plan".into())] title: String,
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    /// Cycles offered for new checkouts (the reference's offering query,
    /// with its not-yet-loaded fallback as the default).
    #[prop(optional, default = default_cycles())]
    offered_cycles: Vec<String>,
    /// The consumer starts Stripe checkout and redirects on success.
    #[prop(into)]
    on_checkout: Callback<CheckoutSelection>,
    #[prop(optional, into)] checking_out: Signal<bool>,
    #[prop(optional, into)] error: Signal<Option<String>>,
) -> impl IntoView {
    let selectable = StoredValue::new(plans);
    let title = StoredValue::new(title);
    let statics = StoredValue::new((
        lock_plan_id,
        current_plan_id,
        default_cycle,
        offered_cycles,
    ));
    let initial_plan = statics.with_value(|(lock, cur, _, _)| {
        lock.clone().or_else(|| cur.clone()).unwrap_or_default()
    });
    let plan_id = RwSignal::new(initial_plan);
    let cycle = RwSignal::new(statics.with_value(|(_, _, d, _)| d.clone()));

    // The reference's reset-on-open effect.
    Effect::new(move |_| {
        if open.get() {
            plan_id.set(statics.with_value(|(lock, cur, _, _)| {
                lock.clone()
                    .or_else(|| cur.clone())
                    .or_else(|| selectable.with_value(|p| p.first().map(|p| p.id.clone())))
                    .unwrap_or_default()
            }));
            cycle.set(statics.with_value(|(_, _, d, _)| d.clone()));
        }
    });

    let valid = move || !plan_id.get().is_empty();
    let submit = move |_| {
        if !plan_id.get_untracked().is_empty() {
            on_checkout.run(CheckoutSelection {
                plan_id: plan_id.get_untracked(),
                billing_cycle: cycle.get_untracked(),
            });
        }
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>{title.get_value()}</DialogTitle>
                    <DialogDescription>
                        "You will be redirected to Stripe to complete payment. Your subscription updates once checkout completes."
                    </DialogDescription>
                </DialogHeader>
                <div class=BILL_COL>
                    {statics
                        .with_value(|(lock, _, _, _)| lock.is_none())
                        .then(|| {
                            let options = selectable
                                .get_value()
                                .into_iter()
                                .map(|p| {
                                    let current = statics
                                        .with_value(|(_, cur, _, _)| {
                                            cur.as_deref() == Some(p.id.as_str())
                                        });
                                    view! {
                                        <option value=p.id.clone()>
                                            {p.name}
                                            {current.then_some(" (current)")}
                                        </option>
                                    }
                                })
                                .collect_view();
                            view! {
                                <label class=BILL_LABEL>
                                    "Plan"
                                    <select
                                        class=BILL_FIELD
                                        prop:value=move || plan_id.get()
                                        on:change=move |ev| plan_id.set(event_target_value(&ev))
                                    >
                                        {options}
                                    </select>
                                </label>
                            }
                        })}
                    <label class=BILL_LABEL>
                        "Billing cycle"
                        <select
                            class=BILL_FIELD
                            prop:value=move || cycle.get()
                            on:change=move |ev| cycle.set(event_target_value(&ev))
                        >
                            {statics
                                .with_value(|(_, _, _, cycles)| {
                                    cycles
                                        .iter()
                                        .map(|c| {
                                            view! {
                                                <option value=c.clone()>{cycle_label(c)}</option>
                                            }
                                        })
                                        .collect_view()
                                })}
                        </select>
                    </label>
                    {move || {
                        let id = plan_id.get();
                        selectable
                            .with_value(|plans| plans.iter().find(|p| p.id == id).cloned())
                            .map(|p| {
                                let c = cycle.get();
                                view! {
                                    <p class=BILL_PRICE>
                                        {p.name.clone()}
                                        " · "
                                        <span class=format!("num {BILL_PRICE_NUM}")>
                                            {format_plan_price(plan_price_for_cycle(&p, &c))}
                                        </span>
                                        " "
                                        {cycle_suffix(&c)}
                                    </p>
                                }
                            })
                    }}
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=BILL_ERR style="color: var(--color-danger)">
                                        "Could not start checkout: "
                                        {msg}
                                    </p>
                                }
                            })
                    }}
                </div>
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || checking_out.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        "Cancel"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || (!valid() || checking_out.get()).then_some("")
                        on:click=submit
                    >
                        {move || {
                            if checking_out.get() { "Redirecting…" } else { "Continue to Stripe" }
                        }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

/// Render a minted crypto quote: amount, USD value, destination, expiry.
#[component]
fn CryptoQuote(quote: CryptoPayment) -> impl IntoView {
    let expires = format_expires(&quote.expires_at);
    view! {
        <div class=BILL_QUOTE>
            <div class=BILL_QUOTE_ROW>
                <span class=BILL_QUOTE_KEY>"Send exactly"</span>
                <span class=format!("num {BILL_QUOTE_AMT}")>
                    {quote.amount_crypto}
                    " "
                    {quote.currency}
                </span>
            </div>
            <div class=BILL_QUOTE_ROW>
                <span class=BILL_QUOTE_KEY>"USD value"</span>
                <span class="num">{format!("${:.2}", quote.amount_usd)}</span>
            </div>
            <div class=BILL_QUOTE_ADDR_COL>
                <span class=BILL_QUOTE_KEY>"To address"</span>
                <code class=format!("mono {BILL_QUOTE_ADDR}")>{quote.wallet_address}</code>
            </div>
            <span class=BILL_QUOTE_EXP>
                "Quote expires "
                {expires}
                ". The subscription activates after the payment confirms on-chain."
            </span>
        </div>
    }
}

/// Pick a currency + cycle and mint a crypto payment quote for the plan.
#[component]
pub fn CryptoCheckoutDialog(
    /// The active plan to pay for; `None` renders the no-plan error and
    /// disables quoting.
    plan_id: Option<String>,
    #[prop(into)] default_cycle: String,
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] on_open_change: Callback<bool>,
    #[prop(optional, default = default_cycles())] offered_cycles: Vec<String>,
    /// The minted quote (the consumer's mutation data; it resets it when
    /// re-opening the dialog).
    #[prop(optional, into)]
    quote: Signal<Option<CryptoPayment>>,
    #[prop(into)] on_quote: Callback<CryptoQuoteRequest>,
    #[prop(optional, into)] quoting: Signal<bool>,
    #[prop(optional, into)] error: Signal<Option<String>>,
) -> impl IntoView {
    let statics = StoredValue::new((plan_id, default_cycle, offered_cycles));
    let currency = RwSignal::new("BTC".to_owned());
    let cycle = RwSignal::new(statics.with_value(|(_, d, _)| d.clone()));

    // The reference's reset-on-open effect (the consumer resets its
    // mutation's quote alongside).
    Effect::new(move |_| {
        if open.get() {
            currency.set("BTC".to_owned());
            cycle.set(statics.with_value(|(_, d, _)| d.clone()));
        }
    });

    let submit = move |_| {
        if let Some(id) = statics.with_value(|(p, _, _)| p.clone()) {
            on_quote.run(CryptoQuoteRequest {
                plan_id: id,
                billing_cycle: cycle.get_untracked(),
                currency: currency.get_untracked(),
            });
        }
    };

    view! {
        <DialogControlled open=open on_open_change=on_open_change>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>"Pay with crypto"</DialogTitle>
                    <DialogDescription>
                        "Mint a payment quote in BTC, ETH, or LTC. Send the exact amount to the address shown before the quote expires."
                    </DialogDescription>
                </DialogHeader>
                <div class=BILL_COL>
                    {statics
                        .with_value(|(p, _, _)| p.is_none())
                        .then(|| {
                            view! {
                                <p class=BILL_ERR style="color: var(--color-danger)">
                                    "No active plan to pay for."
                                </p>
                            }
                        })}
                    <div class=BILL_GRID2>
                        <label class=BILL_LABEL>
                            "Currency"
                            <select
                                class=BILL_FIELD
                                prop:value=move || currency.get()
                                on:change=move |ev| currency.set(event_target_value(&ev))
                            >
                                <option value="BTC">"BTC"</option>
                                <option value="ETH">"ETH"</option>
                                <option value="LTC">"LTC"</option>
                            </select>
                        </label>
                        <label class=BILL_LABEL>
                            "Billing cycle"
                            <select
                                class=BILL_FIELD
                                prop:value=move || cycle.get()
                                on:change=move |ev| cycle.set(event_target_value(&ev))
                            >
                                {statics
                                    .with_value(|(_, _, cycles)| {
                                        cycles
                                            .iter()
                                            .map(|c| {
                                                view! {
                                                    <option value=c.clone()>
                                                        {cycle_label(c)}
                                                    </option>
                                                }
                                            })
                                            .collect_view()
                                    })}
                            </select>
                        </label>
                    </div>
                    {move || quote.get().map(|q| view! { <CryptoQuote quote=q /> })}
                    {move || {
                        error
                            .get()
                            .map(|msg| {
                                view! {
                                    <p class=BILL_ERR style="color: var(--color-danger)">
                                        "Could not create quote: "
                                        {msg}
                                    </p>
                                }
                            })
                    }}
                </div>
                <DialogFooter>
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        attr:disabled=move || quoting.get().then_some("")
                        on:click=move |_| on_open_change.run(false)
                    >
                        {move || if quote.get().is_some() { "Done" } else { "Cancel" }}
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        size=ButtonSize::Sm
                        attr:disabled=move || {
                            (statics.with_value(|(p, _, _)| p.is_none()) || quoting.get())
                                .then_some("")
                        }
                        on:click=submit
                    >
                        {move || {
                            if quoting.get() {
                                "Quoting…"
                            } else if quote.get().is_some() {
                                "New quote"
                            } else {
                                "Get quote"
                            }
                        }}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </DialogControlled>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{col}{{display:flex;flex-direction:column;gap:.75rem}}",
            ".{grid2}{{display:grid;grid-template-columns:1fr;gap:.75rem}}",
            "@media (width >= 40rem){{.{grid2}{{",
            "grid-template-columns:repeat(2,minmax(0,1fr))}}}}",
            ".{label}{{display:flex;flex-direction:column;gap:.25rem;",
            "font-size:11.5px;font-weight:500;color:var(--color-text-muted)}}",
            ".{field}{{height:2rem;width:100%;border-radius:var(--radius-sm);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);padding-inline:.5rem;",
            "font-size:12.5px;color:var(--color-text)}}",
            ".{price}{{font-size:12.5px;color:var(--color-text-muted)}}",
            ".{price_num}{{font-weight:500;color:var(--color-text)}}",
            ".{err}{{font-size:12px}}",
            ".{quote}{{display:flex;flex-direction:column;gap:.625rem;",
            "border-radius:var(--radius-md);border:1px solid var(--color-border);",
            "padding:.75rem;font-size:12.5px}}",
            ".{quote_row}{{display:flex;align-items:baseline;",
            "justify-content:space-between}}",
            ".{quote_key}{{color:var(--color-text-muted)}}",
            ".{quote_amt}{{font-weight:600}}",
            ".{quote_addr_col}{{display:flex;flex-direction:column;gap:.25rem}}",
            ".{quote_addr}{{word-break:break-all;border-radius:var(--radius-sm);",
            "background-color:var(--color-surface-2);padding-inline:.5rem;",
            "padding-block:.25rem;font-size:11.5px}}",
            ".{quote_exp}{{font-size:11.5px;color:var(--color-text-muted)}}",
        ),
        col = BILL_COL,
        grid2 = BILL_GRID2,
        label = BILL_LABEL,
        field = BILL_FIELD,
        price = BILL_PRICE,
        price_num = BILL_PRICE_NUM,
        err = BILL_ERR,
        quote = BILL_QUOTE,
        quote_row = BILL_QUOTE_ROW,
        quote_key = BILL_QUOTE_KEY,
        quote_amt = BILL_QUOTE_AMT,
        quote_addr_col = BILL_QUOTE_ADDR_COL,
        quote_addr = BILL_QUOTE_ADDR,
        quote_exp = BILL_QUOTE_EXP,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan {
            id: "p".into(),
            name: "Two Dedicated IP".into(),
            price_monthly: 25.0,
            price_biweekly: 12.5,
            price_annual: 230.0,
        }
    }

    #[test]
    fn price_for_cycle_matches_reference() {
        let p = plan();
        assert_eq!(plan_price_for_cycle(&p, "monthly"), 25.0);
        assert_eq!(plan_price_for_cycle(&p, "biweekly"), 12.5);
        assert_eq!(plan_price_for_cycle(&p, "annual"), 230.0);
        // Unknown cycles fall through to monthly, like the reference.
        assert_eq!(plan_price_for_cycle(&p, "weekly"), 25.0);
    }

    #[test]
    fn cycle_labels_and_suffixes() {
        assert_eq!(cycle_label("annual"), "Annual");
        assert_eq!(cycle_label("biweekly"), "Biweekly");
        assert_eq!(cycle_label("monthly"), "Monthly");
        assert_eq!(cycle_label("anything"), "Monthly");
        assert_eq!(cycle_suffix("annual"), "/ year");
        assert_eq!(cycle_suffix("biweekly"), "/ 2 weeks");
        assert_eq!(cycle_suffix("monthly"), "/ mo");
    }

    /// `formatPlanPrice`: integers render bare, fractions render 2dp.
    #[test]
    fn plan_price_formats_like_js() {
        assert_eq!(format_plan_price(230.0), "$230");
        assert_eq!(format_plan_price(12.5), "$12.50");
        assert_eq!(format_plan_price(0.0), "$0");
    }

    /// Native (ssr) expiry formatting keeps the raw string — locale
    /// formatting is a browser-side behavior.
    #[test]
    fn expires_native_is_raw() {
        assert_eq!(format_expires("2026-03-01T00:00:00Z"), "2026-03-01T00:00:00Z");
        assert_eq!(format_expires("garbage"), "garbage");
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            BILL_COL, BILL_GRID2, BILL_LABEL, BILL_FIELD, BILL_PRICE, BILL_PRICE_NUM, BILL_ERR,
            BILL_QUOTE, BILL_QUOTE_ROW, BILL_QUOTE_KEY, BILL_QUOTE_AMT,
            BILL_QUOTE_ADDR_COL, BILL_QUOTE_ADDR, BILL_QUOTE_EXP,
        ] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
