//! Settings forms — port of `platform/settings-forms.tsx` (T9): the
//! shared `SettingsForm` shell, the field primitives, `SecretField`, and
//! the Stripe / Billing / Email / Crypto / Security forms.
//!
//! Props/callbacks splits, per form: the read query → a
//! `data: Signal<Option<…Read>>` prop plus `loading`/`load_error`; the
//! update mutation → `on_save(…Update)` plus `saving`/`saved`/
//! `save_error` (the reference reflects `UseMutationResult` state);
//! Email additionally splits `useSendTestEmail` →
//! `on_send_test(String)` + `testing`/`test_error`/`test_sent`. Form
//! editing state is presentation and stays here, prefilled from `data`
//! exactly like the reference's `[query.data]` effects. The trim-or-null
//! payload coercions (`secret_or_null`, `num_or_null`) are component
//! logic and port with it.
//!
//! Field ids: the reference uses `React.useId`; ids here derive from the
//! label (deterministic scheme per the crate invariant) — the id value
//! itself is implementation vocabulary, the label→control relation is
//! what the a11y tree compares.

use crate::components::button::{Button, ButtonVariant};
use crate::components::card::Card;
use crate::components::input::Input;
use crate::components::label::Label;
use crate::components::select::{Select, SelectContent, SelectItem, SelectTrigger, SelectValue};
use crate::components::switch::Switch;
use crate::components::acl_dialogs::{fmt_num, js_number};
use crate::icons::{Icon, RI_CHECK_LINE};
use leptos::prelude::*;

pub const SETF_CARD_MSG: &str = "asy-setf__card-msg";
pub const SETF_CARD_ERR: &str = "asy-setf__card-err";
pub const SETF_CARD: &str = "asy-setf__card";
pub const SETF_FORM: &str = "asy-setf__form";
pub const SETF_ERR: &str = "asy-setf__err";
pub const SETF_SAVE_ROW: &str = "asy-setf__save-row";
pub const SETF_SAVED: &str = "asy-setf__saved";
pub const SETF_SAVED_ICON: &str = "asy-setf__saved-icon";
pub const SETF_FIELD: &str = "asy-setf__field";
pub const SETF_TOGGLE_ROW: &str = "asy-setf__toggle-row";
pub const SETF_SECRET_HEAD: &str = "asy-setf__secret-head";
pub const SETF_PRESENCE: &str = "asy-setf__presence";
pub const SETF_EMAIL_COL: &str = "asy-setf__email-col";
pub const SETF_TEST_TITLE: &str = "asy-setf__test-title";
pub const SETF_TEST_FORM: &str = "asy-setf__test-form";
pub const SETF_TEST_ERR: &str = "asy-setf__test-err";
pub const SETF_TEST_OK: &str = "asy-setf__test-ok";

/// Captcha providers — fixed enum domain mirroring the backend allowlist.
pub const CAPTCHA_PROVIDERS: [&str; 4] = ["internal", "recaptcha", "hcaptcha", "turnstile"];

/// The reference's `asCaptchaProvider`: narrow an untrusted string,
/// defaulting to `internal`.
pub fn as_captcha_provider(v: &str) -> &'static str {
    CAPTCHA_PROVIDERS
        .iter()
        .find(|p| **p == v)
        .copied()
        .unwrap_or("internal")
}

/// Whether a stored secret exists (`"<set>"` / `"<unset>"` on the wire).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecretPresence {
    Set,
    Unset,
}

impl SecretPresence {
    /// Parse the wire literal; anything but `"<set>"` reads as unset.
    pub fn from_wire(v: &str) -> Self {
        if v == "<set>" { Self::Set } else { Self::Unset }
    }
}

/// The reference's `secretOrNull`: a typed-but-blank secret becomes the
/// omit (`None`) the API expects.
pub fn secret_or_null(v: &str) -> Option<String> {
    if v.trim().is_empty() { None } else { Some(v.to_owned()) }
}

/// The reference's `numOrNull`: blank = unlimited (`None`); non-numeric
/// also `None`.
pub fn num_or_null(v: &str) -> Option<f64> {
    let t = v.trim();
    if t.is_empty() {
        return None;
    }
    let n = js_number(t);
    n.is_finite().then_some(n)
}

