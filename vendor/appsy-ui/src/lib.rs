//! appsy-ui — the AppSynergy design system as one Rust crate: tokens,
//! components, behavior, fonts, icons. Consumed by Leptos apps; inline
//! [`struct@STYLESHEET`] in the document head and import components — that is
//! the whole integration.

#![recursion_limit = "512"]
#![allow(clippy::all)]
pub mod base;
pub mod behavior;
pub mod components;
pub mod fonts;
pub mod icons;
pub mod tokens;

use std::sync::LazyLock;

/// The complete stylesheet: fonts → tokens → base → components. Inline it
/// once in the document head. Self-contained: every `url()` is a `data:` URI
/// — except under the `font-files` feature, where `@font-face` sources point
/// at the consumer-served `/fonts/` route (see [`fonts::files`]).
pub static STYLESHEET: LazyLock<String> =
    LazyLock::new(|| format!("{}{}{}{}", fonts::css(), tokens::css(), base::css(), components::css()));

pub mod prelude {
    pub use crate::components::avatar::{Avatar, AvatarFallback, AvatarImage};
    pub use crate::components::auth_shell::{AuthHead, AuthShell};
    pub use crate::components::auth_side::AuthSide;
    pub use crate::components::badge::{Badge, BadgeTone};
    pub use crate::components::banner::{Banner, BannerTone};
    pub use crate::components::button::{Button, ButtonSize, ButtonVariant};
    pub use crate::components::card::{Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle};
    pub use crate::components::checkbox::Checkbox;
    pub use crate::components::connected_toast::ConnectedToast;
    pub use crate::components::device_code_challenge::DeviceCodeChallenge;
    pub use crate::components::empty_state::EmptyState;
    pub use crate::components::error_panel::ErrorPanel;
    pub use crate::components::filter_chip::FilterChip;
    pub use crate::components::flow_glyph::{FlowGlyph, NpInbound, NpOutbound};
    pub use crate::components::feature_grid::FeatureGrid;
    pub use crate::components::final_cta::FinalCTA;
    pub use crate::components::hero::Hero;
    pub use crate::components::quote_or_pricing::QuoteOrPricing;
    #[cfg(feature = "charts")]
    pub use crate::components::screenshot_section::ScreenshotSection;
    pub use crate::components::how_it_works::HowItWorksSection;
    pub use crate::components::trust_strip::TrustStrip;
    pub use crate::components::hero_diagram::HeroDiagram;
    pub use crate::components::host_metrics_card::{
        CpuSample, FilesystemMetrics, HostMetrics, HostMetricsCard, InterfaceMetrics, LoadAvg,
        MdArrayMetrics, MemoryMetrics,
    };
    pub use crate::components::input::Input;
    pub use crate::components::install_command::{InstallChannel, InstallCommand};
    pub use crate::components::ip_chip::{BetaPill, Chip, ChipTone, IpChip, LiveDot};
    pub use crate::components::key_val::KeyVal;
    pub use crate::components::kpi::Kpi;
    pub use crate::components::label::Label;
    pub use crate::components::labeled_input::LabeledInput;
    pub use crate::components::logo::{Logo, LogoSize};
    pub use crate::components::marketing_footer::{FooterGroup, FooterLink, MarketingFooter};
    pub use crate::components::legal_page::{LegalPage, LegalSection};
    pub use crate::components::marketing_nav::{MarketingNav, NavItem, ProductItem};
    pub use crate::components::mkt_section::MktSection;
    pub use crate::components::product_diagrams::{
        ContractorDenialDiagram, DiagramClientPopExit, IPMockup, MiniPath,
    };
    pub use crate::components::product_kit::{
        FilterRow, MktBetaPill, MktFeatureBlock, MktHero, MktTwoCol, MockTerminal, PriceCard,
        ProsCard, TierPreviewCard,
    };
    pub use crate::components::mark::Mark;
    pub use crate::components::os_picker::{OsPicker, OsSlug};
    pub use crate::components::port_table::{ForwardDraft, PortTable};
    pub use crate::components::profile_card::{NpProfileDef, ProfileCard};
    pub use crate::components::profile_picker::{np_profiles, ProfilePicker};
    pub use crate::components::progress_rail::{ProgressRail, ProgressRailStep, ProgressStepState};
    pub use crate::components::protected_badge::ProtectedBadge;
    pub use crate::components::radio_group::{RadioGroup, RadioGroupItem};
    pub use crate::components::segmented_tabs::SegmentedTabs;
    pub use crate::components::separator::{Separator, SeparatorOrientation};
    pub use crate::components::switch::Switch;
    pub use crate::components::tabs::{Tabs, TabsContent, TabsList, TabsTrigger};
    pub use crate::components::skeleton::{Skeleton, SkeletonCards, SkeletonTable};
    #[cfg(feature = "overlays")]
    pub use crate::components::acl_dialogs::{
        acl_glob_match, evaluate_acl_flow, AclDeleteDialog, AclEditDialog, AclRule, AclRulePatch,
        AclTestDialog,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::alert_dialog::{
        AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
        AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
        AlertDialogTrigger,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::binding_dialogs::{BindingExitRelayDialog, RelayBinding, RelayServer};
    #[cfg(feature = "overlays")]
    pub use crate::components::fleet_dialogs::{
        AgentRetireDialog, AgentRotateKeyDialog, AgentVersionDialog, FleetAgent, ServerProvision,
        ServerProvisionDialog, VersionPins,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::fleet_pet_dialogs::{
        AgentPet, PetCreate, PetCreateDialog, PetRemoveDialog,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::fleet_rollout_dialogs::{
        ImageRollout, RolloutAdvanceDialog, RolloutCreate, RolloutCreateDialog,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::dialog::{
        Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader,
        DialogTitle, DialogTrigger,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::dropdown_menu::{
        DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel,
        DropdownMenuSeparator, DropdownMenuTrigger,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::popover::{Popover, PopoverContent, PopoverTrigger};
    #[cfg(feature = "overlays")]
    pub use crate::components::customize_panel::{CustomizePanel, RuleDraft};
    pub use crate::components::activation_checklist::{
        first_todo_id, ActivationChecklist, ActivationStep, ChecklistItem,
    };
    #[cfg(all(feature = "command", feature = "overlays"))]
    pub use crate::components::dash_shell::{
        ConfiguredDashShell, DashShell, DashShellConfig, PageHead,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::billing_actions::{
        cycle_label, cycle_suffix, format_plan_price, plan_price_for_cycle,
        CancelSubscriptionDialog, CheckoutSelection, CryptoCheckoutDialog, CryptoPayment,
        CryptoQuoteRequest, Plan, UpgradeDialog,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::dns_filter_dialog::{
        FilterSkuEditorDialog, PlatformDnsFilterList, SkuCreate, SkuUpdate,
    };
    pub use crate::components::family_profile_picker::{
        DnsFilterList, FamilyProfilePicker, UNFILTERED_PROFILE,
    };
    #[cfg(feature = "charts")]
    pub use crate::components::topology::graph::{
        format_bytes as format_bytes_tunnels, fresh, layout, org_label, parse_date_ms,
        LayoutPos, LayoutState, TopologyAgent, TopologyDevice, TopologyMap, TopologyOrg,
        TopologyServer, TopologyTunnel, LIVE_HANDSHAKE_MS, LIVE_SEEN_MS,
    };
    #[cfg(feature = "charts")]
    pub use crate::components::topology::{FlowRate, NetworkMap};
    #[cfg(feature = "overlays")]
    pub use crate::components::token_create_dialog::{
        area_label, area_of, group_by_area, wildcard_of, AreaGroup, GrantableAction,
        TokenCreateDialog, TokenCreateRequest, TokenSecret, MAX_ACTIONS,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::subscription_dialogs::{
        GrantSubscriptionDialog, PlatformPlan, SubscriptionGrant, BILLING_CYCLES,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::ip_bind_dialog::{
        tunnel_label, tunnel_protocol_label, IpBindDialog, IpBindRequest, IpBindTarget,
        IpBinding, Tunnel, RULE_TYPES,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::notification_prefs::{
        title_for_kind, KindPref, NotificationPrefsDialog,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::settings_forms::{
        as_captcha_provider, num_or_null, secret_or_null, BillingSettingsForm,
        BillingSettingsRead, BillingSettingsUpdate, CryptoSettingsForm, CryptoSettingsRead,
        CryptoSettingsUpdate, EmailSettingsForm, EmailSettingsRead, EmailSettingsUpdate,
        SecretField, SecretPresence, SecuritySettingsForm, SecuritySettingsRead,
        SecuritySettingsUpdate, SettingsForm, StripeSettingsForm, StripeSettingsRead,
        StripeSettingsUpdate, CAPTCHA_PROVIDERS,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::sidebar::{initials_of, SideNavItem, Sidebar};
    #[cfg(feature = "overlays")]
    pub use crate::components::topbar::{OrgMembership, Topbar};
    #[cfg(feature = "overlays")]
    pub use crate::components::select::{
        Select, SelectContent, SelectItem, SelectLabel, SelectSeparator, SelectTrigger,
        SelectValue,
    };
    #[cfg(feature = "overlays")]
    pub use crate::components::tooltip::{Tooltip, TooltipContent, TooltipProvider, TooltipTrigger};
    #[cfg(feature = "calendar")]
    pub use crate::components::calendar::{
        Calendar, CalendarDate, CalendarMode, CalendarRange,
    };
    #[cfg(feature = "command")]
    pub use crate::components::command::{
        Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList,
        CommandSeparator,
    };
    #[cfg(all(feature = "command", feature = "overlays"))]
    pub use crate::components::command_palette::{CommandPalette, PaletteGroup, PaletteItem};
    #[cfg(feature = "toast")]
    pub use crate::components::toast::{toast, toast_error, toast_success, ToastKind, Toaster};
    #[cfg(feature = "charts")]
    pub use crate::components::area_chart::AreaChart;
    #[cfg(feature = "charts")]
    pub use crate::components::bar_list::{BarList, BarListItem};
    #[cfg(feature = "charts")]
    pub use crate::components::ring::Ring;
    #[cfg(feature = "charts")]
    pub use crate::components::sparkline::Sparkline;
    pub use crate::STYLESHEET;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Delivery-contract invariant: no external references — every `url()`
    /// in the assembled stylesheet is a `data:` URI.
    #[cfg(not(feature = "font-files"))]
    #[test]
    fn stylesheet_urls_are_all_data_uris() {
        for (i, chunk) in STYLESHEET.split("url(").enumerate().skip(1) {
            assert!(
                chunk.starts_with("data:"),
                "url() #{i} is not a data: URI: {}…",
                &chunk[..chunk.len().min(40)]
            );
        }
    }

    /// `font-files` variant of the invariant: still no external references —
    /// every `url()` is a `data:` URI or the consumer-served `/fonts/` route.
    #[cfg(feature = "font-files")]
    #[test]
    fn stylesheet_urls_are_data_uris_or_served_fonts() {
        for (i, chunk) in STYLESHEET.split("url(").enumerate().skip(1) {
            assert!(
                chunk.starts_with("data:") || chunk.starts_with("/fonts/"),
                "url() #{i} is neither data: nor /fonts/: {}…",
                &chunk[..chunk.len().min(40)]
            );
        }
    }

    #[test]
    fn stylesheet_braces_balance_and_no_empty_rules() {
        let css: &str = &STYLESHEET;
        assert_eq!(css.matches('{').count(), css.matches('}').count());
        assert!(!css.contains("{}"), "empty rule in stylesheet");
    }

    /// Both directions of the class-wiring gate: every markup class has a
    /// rule, and every `asy-` selector in the stylesheet is a class some
    /// component can emit.
    #[test]
    fn stylesheet_and_markup_classes_agree() {
        let css: &str = &STYLESHEET;
        let registered = components::classes();
        for class in &registered {
            assert!(css.contains(&format!(".{class}")), "no rule for markup class .{class}");
        }
        for chunk in css.split(".asy-").skip(1) {
            let name: String = format!(
                "asy-{}",
                chunk
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect::<String>()
            );
            assert!(
                registered.contains(&name.as_str()),
                "stylesheet rule .{name} matches no registered markup class"
            );
        }
    }

    #[test]
    fn stylesheet_orders_fonts_tokens_base_components() {
        let font = STYLESHEET.find("@font-face").expect("fonts");
        let token = STYLESHEET.find(":root{").expect("tokens");
        let base = STYLESHEET.find("box-sizing:border-box").expect("base");
        let component = STYLESHEET.find(".asy-btn{").expect("components");
        assert!(font < token && token < base && base < component);
    }
}