/// The reference's trim-or-null convention for optional text payloads.
fn trim_or_null(v: &str) -> Option<String> {
    let t = v.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// Deterministic field id from a label (replaces `React.useId`).
fn field_id(label: &str) -> String {
    let slug: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    format!("asy-setf-{slug}")
}

/// Shared shell: loading / error gates, the save row, and the saved
/// flash. The consumer owns the mutate call behind `on_submit`.
#[component]
pub fn SettingsForm(
    #[prop(optional, into)] loading: Signal<bool>,
    #[prop(optional, into)] load_error: Signal<Option<String>>,
    #[prop(into)] on_submit: Callback<()>,
    #[prop(optional, into)] saving: Signal<bool>,
    /// The mutation succeeded (`isSuccess`).
    #[prop(optional, into)]
    saved: Signal<bool>,
    #[prop(optional, into)] save_error: Signal<Option<String>>,
    children: ChildrenFn,
) -> impl IntoView {
    let children = StoredValue::new(children);
    view! {
        {move || {
            if let Some(err) = load_error.get() {
                view! {
                    <Card class=SETF_CARD_ERR attr:style="color: var(--color-danger)">
                        "Could not load: "
                        {err}
                    </Card>
                }
                    .into_any()
            } else if loading.get() {
                view! { <Card class=SETF_CARD_MSG>"Loading…"</Card> }.into_any()
            } else {
                view! {
                    <Card class=SETF_CARD>
                        <form
                            class=SETF_FORM
                            on:submit=move |ev| {
                                ev.prevent_default();
                                on_submit.run(());
                            }
                        >
                            {children.with_value(|c| c())}
                            {move || {
                                save_error
                                    .get()
                                    .map(|err| {
                                        view! {
                                            <p class=SETF_ERR style="color: var(--color-danger)">
                                                "Could not save: "
                                                {err}
                                            </p>
                                        }
                                    })
                            }}
                            <div class=SETF_SAVE_ROW>
                                <Button
                                    attr:r#type="submit"
                                    attr:disabled=move || saving.get().then_some("")
                                >
                                    {move || if saving.get() { "Saving…" } else { "Save" }}
                                </Button>
                                {move || {
                                    (saved.get() && !saving.get())
                                        .then(|| {
                                            view! {
                                                <span class=SETF_SAVED>
                                                    <Icon d=RI_CHECK_LINE class=SETF_SAVED_ICON />
                                                    " Saved"
                                                </span>
                                            }
                                        })
                                }}
                            </div>
                        </form>
                    </Card>
                }
                    .into_any()
            }
        }}
    }
}

#[component]
fn TextField(
    #[prop(into)] label: String,
    value: RwSignal<String>,
    #[prop(optional, into, default = "text".into())] r#type: String,
    #[prop(optional, into)] placeholder: Option<String>,
) -> impl IntoView {
    let id = field_id(&label);
    view! {
        <div class=SETF_FIELD>
            <Label r#for=id.clone()>{label}</Label>
            <Input
                id=id
                r#type=r#type
                attr:placeholder=placeholder
                attr:value=move || value.get()
                prop:value=move || value.get()
                on:input=move |ev| value.set(event_target_value(&ev))
            />
        </div>
    }
}

#[component]
fn NumberField(#[prop(into)] label: String, value: RwSignal<f64>) -> impl IntoView {
    let id = field_id(&label);
    let warn_label = StoredValue::new(label.clone());
    view! {
        <div class=SETF_FIELD>
            <Label r#for=id.clone()>{label}</Label>
            <Input
                id=id
                r#type="number"
                attr:value=move || fmt_num(value.get())
                prop:value=move || fmt_num(value.get())
                on:input=move |ev| {
                    let raw = event_target_value(&ev);
                    let parsed = js_number(&raw);
                    if !raw.is_empty() && !parsed.is_finite() {
                        // Non-numeric input would otherwise coerce to 0.
                        leptos::logging::warn!(
                            "NumberField {:?}: ignoring non-numeric input {raw:?}",
                            warn_label.get_value()
                        );
                        return;
                    }
                    value.set(if parsed.is_finite() { parsed } else { 0.0 });
                }
            />
        </div>
    }
}

#[component]
fn ToggleField(#[prop(into)] label: String, checked: RwSignal<bool>) -> impl IntoView {
    let id = field_id(&label);
    view! {
        <div class=SETF_TOGGLE_ROW>
            <Label r#for=id.clone()>{label}</Label>
            <Switch
                id=id
                checked=Signal::from(checked)
                on_checked_change=Callback::new(move |v: bool| checked.set(v))
            />
        </div>
    }
}

/// Secret rotation field: shows whether a value is stored; only a typed
/// value replaces it (blank keeps the stored secret intact).
#[component]
pub fn SecretField(
    #[prop(into)] label: String,
    present: SecretPresence,
    value: RwSignal<String>,
) -> impl IntoView {
    let id = field_id(&label);
    let placeholder = if present == SecretPresence::Set {
        "•••••• (leave blank to keep)"
    } else {
        "Enter a value"
    };
    view! {
        <div class=SETF_FIELD>
            <div class=SETF_SECRET_HEAD>
                <Label r#for=id.clone()>{label}</Label>
                <span class=SETF_PRESENCE>
                    {if present == SecretPresence::Set { "currently set" } else { "not set" }}
                </span>
            </div>
            <Input
                id=id
                r#type="password"
                placeholder=placeholder
                attr:value=move || value.get()
                prop:value=move || value.get()
                attr:autocomplete="off"
                on:input=move |ev| value.set(event_target_value(&ev))
            />
        </div>
    }
}

/// `GET /settings/stripe` — the fields the form renders.
#[derive(Clone, PartialEq, Debug)]
pub struct StripeSettingsRead {
    pub enabled: bool,
    pub publishable_key: Option<String>,
    pub secret_key: SecretPresence,
    pub webhook_secret: SecretPresence,
}

/// `PATCH /settings/stripe` payload emitted on save.
#[derive(Clone, PartialEq, Debug)]
pub struct StripeSettingsUpdate {
    pub enabled: bool,
    pub publishable_key: Option<String>,
    pub secret_key: Option<String>,
    pub webhook_secret: Option<String>,
}

#[component]
pub fn StripeSettingsForm(
    #[prop(into)] data: Signal<Option<StripeSettingsRead>>,
    #[prop(optional, into)] loading: Signal<bool>,
    #[prop(optional, into)] load_error: Signal<Option<String>>,
    #[prop(into)] on_save: Callback<StripeSettingsUpdate>,
    #[prop(optional, into)] saving: Signal<bool>,
    #[prop(optional, into)] saved: Signal<bool>,
    #[prop(optional, into)] save_error: Signal<Option<String>>,
) -> impl IntoView {
    let enabled = RwSignal::new(false);
    let publishable = RwSignal::new(String::new());
    let secret = RwSignal::new(String::new());
    let webhook = RwSignal::new(String::new());

    Effect::new(move |_| {
        if let Some(d) = data.get() {
            enabled.set(d.enabled);
            publishable.set(d.publishable_key.unwrap_or_default());
        }
    });

    let submit = move |_: ()| {
        on_save.run(StripeSettingsUpdate {
            enabled: enabled.get_untracked(),
            publishable_key: trim_or_null(&publishable.get_untracked()),
            secret_key: secret_or_null(&secret.get_untracked()),
            webhook_secret: secret_or_null(&webhook.get_untracked()),
        });
    };

    view! {
        <SettingsForm
            loading=Signal::derive(move || loading.get() || data.get().is_none())
            load_error=load_error
            on_submit=Callback::new(submit)
            saving=saving
            saved=saved
            save_error=save_error
        >
            <ToggleField label="Stripe enabled" checked=enabled />
            <TextField label="Publishable key" value=publishable />
            {move || {
                data.get()
                    .map(|d| {
                        view! {
                            <SecretField
                                label="Secret key"
                                present=d.secret_key
                                value=secret
                            />
                            <SecretField
                                label="Webhook secret"
                                present=d.webhook_secret
                                value=webhook
                            />
                        }
                    })
            }}
        </SettingsForm>
    }
}

/// `GET /settings/billing`.
#[derive(Clone, PartialEq, Debug)]
pub struct BillingSettingsRead {
    pub default_currency: String,
    pub invoice_prefix: Option<String>,
    pub trial_days: f64,
    pub grace_period_days: f64,
    pub tax_inclusive: bool,
    pub free_tier_enabled: bool,
    pub free_tier_max_active_orgs: Option<f64>,
}

/// `PATCH /settings/billing` payload.
#[derive(Clone, PartialEq, Debug)]
pub struct BillingSettingsUpdate {
    pub default_currency: Option<String>,
    pub invoice_prefix: Option<String>,
    pub trial_days: f64,
    pub grace_period_days: f64,
    pub tax_inclusive: bool,
    pub free_tier_enabled: bool,
    pub free_tier_max_active_orgs: Option<f64>,
}

#[component]
pub fn BillingSettingsForm(
    #[prop(into)] data: Signal<Option<BillingSettingsRead>>,
    #[prop(optional, into)] loading: Signal<bool>,
    #[prop(optional, into)] load_error: Signal<Option<String>>,
    #[prop(into)] on_save: Callback<BillingSettingsUpdate>,
    #[prop(optional, into)] saving: Signal<bool>,
    #[prop(optional, into)] saved: Signal<bool>,
    #[prop(optional, into)] save_error: Signal<Option<String>>,
) -> impl IntoView {
    let currency = RwSignal::new(String::new());
    let prefix = RwSignal::new(String::new());
    let trial = RwSignal::new(0.0_f64);
    let grace = RwSignal::new(0.0_f64);
    let tax_inclusive = RwSignal::new(false);
    let free_tier_enabled = RwSignal::new(true);
    let free_tier_max_orgs = RwSignal::new(String::new());

    Effect::new(move |_| {
        if let Some(d) = data.get() {
            currency.set(d.default_currency);
            prefix.set(d.invoice_prefix.unwrap_or_default());
            trial.set(d.trial_days);
            grace.set(d.grace_period_days);
            tax_inclusive.set(d.tax_inclusive);
            free_tier_enabled.set(d.free_tier_enabled);
            free_tier_max_orgs
                .set(d.free_tier_max_active_orgs.map(fmt_num).unwrap_or_default());
        }
    });

    let submit = move |_: ()| {
        on_save.run(BillingSettingsUpdate {
            default_currency: trim_or_null(&currency.get_untracked()),
            invoice_prefix: trim_or_null(&prefix.get_untracked()),
            trial_days: trial.get_untracked(),
            grace_period_days: grace.get_untracked(),
            tax_inclusive: tax_inclusive.get_untracked(),
            free_tier_enabled: free_tier_enabled.get_untracked(),
            free_tier_max_active_orgs: num_or_null(&free_tier_max_orgs.get_untracked()),
        });
    };

    view! {
        <SettingsForm
            loading=Signal::derive(move || loading.get() || data.get().is_none())
            load_error=load_error
            on_submit=Callback::new(submit)
            saving=saving
            saved=saved
            save_error=save_error
        >
            <TextField label="Default currency" value=currency placeholder="usd" />
            <TextField label="Invoice prefix" value=prefix placeholder="INV-" />
            <NumberField label="Trial days" value=trial />
            <NumberField label="Grace period days" value=grace />
            <ToggleField label="Tax inclusive" checked=tax_inclusive />
            <ToggleField label="Free tier signups enabled" checked=free_tier_enabled />
            <TextField
                label="Free tier org cap (blank = unlimited)"
                value=free_tier_max_orgs
                r#type="number"
                placeholder="unlimited"
            />
        </SettingsForm>
    }
}

/// `GET /settings/email`.
#[derive(Clone, PartialEq, Debug)]
pub struct EmailSettingsRead {
    pub enabled: bool,
    pub from_name: Option<String>,
    pub from_address: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<f64>,
    pub smtp_username: Option<String>,
    pub smtp_password: SecretPresence,
    pub use_tls: bool,
}

/// `PATCH /settings/email` payload.
#[derive(Clone, PartialEq, Debug)]
pub struct EmailSettingsUpdate {
    pub enabled: bool,
    pub from_name: Option<String>,
    pub from_address: Option<String>,
    pub smtp_host: Option<String>,
    /// `0` coerces to `None`, like the reference's `port || null`.
    pub smtp_port: Option<f64>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub use_tls: bool,
}

#[component]
pub fn EmailSettingsForm(
    #[prop(into)] data: Signal<Option<EmailSettingsRead>>,
    #[prop(optional, into)] loading: Signal<bool>,
    #[prop(optional, into)] load_error: Signal<Option<String>>,
    #[prop(into)] on_save: Callback<EmailSettingsUpdate>,
    #[prop(optional, into)] saving: Signal<bool>,
    #[prop(optional, into)] saved: Signal<bool>,
    #[prop(optional, into)] save_error: Signal<Option<String>>,
    /// The consumer's `useSendTestEmail` mutation.
    #[prop(into)]
    on_send_test: Callback<String>,
    #[prop(optional, into)] testing: Signal<bool>,
    #[prop(optional, into)] test_error: Signal<Option<String>>,
    #[prop(optional, into)] test_sent: Signal<bool>,
) -> impl IntoView {
    let enabled = RwSignal::new(false);
    let from_name = RwSignal::new(String::new());
    let from_addr = RwSignal::new(String::new());
    let host = RwSignal::new(String::new());
    let port = RwSignal::new(0.0_f64);
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let use_tls = RwSignal::new(true);
    let test_to = RwSignal::new(String::new());

    Effect::new(move |_| {
        if let Some(d) = data.get() {
            enabled.set(d.enabled);
            from_name.set(d.from_name.unwrap_or_default());
            from_addr.set(d.from_address.unwrap_or_default());
            host.set(d.smtp_host.unwrap_or_default());
            port.set(d.smtp_port.unwrap_or(0.0));
            username.set(d.smtp_username.unwrap_or_default());
            use_tls.set(d.use_tls);
        }
    });

    let submit = move |_: ()| {
        let p = port.get_untracked();
        on_save.run(EmailSettingsUpdate {
            enabled: enabled.get_untracked(),
            from_name: trim_or_null(&from_name.get_untracked()),
            from_address: trim_or_null(&from_addr.get_untracked()),
            smtp_host: trim_or_null(&host.get_untracked()),
            smtp_port: (p != 0.0).then_some(p),
            smtp_username: trim_or_null(&username.get_untracked()),
            smtp_password: secret_or_null(&password.get_untracked()),
            use_tls: use_tls.get_untracked(),
        });
    };

    view! {
        <div class=SETF_EMAIL_COL>
            <SettingsForm
                loading=Signal::derive(move || loading.get() || data.get().is_none())
                load_error=load_error
                on_submit=Callback::new(submit)
                saving=saving
                saved=saved
                save_error=save_error
            >
                <ToggleField label="Email enabled" checked=enabled />
                <TextField label="From name" value=from_name />
                <TextField label="From address" r#type="email" value=from_addr />
                <TextField label="SMTP host" value=host />
                <NumberField label="SMTP port" value=port />
                <TextField label="SMTP username" value=username />
                {move || {
                    data.get()
                        .map(|d| {
                            view! {
                                <SecretField
                                    label="SMTP password"
                                    present=d.smtp_password
                                    value=password
                                />
                            }
                        })
                }}
                <ToggleField label="Use TLS" checked=use_tls />
            </SettingsForm>

            <Card class=SETF_CARD>
                <span class=SETF_TEST_TITLE>"Send a test email"</span>
                <form
                    class=SETF_TEST_FORM
                    on:submit=move |ev| {
                        ev.prevent_default();
                        let to = test_to.get_untracked().trim().to_owned();
                        if !to.is_empty() {
                            on_send_test.run(to);
                        }
                    }
                >
                    <Input
                        r#type="email"
                        placeholder="you@example.com"
                        attr:value=move || test_to.get()
                        prop:value=move || test_to.get()
                        on:input=move |ev| test_to.set(event_target_value(&ev))
                    />
                    <Button
                        attr:r#type="submit"
                        variant=ButtonVariant::Default
                        attr:disabled=move || {
                            (test_to.get().trim().is_empty() || testing.get()).then_some("")
                        }
                    >
                        {move || if testing.get() { "Sending…" } else { "Send test" }}
                    </Button>
                </form>
                {move || {
                    test_error
                        .get()
                        .map(|err| {
                            view! {
                                <p class=SETF_TEST_ERR style="color: var(--color-danger)">
                                    {err}
                                </p>
                            }
                        })
                }}
                {move || {
                    test_sent
                        .get()
                        .then(|| view! { <p class=SETF_TEST_OK>"Test email sent."</p> })
                }}
            </Card>
        </div>
    }
}

/// `GET /settings/crypto`.
#[derive(Clone, PartialEq, Debug)]
pub struct CryptoSettingsRead {
    pub enabled: bool,
    pub btc_wallet_address: Option<String>,
    pub eth_wallet_address: Option<String>,
    pub ltc_wallet_address: Option<String>,
    pub rate_provider: String,
    pub confirmation_threshold: f64,
    pub payment_window_minutes: f64,
}

/// `PATCH /settings/crypto` payload.
#[derive(Clone, PartialEq, Debug)]
pub struct CryptoSettingsUpdate {
    pub enabled: bool,
    pub btc_wallet_address: Option<String>,
    pub eth_wallet_address: Option<String>,
    pub ltc_wallet_address: Option<String>,
    pub rate_provider: Option<String>,
    pub confirmation_threshold: f64,
    pub payment_window_minutes: f64,
}

#[component]
pub fn CryptoSettingsForm(
    #[prop(into)] data: Signal<Option<CryptoSettingsRead>>,
    #[prop(optional, into)] loading: Signal<bool>,
    #[prop(optional, into)] load_error: Signal<Option<String>>,
    #[prop(into)] on_save: Callback<CryptoSettingsUpdate>,
    #[prop(optional, into)] saving: Signal<bool>,
    #[prop(optional, into)] saved: Signal<bool>,
    #[prop(optional, into)] save_error: Signal<Option<String>>,
) -> impl IntoView {
    let enabled = RwSignal::new(false);
    let btc = RwSignal::new(String::new());
    let eth = RwSignal::new(String::new());
    let ltc = RwSignal::new(String::new());
    let provider = RwSignal::new(String::new());
    let threshold = RwSignal::new(0.0_f64);
    let window_min = RwSignal::new(0.0_f64);

    Effect::new(move |_| {
        if let Some(d) = data.get() {
            enabled.set(d.enabled);
            btc.set(d.btc_wallet_address.unwrap_or_default());
            eth.set(d.eth_wallet_address.unwrap_or_default());
            ltc.set(d.ltc_wallet_address.unwrap_or_default());
            provider.set(d.rate_provider);
            threshold.set(d.confirmation_threshold);
            window_min.set(d.payment_window_minutes);
        }
    });

    let submit = move |_: ()| {
        on_save.run(CryptoSettingsUpdate {
            enabled: enabled.get_untracked(),
            btc_wallet_address: trim_or_null(&btc.get_untracked()),
            eth_wallet_address: trim_or_null(&eth.get_untracked()),
            ltc_wallet_address: trim_or_null(&ltc.get_untracked()),
            rate_provider: trim_or_null(&provider.get_untracked()),
            confirmation_threshold: threshold.get_untracked(),
            payment_window_minutes: window_min.get_untracked(),
        });
    };

    view! {
        <SettingsForm
            loading=Signal::derive(move || loading.get() || data.get().is_none())
            load_error=load_error
            on_submit=Callback::new(submit)
            saving=saving
            saved=saved
            save_error=save_error
        >
            <ToggleField label="Crypto payments enabled" checked=enabled />
            <TextField label="BTC wallet" value=btc />
            <TextField label="ETH wallet" value=eth />
            <TextField label="LTC wallet" value=ltc />
            <TextField label="Rate provider" value=provider placeholder="coingecko" />
            <NumberField label="Confirmation threshold" value=threshold />
            <NumberField label="Payment window (min)" value=window_min />
        </SettingsForm>
    }
}

/// `GET /settings/security`.
#[derive(Clone, PartialEq, Debug)]
pub struct SecuritySettingsRead {
    pub captcha_provider: String,
    pub captcha_site_key: Option<String>,
    pub captcha_secret_key: SecretPresence,
    pub captcha_login_enabled: bool,
    pub captcha_signup_enabled: bool,
    pub captcha_password_reset_enabled: bool,
    pub login_throttle_enabled: bool,
    pub login_throttle_max_attempts: f64,
    pub login_throttle_window_seconds: f64,
    pub lockout_threshold_soft: f64,
    pub lockout_window_soft_seconds: f64,
    pub lockout_threshold_medium: f64,
    pub lockout_window_medium_seconds: f64,
    pub lockout_threshold_hard: f64,
    pub lockout_window_hard_seconds: f64,
    pub rate_limit_anon_max: f64,
    pub rate_limit_anon_window_seconds: f64,
    pub rate_limit_principal_max: f64,
    pub rate_limit_principal_window_seconds: f64,
    pub rate_limit_leads_max: f64,
    pub rate_limit_leads_window_seconds: f64,
}

/// `PATCH /settings/security` payload — same fields, secret as rotation.
#[derive(Clone, PartialEq, Debug)]
pub struct SecuritySettingsUpdate {
    pub captcha_provider: String,
    pub captcha_site_key: Option<String>,
    pub captcha_secret_key: Option<String>,
    pub captcha_login_enabled: bool,
    pub captcha_signup_enabled: bool,
    pub captcha_password_reset_enabled: bool,
    pub login_throttle_enabled: bool,
    pub login_throttle_max_attempts: f64,
    pub login_throttle_window_seconds: f64,
    pub lockout_threshold_soft: f64,
    pub lockout_window_soft_seconds: f64,
    pub lockout_threshold_medium: f64,
    pub lockout_window_medium_seconds: f64,
    pub lockout_threshold_hard: f64,
    pub lockout_window_hard_seconds: f64,
    pub rate_limit_anon_max: f64,
    pub rate_limit_anon_window_seconds: f64,
    pub rate_limit_principal_max: f64,
    pub rate_limit_principal_window_seconds: f64,
    pub rate_limit_leads_max: f64,
    pub rate_limit_leads_window_seconds: f64,
}

#[component]
pub fn SecuritySettingsForm(
    #[prop(into)] data: Signal<Option<SecuritySettingsRead>>,
    #[prop(optional, into)] loading: Signal<bool>,
    #[prop(optional, into)] load_error: Signal<Option<String>>,
    #[prop(into)] on_save: Callback<SecuritySettingsUpdate>,
    #[prop(optional, into)] saving: Signal<bool>,
    #[prop(optional, into)] saved: Signal<bool>,
    #[prop(optional, into)] save_error: Signal<Option<String>>,
) -> impl IntoView {
    let provider = RwSignal::new("internal".to_owned());
    let site_key = RwSignal::new(String::new());
    let secret = RwSignal::new(String::new());
    let on_login = RwSignal::new(false);
    let on_signup = RwSignal::new(false);
    let on_reset = RwSignal::new(false);
    let throttle = RwSignal::new(false);
    let max_attempts = RwSignal::new(0.0_f64);
    let window_sec = RwSignal::new(0.0_f64);
    let lock_soft_threshold = RwSignal::new(0.0_f64);
    let lock_soft_window = RwSignal::new(0.0_f64);
    let lock_med_threshold = RwSignal::new(0.0_f64);
    let lock_med_window = RwSignal::new(0.0_f64);
    let lock_hard_threshold = RwSignal::new(0.0_f64);
    let lock_hard_window = RwSignal::new(0.0_f64);
    let rl_anon_max = RwSignal::new(0.0_f64);
    let rl_anon_window = RwSignal::new(0.0_f64);
    let rl_principal_max = RwSignal::new(0.0_f64);
    let rl_principal_window = RwSignal::new(0.0_f64);
    let rl_leads_max = RwSignal::new(0.0_f64);
    let rl_leads_window = RwSignal::new(0.0_f64);

    Effect::new(move |_| {
        if let Some(d) = data.get() {
            provider.set(as_captcha_provider(&d.captcha_provider).to_owned());
            site_key.set(d.captcha_site_key.unwrap_or_default());
            on_login.set(d.captcha_login_enabled);
            on_signup.set(d.captcha_signup_enabled);
            on_reset.set(d.captcha_password_reset_enabled);
            throttle.set(d.login_throttle_enabled);
            max_attempts.set(d.login_throttle_max_attempts);
            window_sec.set(d.login_throttle_window_seconds);
            lock_soft_threshold.set(d.lockout_threshold_soft);
            lock_soft_window.set(d.lockout_window_soft_seconds);
            lock_med_threshold.set(d.lockout_threshold_medium);
            lock_med_window.set(d.lockout_window_medium_seconds);
            lock_hard_threshold.set(d.lockout_threshold_hard);
            lock_hard_window.set(d.lockout_window_hard_seconds);
            rl_anon_max.set(d.rate_limit_anon_max);
            rl_anon_window.set(d.rate_limit_anon_window_seconds);
            rl_principal_max.set(d.rate_limit_principal_max);
            rl_principal_window.set(d.rate_limit_principal_window_seconds);
            rl_leads_max.set(d.rate_limit_leads_max);
            rl_leads_window.set(d.rate_limit_leads_window_seconds);
        }
    });

    let provider_value: Signal<String> = provider.into();
    let submit = move |_: ()| {
        on_save.run(SecuritySettingsUpdate {
            captcha_provider: provider.get_untracked(),
            captcha_site_key: trim_or_null(&site_key.get_untracked()),
            captcha_secret_key: secret_or_null(&secret.get_untracked()),
            captcha_login_enabled: on_login.get_untracked(),
            captcha_signup_enabled: on_signup.get_untracked(),
            captcha_password_reset_enabled: on_reset.get_untracked(),
            login_throttle_enabled: throttle.get_untracked(),
            login_throttle_max_attempts: max_attempts.get_untracked(),
            login_throttle_window_seconds: window_sec.get_untracked(),
            lockout_threshold_soft: lock_soft_threshold.get_untracked(),
            lockout_window_soft_seconds: lock_soft_window.get_untracked(),
            lockout_threshold_medium: lock_med_threshold.get_untracked(),
            lockout_window_medium_seconds: lock_med_window.get_untracked(),
            lockout_threshold_hard: lock_hard_threshold.get_untracked(),
            lockout_window_hard_seconds: lock_hard_window.get_untracked(),
            rate_limit_anon_max: rl_anon_max.get_untracked(),
            rate_limit_anon_window_seconds: rl_anon_window.get_untracked(),
            rate_limit_principal_max: rl_principal_max.get_untracked(),
            rate_limit_principal_window_seconds: rl_principal_window.get_untracked(),
            rate_limit_leads_max: rl_leads_max.get_untracked(),
            rate_limit_leads_window_seconds: rl_leads_window.get_untracked(),
        });
    };

    view! {
        <SettingsForm
            loading=Signal::derive(move || loading.get() || data.get().is_none())
            load_error=load_error
            on_submit=Callback::new(submit)
            saving=saving
            saved=saved
            save_error=save_error
        >
            <div class=SETF_FIELD>
                <Label>"Captcha provider"</Label>
                <Select
                    value=provider_value
                    on_value_change=Callback::new(move |v: String| {
                        provider.set(as_captcha_provider(&v).to_owned());
                    })
                >
                    <SelectTrigger>
                        <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                        {CAPTCHA_PROVIDERS
                            .iter()
                            .map(|p| {
                                view! { <SelectItem value=*p>{*p}</SelectItem> }
                            })
                            .collect_view()}
                    </SelectContent>
                </Select>
            </div>
            <TextField label="Captcha site key" value=site_key />
            {move || {
                data.get()
                    .map(|d| {
                        view! {
                            <SecretField
                                label="Captcha secret key"
                                present=d.captcha_secret_key
                                value=secret
                            />
                        }
                    })
            }}
            <ToggleField label="Captcha on login" checked=on_login />
            <ToggleField label="Captcha on signup" checked=on_signup />
            <ToggleField label="Captcha on password reset" checked=on_reset />
            <ToggleField label="Login throttle" checked=throttle />
            <NumberField label="Throttle max attempts" value=max_attempts />
            <NumberField label="Throttle window (s)" value=window_sec />
            <NumberField label="Lockout soft threshold (failures)" value=lock_soft_threshold />
            <NumberField label="Lockout soft window (s)" value=lock_soft_window />
            <NumberField label="Lockout medium threshold (failures)" value=lock_med_threshold />
            <NumberField label="Lockout medium window (s)" value=lock_med_window />
            <NumberField label="Lockout hard threshold (failures)" value=lock_hard_threshold />
            <NumberField label="Lockout hard window (s)" value=lock_hard_window />
            <NumberField label="Rate limit: anonymous max requests" value=rl_anon_max />
            <NumberField label="Rate limit: anonymous window (s)" value=rl_anon_window />
            <NumberField label="Rate limit: per-principal max requests" value=rl_principal_max />
            <NumberField label="Rate limit: per-principal window (s)" value=rl_principal_window />
            <NumberField label="Rate limit: lead submissions max" value=rl_leads_max />
            <NumberField label="Rate limit: lead submissions window (s)" value=rl_leads_window />
        </SettingsForm>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{card_msg}{{padding:1.5rem;font-size:12.5px;",
            "color:var(--color-text-muted)}}",
            ".{card_err}{{padding:1.5rem;font-size:12.5px}}",
            ".{card}{{max-width:560px;padding:1.25rem}}",
            ".{form}{{display:grid;gap:1rem}}",
            ".{err}{{font-size:12px}}",
            ".{save_row}{{display:flex;align-items:center;gap:.625rem}}",
            ".{saved}{{display:flex;align-items:center;gap:.25rem;",
            "font-size:12px;color:var(--color-success)}}",
            ".{saved_icon}{{width:.875rem;height:.875rem}}",
            ".{field}{{display:grid;gap:.375rem}}",
            ".{toggle_row}{{display:flex;align-items:center;",
            "justify-content:space-between}}",
            ".{secret_head}{{display:flex;align-items:center;",
            "justify-content:space-between}}",
            ".{presence}{{font-size:11px;color:var(--color-text-muted)}}",
            ".{email_col}{{display:flex;flex-direction:column;gap:.75rem}}",
            ".{test_title}{{font-size:13px;font-weight:500}}",
            ".{test_form}{{margin-top:.75rem;display:flex;flex-wrap:wrap;",
            "align-items:center;gap:.5rem}}",
            ".{test_form} .asy-input{{min-width:0;flex:1 1 12rem}}",
            ".{test_err}{{margin-top:.5rem;font-size:12px}}",
            ".{test_ok}{{margin-top:.5rem;font-size:12px;",
            "color:var(--color-success)}}",
        ),
        card_msg = SETF_CARD_MSG,
        card_err = SETF_CARD_ERR,
        card = SETF_CARD,
        form = SETF_FORM,
        err = SETF_ERR,
        save_row = SETF_SAVE_ROW,
        saved = SETF_SAVED,
        saved_icon = SETF_SAVED_ICON,
        field = SETF_FIELD,
        toggle_row = SETF_TOGGLE_ROW,
        secret_head = SETF_SECRET_HEAD,
        presence = SETF_PRESENCE,
        email_col = SETF_EMAIL_COL,
        test_title = SETF_TEST_TITLE,
        test_form = SETF_TEST_FORM,
        test_err = SETF_TEST_ERR,
        test_ok = SETF_TEST_OK,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captcha_provider_narrowing() {
        assert_eq!(as_captcha_provider("recaptcha"), "recaptcha");
        assert_eq!(as_captcha_provider("turnstile"), "turnstile");
        assert_eq!(as_captcha_provider("bogus"), "internal");
        assert_eq!(as_captcha_provider(""), "internal");
    }

    #[test]
    fn secret_or_null_blank_means_keep() {
        assert_eq!(secret_or_null(""), None);
        assert_eq!(secret_or_null("   "), None);
        // Untrimmed value passes through — the reference sends `v`, not
        // `v.trim()`.
        assert_eq!(secret_or_null(" x "), Some(" x ".to_owned()));
    }

    #[test]
    fn num_or_null_blank_means_unlimited() {
        assert_eq!(num_or_null(""), None);
        assert_eq!(num_or_null("  "), None);
        assert_eq!(num_or_null("5"), Some(5.0));
        assert_eq!(num_or_null("abc"), None);
    }

    #[test]
    fn secret_presence_wire_literals() {
        assert_eq!(SecretPresence::from_wire("<set>"), SecretPresence::Set);
        assert_eq!(SecretPresence::from_wire("<unset>"), SecretPresence::Unset);
        assert_eq!(SecretPresence::from_wire("anything"), SecretPresence::Unset);
    }

    #[test]
    fn field_ids_are_deterministic() {
        assert_eq!(field_id("SMTP host"), field_id("SMTP host"));
        assert_eq!(field_id("Trial days"), "asy-setf-trial-days");
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            SETF_CARD_MSG, SETF_CARD_ERR, SETF_CARD, SETF_FORM, SETF_ERR, SETF_SAVE_ROW,
            SETF_SAVED, SETF_SAVED_ICON, SETF_FIELD, SETF_TOGGLE_ROW, SETF_SECRET_HEAD,
            SETF_PRESENCE, SETF_EMAIL_COL, SETF_TEST_TITLE, SETF_TEST_FORM, SETF_TEST_ERR,
            SETF_TEST_OK,
        ] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
