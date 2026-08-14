//! One module per component: Leptos view + `css()` + class-name consts +
//! tests. `css()` here assembles every component's styles into the shared
//! stylesheet in a stable order.

/// Remove hydration-marker comment nodes from an element's children.
/// Chromium's accessibility tree treats text mixed with comment nodes
/// differently from React's plain adjacent text nodes: a whitespace-only
/// run between comments is dropped, and a button child that would be
/// pruned as redundant with its `aria-label` stays exposed. Stripping the
/// markers restores the reference's AX shape; tachys text bindings hold
/// the text nodes themselves and stay live.
#[cfg(any(feature = "csr", feature = "hydrate"))]
pub(crate) fn strip_comment_children(el: &web_sys::Element) {
    let nodes = el.child_nodes();
    let comments: Vec<web_sys::Node> = (0..nodes.length())
        .filter_map(|i| nodes.item(i))
        .filter(|n| n.node_type() == web_sys::Node::COMMENT_NODE)
        .collect();
    for c in comments {
        let _ = el.remove_child(&c);
    }
}

pub mod avatar;
pub mod badge;
pub mod banner;
pub mod button;
pub mod card;
pub mod checkbox;
pub mod connected_toast;
pub mod device_code_challenge;
pub mod empty_state;
pub mod error_panel;
pub mod filter_chip;
pub mod flow_glyph;
pub mod host_metrics_card;
pub mod input;
pub mod install_command;
pub mod ip_chip;
pub mod key_val;
pub mod kpi;
pub mod label;
pub mod logo;
pub mod mark;
pub mod os_picker;
pub mod port_table;
pub mod profile_card;
pub mod hero;
pub mod hero_diagram;
pub mod feature_grid;
pub mod final_cta;
pub mod how_it_works;
pub mod marketing_footer;
pub mod auth_shell;
pub mod auth_side;
pub mod labeled_input;
pub mod legal_page;
pub mod marketing_nav;
pub mod mkt_section;
pub mod product_diagrams;
pub mod product_kit;
pub mod profile_picker;
pub mod quote_or_pricing;
#[cfg(feature = "charts")]
pub mod screenshot_section;
pub mod trust_strip;
pub mod progress_rail;
pub mod radio_group;
pub mod protected_badge;
pub mod segmented_tabs;
pub mod separator;
pub mod switch;
pub mod skeleton;
pub mod tabs;
#[cfg(feature = "overlays")]
pub mod acl_dialogs;
#[cfg(feature = "overlays")]
pub mod alert_dialog;
#[cfg(feature = "overlays")]
pub mod binding_dialogs;
#[cfg(feature = "overlays")]
pub mod dialog;
#[cfg(feature = "overlays")]
pub mod dropdown_menu;
#[cfg(feature = "overlays")]
pub mod popover;
#[cfg(feature = "overlays")]
pub mod customize_panel;
#[cfg(feature = "overlays")]
pub mod fleet_dialogs;
#[cfg(feature = "overlays")]
pub mod fleet_pet_dialogs;
#[cfg(feature = "overlays")]
pub mod fleet_rollout_dialogs;
#[cfg(feature = "overlays")]
pub mod select;
#[cfg(feature = "overlays")]
pub mod billing_actions;
#[cfg(feature = "overlays")]
pub mod dns_filter_dialog;
pub mod family_profile_picker;
#[cfg(feature = "overlays")]
pub mod ip_bind_dialog;
#[cfg(feature = "overlays")]
pub mod notification_prefs;
#[cfg(feature = "overlays")]
pub mod settings_forms;
#[cfg(feature = "overlays")]
pub mod sidebar;
#[cfg(feature = "overlays")]
pub mod subscription_dialogs;
#[cfg(feature = "overlays")]
pub mod token_create_dialog;
#[cfg(feature = "overlays")]
pub mod topbar;
#[cfg(feature = "overlays")]
pub mod tooltip;
#[cfg(feature = "calendar")]
pub mod calendar;
#[cfg(feature = "command")]
pub mod command;
#[cfg(all(feature = "command", feature = "overlays"))]
pub mod command_palette;
#[cfg(all(feature = "command", feature = "overlays"))]
pub mod dash_shell;
pub mod activation_checklist;
#[cfg(feature = "toast")]
pub mod toast;
#[cfg(feature = "charts")]
pub mod area_chart;
#[cfg(feature = "charts")]
pub mod bar_list;
#[cfg(feature = "charts")]
pub mod ring;
#[cfg(feature = "charts")]
pub mod sparkline;
#[cfg(feature = "charts")]
pub mod topology;

pub fn css() -> String {
    #[allow(unused_mut)]
    let mut css = avatar::css()
        + &badge::css()
        + &checkbox::css()
        + &connected_toast::css()
        + &device_code_challenge::css()
        + &banner::css()
        + &button::css()
        + &card::css()
        + &empty_state::css()
        + &error_panel::css()
        + &filter_chip::css()
        + &flow_glyph::css()
        + &host_metrics_card::css()
        + &input::css()
        + &install_command::css()
        + &ip_chip::css()
        + &key_val::css()
        + &kpi::css()
        + &logo::css()
        + &mark::css()
        + &label::css()
        + &os_picker::css()
        + &port_table::css()
        + &profile_card::css()
        + &feature_grid::css()
        + &final_cta::css()
        + &hero::css()
        + &how_it_works::css()
        + &trust_strip::css()
        + &hero_diagram::css()
        + &marketing_footer::css()
        + &auth_shell::css()
        + &auth_side::css()
        + &labeled_input::css()
        + &legal_page::css()
        + &marketing_nav::css()
        + &mkt_section::css()
        + &product_diagrams::css()
        + &product_kit::css()
        + &profile_picker::css()
        + &quote_or_pricing::css()
        + &progress_rail::css()
        + &protected_badge::css()
        + &radio_group::css()
        + &segmented_tabs::css()
        + &separator::css()
        + &skeleton::css()
        + &switch::css()
        + &tabs::css()
        + &activation_checklist::css()
        + &family_profile_picker::css();
    #[cfg(feature = "overlays")]
    {
        css += &acl_dialogs::css();
        css += &alert_dialog::css();
        css += &binding_dialogs::css();
        css += &dialog::css();
        css += &dropdown_menu::css();
        css += &fleet_dialogs::css();
        css += &fleet_pet_dialogs::css();
        css += &fleet_rollout_dialogs::css();
        css += &popover::css();
        css += &select::css();
        css += &billing_actions::css();
        css += &dns_filter_dialog::css();
        css += &ip_bind_dialog::css();
        css += &notification_prefs::css();
        css += &settings_forms::css();
        css += &sidebar::css();
        css += &subscription_dialogs::css();
        css += &token_create_dialog::css();
        css += &topbar::css();
        css += &tooltip::css();
        // After select: the panel's w150 override on SelectTrigger wins by
        // rule order at equal specificity.
        css += &customize_panel::css();
    }
    #[cfg(feature = "calendar")]
    {
        css += &calendar::css();
    }
    #[cfg(feature = "command")]
    {
        css += &command::css();
    }
    #[cfg(all(feature = "command", feature = "overlays"))]
    {
        css += &command_palette::css();
        css += &dash_shell::css();
    }
    #[cfg(feature = "toast")]
    {
        css += &toast::css();
    }
    #[cfg(feature = "charts")]
    {
        css += &area_chart::css();
        css += &topology::css();
        css += &screenshot_section::css();
        css += &bar_list::css();
        css += &ring::css();
        css += &sparkline::css();
    }
    css
}

/// Every `asy-` class any component's markup can emit. The stylesheet-wiring
/// test holds this and the assembled CSS to each other in both directions:
/// a class without a rule and a rule without a class both fail.
pub fn classes() -> Vec<&'static str> {
    #[allow(unused_mut)]
    let mut classes = vec![
        avatar::AVATAR,
        avatar::AVATAR_IMAGE,
        avatar::AVATAR_FALLBACK,
        badge::BADGE,
        badge::BADGE_DEFAULT,
        badge::BADGE_OK,
        badge::BADGE_WARN,
        badge::BADGE_BAD,
        badge::BADGE_ACCENT,
        badge::BADGE_BETA,
        badge::BADGE_DOT,
        banner::BANNER,
        banner::BANNER_INFO,
        banner::BANNER_SUCCESS,
        banner::BANNER_WARNING,
        banner::BANNER_DANGER,
        banner::BANNER_CHIP,
        banner::BANNER_GLYPH,
        banner::BANNER_BODY,
        banner::BANNER_TITLE,
        banner::BANNER_DETAIL,
        banner::BANNER_ACTION,
        button::BTN,
        button::BTN_DEFAULT,
        button::BTN_PRIMARY,
        button::BTN_GHOST,
        button::BTN_DANGER,
        button::BTN_SM,
        button::BTN_MD,
        button::BTN_LG,
        card::CARD,
        card::CARD_HEADER,
        card::CARD_TITLE,
        card::CARD_DESCRIPTION,
        card::CARD_CONTENT,
        card::CARD_FOOTER,
        checkbox::CHECKBOX,
        checkbox::CHECKBOX_CHECKED,
        checkbox::CHECKBOX_INDICATOR,
        checkbox::CHECKBOX_GLYPH,
        connected_toast::CONNECTED_TOAST,
        connected_toast::CONNECTED_TOAST_GLYPH,
        connected_toast::CONNECTED_TOAST_BODY,
        connected_toast::CONNECTED_TOAST_TITLE,
        connected_toast::CONNECTED_TOAST_DETAIL,
        device_code_challenge::DEVICE_CODE,
        device_code_challenge::DEVICE_CODE_LOADING,
        device_code_challenge::DEVICE_CODE_SPINNER,
        device_code_challenge::DEVICE_CODE_EXPIRED,
        device_code_challenge::DEVICE_CODE_EXPIRED_MSG,
        device_code_challenge::DEVICE_CODE_REMINT_GLYPH,
        device_code_challenge::DEVICE_CODE_CODE,
        device_code_challenge::DEVICE_CODE_HINT,
        empty_state::EMPTY,
        empty_state::EMPTY_CHIP,
        empty_state::EMPTY_GLYPH,
        empty_state::EMPTY_TITLE,
        empty_state::EMPTY_BODY,
        empty_state::EMPTY_ACTION,
        error_panel::ERROR_PANEL,
        error_panel::ERROR_CHIP,
        error_panel::ERROR_GLYPH,
        error_panel::ERROR_BODY,
        error_panel::ERROR_TITLE,
        error_panel::ERROR_DETAIL,
        error_panel::ERROR_RETRY,
        error_panel::ERROR_RETRY_GLYPH,
        filter_chip::FILTER_CHIP,
        filter_chip::FILTER_CHIP_LABEL,
        filter_chip::FILTER_CHIP_VALUE,
        filter_chip::FILTER_CHIP_ARROW,
        flow_glyph::FLOW_GLYPH,
        host_metrics_card::HMC,
        host_metrics_card::HMC_TITLE,
        host_metrics_card::HMC_SECTION,
        host_metrics_card::HMC_SECTION_HEAD,
        host_metrics_card::HMC_SECTION_GLYPH,
        host_metrics_card::HMC_SECTION_TITLE,
        host_metrics_card::HMC_STAT,
        host_metrics_card::HMC_STAT_LABEL,
        host_metrics_card::HMC_BAR,
        host_metrics_card::HMC_BAR_FILL,
        host_metrics_card::HMC_NOTE,
        host_metrics_card::HMC_ROWS,
        host_metrics_card::HMC_ROW,
        host_metrics_card::HMC_ROW_HEAD,
        host_metrics_card::HMC_ROW_META,
        input::INPUT,
        install_command::INSTALL_CMD,
        install_command::INSTALL_CMD_LABEL,
        install_command::INSTALL_CMD_PRE,
        install_command::INSTALL_CMD_COPY,
        install_command::INSTALL_CMD_COPY_GLYPH,
        install_command::INSTALL_CMD_LOADING,
        install_command::INSTALL_CMD_SPINNER,
        ip_chip::IP_CHIP,
        ip_chip::IP_CHIP_DOT,
        ip_chip::IP_CHIP_DOT_BOUND,
        ip_chip::IP_CHIP_DOT_OFF,
        ip_chip::IP_CHIP_IP,
        ip_chip::LIVE_DOT,
        ip_chip::LIVE_DOT_ON,
        ip_chip::LIVE_DOT_OFF,
        ip_chip::LIVE_PULSE,
        ip_chip::BETA_PILL,
        ip_chip::CHIP,
        ip_chip::CHIP_DEFAULT,
        ip_chip::CHIP_WARN,
        ip_chip::CHIP_OK,
        ip_chip::CHIP_BAD,
        ip_chip::CHIP_ACCENT,
        key_val::KEY_VAL,
        key_val::KEY_VAL_LABEL,
        key_val::KEY_VAL_RIGHT,
        key_val::KEY_VAL_VALUE,
        key_val::KEY_VAL_VALUE_MONO,
        key_val::KEY_VAL_BTN,
        key_val::KEY_VAL_GLYPH,
        kpi::KPI,
        kpi::KPI_HEAD,
        kpi::KPI_ACCENT,
        kpi::KPI_ROW,
        kpi::KPI_VALUE,
        kpi::KPI_SUB,
        kpi::KPI_SPARK,
        kpi::FADE_SLIDE_IN,
        label::LABEL,
        label::PEER,
        logo::LOGO,
        logo::LOGO_BRACKET,
        logo::LOGO_BRACKET_SM,
        logo::LOGO_BRACKET_MD,
        logo::LOGO_BRACKET_LG,
        logo::LOGO_BRACKET_ACCENT,
        logo::LOGO_BRACKET_MONO,
        logo::LOGO_WORDMARK,
        logo::LOGO_WORDMARK_SM,
        logo::LOGO_WORDMARK_MD,
        logo::LOGO_WORDMARK_LG,
        mark::MARK,
        os_picker::OS_PICKER,
        os_picker::OS_PICKER_BTN,
        os_picker::OS_PICKER_BTN_ON,
        os_picker::OS_PICKER_BTN_OFF,
        os_picker::OS_PICKER_GLYPH,
        port_table::PORT_TABLE,
        port_table::PORT_TABLE_HEAD,
        port_table::PORT_TABLE_TITLE_COL,
        port_table::PORT_TABLE_TITLE,
        port_table::PORT_TABLE_SUB,
        port_table::PORT_TABLE_OPEN_GLYPH,
        port_table::PORT_TABLE_SCROLL,
        port_table::TBL,
        port_table::PORT_TABLE_TABLE,
        port_table::PORT_TABLE_THEAD_ROW,
        port_table::PORT_TABLE_TH,
        port_table::PORT_TABLE_TH_MEDIUM,
        port_table::PORT_TABLE_TH_PORT,
        port_table::PORT_TABLE_TH_ARROW,
        port_table::PORT_TABLE_TH_ACTIONS,
        port_table::PORT_TABLE_EMPTY,
        port_table::PORT_TABLE_ROW,
        port_table::PORT_TABLE_TD,
        port_table::PORT_TABLE_TD_CENTER,
        port_table::PORT_TABLE_TD_NOTE,
        port_table::PORT_TABLE_TD_RIGHT,
        port_table::PORT_TABLE_CELL_WRAP,
        port_table::PORT_TABLE_PROTO_PILL,
        port_table::PORT_TABLE_PORT_VAL,
        port_table::PORT_TABLE_ROW_ARROW,
        port_table::PORT_TABLE_EP_GLYPH,
        port_table::PORT_TABLE_EP_VAL,
        port_table::PORT_TABLE_REMOVE,
        port_table::PORT_TABLE_REMOVE_GLYPH,
        port_table::PORT_TABLE_ADDER,
        port_table::PORT_TABLE_PROTO,
        port_table::PORT_TABLE_PORT_INPUT,
        port_table::PORT_TABLE_MID_ARROW,
        port_table::PORT_TABLE_EP_INPUT,
        port_table::PORT_TABLE_ADD_GLYPH,
        port_table::PORT_TABLE_HINT,
        profile_card::PROFILE_CARD,
        profile_card::PROFILE_CARD_ACTIVE,
        profile_card::PROFILE_CARD_IDLE,
        profile_card::PROFILE_CARD_HEAD,
        profile_card::PROFILE_CARD_ID,
        profile_card::PROFILE_CARD_CHIP,
        profile_card::PROFILE_CARD_CHIP_ACTIVE,
        profile_card::PROFILE_CARD_CHIP_IDLE,
        profile_card::PROFILE_CARD_GLYPH,
        profile_card::PROFILE_CARD_NAME,
        profile_card::PROFILE_CARD_DEFAULT_PILL,
        profile_card::PROFILE_CARD_RADIO,
        profile_card::PROFILE_CARD_RADIO_ACTIVE,
        profile_card::PROFILE_CARD_RADIO_IDLE,
        profile_card::PROFILE_CARD_CHECK,
        profile_card::PROFILE_CARD_LINE,
        profile_card::PROFILE_CARD_FLOW,
        hero::HERO,
        hero::HERO_CONTENT,
        hero::HERO_BADGE,
        hero::HERO_BADGE_SEP,
        hero::HERO_H1,
        hero::HERO_SUB,
        hero::HERO_CTAS,
        hero::HERO_CTA_QUIET,
        hero::HERO_CTA_ICON,
        hero::HERO_BULLETS,
        hero::HERO_BULLET_ICON,
        hero_diagram::HERO_DIAGRAM,
        feature_grid::FGRID,
        feature_grid::FGRID_GRID,
        feature_grid::FGRID_CELL,
        feature_grid::FGRID_ICON,
        feature_grid::FGRID_H3,
        feature_grid::FGRID_P,
        final_cta::FCTA,
        final_cta::FCTA_CARD,
        final_cta::FCTA_H2,
        final_cta::FCTA_P,
        final_cta::FCTA_ROW,
        final_cta::FCTA_QUIET,
        final_cta::FCTA_ICON,
        how_it_works::HIW,
        how_it_works::HIW_HEAD,
        how_it_works::HIW_EYEBROW,
        how_it_works::HIW_H2,
        how_it_works::HIW_GRID,
        how_it_works::HIW_CARD,
        how_it_works::HIW_CARD_HEAD,
        how_it_works::HIW_NUM,
        how_it_works::HIW_ICON,
        how_it_works::HIW_H3,
        how_it_works::HIW_P,
        marketing_footer::MFOOT,
        marketing_footer::MFOOT_GRID,
        marketing_footer::MFOOT_BRAND,
        marketing_footer::MFOOT_TAGLINE,
        marketing_footer::MFOOT_SOCIAL,
        marketing_footer::MFOOT_SOCIAL_ICON,
        marketing_footer::MFOOT_GROUP,
        marketing_footer::MFOOT_GROUP_TITLE,
        marketing_footer::MFOOT_LINK,
        marketing_footer::MFOOT_BOTTOM,
        marketing_footer::MFOOT_BUILD,
        auth_shell::AUTH,
        auth_shell::AUTH_COL,
        auth_shell::AUTH_CENTER,
        auth_shell::AUTH_SLOT,
        auth_shell::AUTH_FOOT,
        auth_shell::AUTH_FOOT_LINKS,
        auth_shell::AUTH_FOOT_LINK,
        auth_shell::AUTH_HEAD,
        auth_shell::AUTH_H1,
        auth_shell::AUTH_SUB,
        auth_side::ASIDE,
        auth_side::ASIDE_SVG,
        auth_side::ASIDE_CONTENT,
        auth_side::ASIDE_KICKER,
        auth_side::ASIDE_H2,
        auth_side::ASIDE_P,
        auth_side::ASIDE_CHIPS,
        auth_side::ASIDE_CHIP,
        auth_side::ASIDE_CHIP_ICON,
        labeled_input::LINPUT,
        labeled_input::LINPUT_FIELD,
        labeled_input::LINPUT_BOX,
        labeled_input::LINPUT_BOX_AFTER,
        labeled_input::LINPUT_AFTER,
        legal_page::LEGAL_HERO,
        legal_page::LEGAL_BADGE,
        legal_page::LEGAL_H1,
        legal_page::LEGAL_EFFECTIVE,
        legal_page::LEGAL_INTRO,
        legal_page::LEGAL_BODY,
        legal_page::LEGAL_TOC,
        legal_page::LEGAL_TOC_TITLE,
        legal_page::LEGAL_TOC_LIST,
        legal_page::LEGAL_TOC_LINK,
        legal_page::LEGAL_TOC_NUM,
        legal_page::LEGAL_ARTICLE,
        legal_page::LEGAL_SEC,
        legal_page::LEGAL_SEC_H2,
        legal_page::LEGAL_SEC_NUM,
        legal_page::LEGAL_SEC_BODY,
        marketing_nav::MNAV,
        marketing_nav::MNAV_ROW,
        marketing_nav::MNAV_DESKTOP,
        marketing_nav::MNAV_GROUP,
        marketing_nav::MNAV_PRODUCT_LINK,
        marketing_nav::MNAV_CARET,
        marketing_nav::MNAV_DROPWRAP,
        marketing_nav::MNAV_DROPPANEL,
        marketing_nav::MNAV_DROPITEM,
        marketing_nav::MNAV_DROPLABEL,
        marketing_nav::MNAV_DROPDESC,
        marketing_nav::MNAV_LINK,
        marketing_nav::MNAV_LINK_ACTIVE,
        marketing_nav::MNAV_SPACER,
        marketing_nav::MNAV_ACTIONS,
        marketing_nav::MNAV_BURGER,
        marketing_nav::MNAV_BURGER_ICON,
        marketing_nav::MNAV_SHEET,
        marketing_nav::MNAV_SHEET_TITLE,
        marketing_nav::MNAV_SHEET_GRID,
        marketing_nav::MNAV_SHEET_LINK,
        marketing_nav::MNAV_SHEET_NAV,
        marketing_nav::MNAV_SHEET_NAVLINK,
        marketing_nav::MNAV_SHEET_ACTIONS,
        mkt_section::MKTS,
        mkt_section::MKTS_INNER,
        mkt_section::MKTS_HEAD,
        mkt_section::MKTS_KICKER,
        mkt_section::MKTS_TITLE,
        mkt_section::MKTS_SUB,
        product_diagrams::IPMOCK,
        product_diagrams::IPMOCK_ROW,
        product_diagrams::IPMOCK_LABEL,
        product_diagrams::IPMOCK_PILL,
        product_diagrams::IPMOCK_DOT,
        product_diagrams::IPMOCK_IP_ROW,
        product_diagrams::IPMOCK_IP,
        product_diagrams::IPMOCK_PORTS,
        product_diagrams::IPMOCK_PORT,
        product_diagrams::IPMOCK_HR,
        product_diagrams::IPMOCK_KV,
        product_diagrams::IPMOCK_KEY,
        product_diagrams::IPMOCK_MONO,
        product_kit::MKT_BETA,
        product_kit::MKT_DOT,
        product_kit::KIT_HERO,
        product_kit::KIT_HERO_GRID,
        product_kit::KIT_HERO_GRID_VISUAL,
        product_kit::KIT_HERO_COL,
        product_kit::KIT_HERO_KICKER,
        product_kit::KIT_HERO_H1,
        product_kit::KIT_HERO_SUB,
        product_kit::KIT_HERO_CTAS,
        product_kit::KIT_FEAT,
        product_kit::KIT_FEAT_ICON,
        product_kit::KIT_FEAT_H3,
        product_kit::KIT_FEAT_P,
        product_kit::KIT_TWOCOL,
        product_kit::KIT_PROS,
        product_kit::KIT_PROS_GLYPH,
        product_kit::KIT_PROS_H,
        product_kit::KIT_PROS_P,
        product_kit::KIT_PRICE,
        product_kit::KIT_PRICE_POP,
        product_kit::KIT_PRICE_NAME,
        product_kit::KIT_PRICE_ROW,
        product_kit::KIT_PRICE_VALUE,
        product_kit::KIT_PRICE_PER,
        product_kit::KIT_PRICE_SUB,
        product_kit::KIT_TIER,
        product_kit::KIT_TIER_ICON,
        product_kit::KIT_TIER_NAME,
        product_kit::KIT_TIER_LINE,
        product_kit::KIT_TIER_DIAGRAM,
        product_kit::KIT_TIER_COST,
        product_kit::KIT_FILTER,
        product_kit::KIT_FILTER_HEAD,
        product_kit::KIT_FILTER_SLUG,
        product_kit::KIT_FILTER_COUNT,
        product_kit::KIT_FILTER_DESC,
        product_kit::KIT_TERM,
        product_kit::KIT_TERM_BAR,
        product_kit::KIT_TERM_DOT,
        product_kit::KIT_TERM_TITLE,
        product_kit::KIT_TERM_PRE,
        product_kit::KIT_TERM_PROMPT,
        product_kit::KIT_TERM_PAD,
        product_kit::KIT_TERM_CMD,
        product_kit::KIT_TERM_OUT,
        quote_or_pricing::QOP,
        quote_or_pricing::QOP_ROW,
        quote_or_pricing::QOP_PRICING,
        quote_or_pricing::QOP_EYEBROW,
        quote_or_pricing::QOP_GRID,
        quote_or_pricing::QOP_PLAN,
        quote_or_pricing::QOP_POP,
        quote_or_pricing::QOP_PLAN_NAME,
        quote_or_pricing::QOP_PRICE_ROW,
        quote_or_pricing::QOP_PRICE,
        quote_or_pricing::QOP_PER,
        quote_or_pricing::QOP_PLAN_SUB,
        quote_or_pricing::QOP_FINE,
        quote_or_pricing::QOP_QUOTE,
        quote_or_pricing::QOP_QUOTE_ICON,
        quote_or_pricing::QOP_QUOTE_P,
        quote_or_pricing::QOP_ATTEST,
        quote_or_pricing::QOP_AVATAR,
        quote_or_pricing::QOP_WHO,
        quote_or_pricing::QOP_NAME,
        quote_or_pricing::QOP_ROLE,
        trust_strip::TRUST_STRIP,
        trust_strip::TRUST_STRIP_ROW,
        trust_strip::TRUST_STRIP_ITEM,
        trust_strip::TRUST_STRIP_ICON,
        profile_picker::PICKER_HEAD,
        profile_picker::PICKER_TITLE,
        profile_picker::PICKER_SUB,
        profile_picker::PICKER_GRID,
        profile_picker::PICKER_TOGGLE,
        profile_picker::PICKER_TOGGLE_ON,
        profile_picker::PICKER_TOGGLE_OFF,
        profile_picker::PICKER_TOGGLE_ICON,
        profile_picker::PICKER_TOGGLE_ICON_ON,
        profile_picker::PICKER_TOGGLE_ICON_OFF,
        profile_picker::PICKER_TOGGLE_COL,
        profile_picker::PICKER_TOGGLE_TITLE,
        profile_picker::PICKER_TOGGLE_SUB,
        profile_picker::PICKER_TOGGLE_ARROW,
        progress_rail::PROGRESS_RAIL,
        progress_rail::PROGRESS_RAIL_GRID,
        progress_rail::PROGRESS_RAIL_STEP,
        progress_rail::PROGRESS_RAIL_BAR,
        progress_rail::PROGRESS_RAIL_BAR_DONE,
        progress_rail::PROGRESS_RAIL_BAR_ACTIVE,
        progress_rail::PROGRESS_RAIL_BAR_TODO,
        progress_rail::PROGRESS_RAIL_META,
        progress_rail::PROGRESS_RAIL_CHECK,
        progress_rail::PROGRESS_RAIL_NUM,
        progress_rail::PROGRESS_RAIL_NUM_ACTIVE,
        progress_rail::PROGRESS_RAIL_NUM_TODO,
        progress_rail::PROGRESS_RAIL_LABEL_ACTIVE,
        progress_rail::PROGRESS_RAIL_LABEL_DONE,
        progress_rail::PROGRESS_RAIL_LABEL_TODO,
        protected_badge::PROTECTED_BADGE,
        protected_badge::PROTECTED_BADGE_SHIELD,
        protected_badge::PROTECTED_BADGE_BODY,
        protected_badge::PROTECTED_BADGE_TITLE,
        protected_badge::PROTECTED_BADGE_DETAIL,
        protected_badge::PROTECTED_BADGE_TAG,
        protected_badge::PROTECTED_BADGE_LOCK,
        radio_group::RADIO_GROUP,
        radio_group::RADIO,
        radio_group::RADIO_CHECKED,
        radio_group::RADIO_INDICATOR,
        radio_group::RADIO_DOT,
        segmented_tabs::SEG_TABS,
        segmented_tabs::SEG_TABS_BTN,
        segmented_tabs::SEG_TABS_BTN_ACTIVE,
        segmented_tabs::SEG_TABS_BTN_IDLE,
        separator::SEPARATOR,
        separator::SEPARATOR_H,
        separator::SEPARATOR_V,
        switch::SWITCH,
        switch::SWITCH_CHECKED,
        switch::SWITCH_UNCHECKED,
        switch::SWITCH_THUMB,
        switch::SWITCH_THUMB_CHECKED,
        switch::SWITCH_THUMB_UNCHECKED,
        skeleton::SKEL,
        skeleton::SKEL_SHIMMER,
        skeleton::SKEL_PULSE,
        skeleton::SKEL_TABLE,
        skeleton::SKEL_TABLE_HEAD,
        skeleton::SKEL_TABLE_HCELL,
        skeleton::SKEL_TABLE_BODY,
        skeleton::SKEL_TABLE_ROW,
        skeleton::SKEL_TABLE_CELL,
        skeleton::SKEL_CARDS,
        skeleton::SKEL_CARDS_CARD,
        skeleton::SKEL_CARDS_LABEL,
        skeleton::SKEL_CARDS_VALUE,
        skeleton::SKEL_CARDS_BAR,
        tabs::TABS_LIST,
        tabs::TABS_TRIGGER,
        tabs::TABS_TRIGGER_ACTIVE,
        tabs::TABS_CONTENT,
        activation_checklist::CHECK_CARD,
        activation_checklist::CHECK_HEAD,
        activation_checklist::CHECK_H2,
        activation_checklist::CHECK_SUB,
        activation_checklist::CHECK_LIST,
        activation_checklist::CHECK_ROW,
        activation_checklist::CHECK_MARKER,
        activation_checklist::CHECK_MARKER_DONE,
        activation_checklist::CHECK_MARKER_ICON,
        activation_checklist::CHECK_LABEL,
        activation_checklist::CHECK_LABEL_DONE,
        activation_checklist::CHECK_CTA_ICON,
        family_profile_picker::FPP_GROUP,
        family_profile_picker::FPP_OPTION,
        family_profile_picker::FPP_OPTION_ACTIVE,
        family_profile_picker::FPP_RADIO,
        family_profile_picker::FPP_ICON,
        family_profile_picker::FPP_COL,
        family_profile_picker::FPP_NAME,
        family_profile_picker::FPP_DESC,
    ];
    #[cfg(feature = "overlays")]
    classes.extend([
        acl_dialogs::ACL_FIELD,
        acl_dialogs::ACL_LABEL,
        acl_dialogs::ACL_COL,
        acl_dialogs::ACL_GRID2,
        acl_dialogs::ACL_RESULT,
        acl_dialogs::ACL_RESULT_HEAD,
        acl_dialogs::ACL_RESULT_LABEL,
        acl_dialogs::ACL_PREVIEW,
        acl_dialogs::ACL_RESULT_P,
        acl_dialogs::ACL_ERROR,
        alert_dialog::ALERT_DIALOG,
        alert_dialog::ALERT_DIALOG_HEADER,
        alert_dialog::ALERT_DIALOG_FOOTER,
        alert_dialog::ALERT_DIALOG_TITLE,
        alert_dialog::ALERT_DIALOG_DESCRIPTION,
        binding_dialogs::BINDR_FIELD,
        binding_dialogs::BINDR_LABEL,
        binding_dialogs::BINDR_COL,
        binding_dialogs::BINDR_NOTE,
        binding_dialogs::BINDR_ERROR,
        fleet_dialogs::FLEET_FIELD,
        fleet_dialogs::FLEET_LABEL,
        fleet_dialogs::FLEET_COL,
        fleet_dialogs::FLEET_GRID2,
        fleet_dialogs::FLEET_GRID2_END,
        fleet_dialogs::FLEET_CHECK,
        fleet_dialogs::FLEET_KEYBOX,
        fleet_dialogs::FLEET_KEY,
        fleet_dialogs::FLEET_ERROR,
        fleet_dialogs::FLEET_BTN_DANGER,
        fleet_pet_dialogs::PET_FIELD,
        fleet_pet_dialogs::PET_LABEL,
        fleet_pet_dialogs::PET_COL,
        fleet_pet_dialogs::PET_ERROR,
        fleet_rollout_dialogs::ROLL_FIELD,
        fleet_rollout_dialogs::ROLL_LABEL,
        fleet_rollout_dialogs::ROLL_COL,
        fleet_rollout_dialogs::ROLL_ERROR,
        dialog::DIALOG_OVERLAY,
        dialog::DIALOG,
        dialog::DIALOG_CLOSE,
        dialog::DIALOG_CLOSE_GLYPH,
        dialog::DIALOG_HEADER,
        dialog::DIALOG_HEADER_ICON,
        dialog::DIALOG_CHIP,
        dialog::DIALOG_HEADER_COL,
        dialog::DIALOG_FOOTER,
        dialog::DIALOG_TITLE,
        dialog::DIALOG_DESCRIPTION,
        settings_forms::SETF_CARD_MSG,
        settings_forms::SETF_CARD_ERR,
        settings_forms::SETF_CARD,
        settings_forms::SETF_FORM,
        settings_forms::SETF_ERR,
        settings_forms::SETF_SAVE_ROW,
        settings_forms::SETF_SAVED,
        settings_forms::SETF_SAVED_ICON,
        settings_forms::SETF_FIELD,
        settings_forms::SETF_TOGGLE_ROW,
        settings_forms::SETF_SECRET_HEAD,
        settings_forms::SETF_PRESENCE,
        settings_forms::SETF_EMAIL_COL,
        settings_forms::SETF_TEST_TITLE,
        settings_forms::SETF_TEST_FORM,
        settings_forms::SETF_TEST_ERR,
        settings_forms::SETF_TEST_OK,
        sidebar::SIDE,
        sidebar::SIDE_HEAD,
        sidebar::SIDE_SCROLL,
        sidebar::SIDE_GROUP,
        sidebar::SIDE_GROUP_DIM,
        sidebar::SIDE_LINK,
        sidebar::SIDE_LINK_ACTIVE,
        sidebar::SIDE_BAR,
        sidebar::SIDE_ICON,
        sidebar::SIDE_ICON_ACTIVE,
        sidebar::SIDE_LABEL,
        sidebar::SIDE_RULE,
        sidebar::SIDE_PLAT_ROW,
        sidebar::SIDE_PLAT_LABEL,
        sidebar::SIDE_PILL,
        sidebar::SIDE_FOOT,
        sidebar::SIDE_AVATAR,
        sidebar::SIDE_ID_COL,
        sidebar::SIDE_NAME,
        sidebar::SIDE_SECONDARY,
        sidebar::SIDE_MORE,
        sidebar::SIDE_MENU,
        sidebar::SIDE_MENU_LINK,
        sidebar::SIDE_MENU_ICON,
        sidebar::SIDE_MENU_TRUNC,
        billing_actions::BILL_COL,
        billing_actions::BILL_GRID2,
        billing_actions::BILL_LABEL,
        billing_actions::BILL_FIELD,
        billing_actions::BILL_PRICE,
        billing_actions::BILL_PRICE_NUM,
        billing_actions::BILL_ERR,
        billing_actions::BILL_QUOTE,
        billing_actions::BILL_QUOTE_ROW,
        billing_actions::BILL_QUOTE_KEY,
        billing_actions::BILL_QUOTE_AMT,
        billing_actions::BILL_QUOTE_ADDR_COL,
        billing_actions::BILL_QUOTE_ADDR,
        billing_actions::BILL_QUOTE_EXP,
        dns_filter_dialog::DNSF_COL,
        dns_filter_dialog::DNSF_GRID2,
        dns_filter_dialog::DNSF_LABEL,
        dns_filter_dialog::DNSF_FIELD,
        dns_filter_dialog::DNSF_CHECK_ROW,
        dns_filter_dialog::DNSF_ERR,
        ip_bind_dialog::IPBIND_COL,
        ip_bind_dialog::IPBIND_LABEL,
        ip_bind_dialog::IPBIND_FIELD,
        ip_bind_dialog::IPBIND_IP,
        ip_bind_dialog::IPBIND_EMPTY,
        ip_bind_dialog::IPBIND_ERR,
        subscription_dialogs::SUBG_COL,
        subscription_dialogs::SUBG_GRID2,
        subscription_dialogs::SUBG_LABEL,
        subscription_dialogs::SUBG_FIELD,
        subscription_dialogs::SUBG_ERR,
        token_create_dialog::TOKC_CONTENT,
        token_create_dialog::TOKC_BODY,
        token_create_dialog::TOKC_FIELD,
        token_create_dialog::TOKC_SCOPE_HEAD,
        token_create_dialog::TOKC_COUNT,
        token_create_dialog::TOKC_LIST,
        token_create_dialog::TOKC_GROUP,
        token_create_dialog::TOKC_AREA,
        token_create_dialog::TOKC_AREA_NAME,
        token_create_dialog::TOKC_WILDCARD,
        token_create_dialog::TOKC_ACTIONS,
        token_create_dialog::TOKC_ACTION,
        token_create_dialog::TOKC_SLUG,
        token_create_dialog::TOKC_NOTE,
        token_create_dialog::TOKC_EMPTY,
        token_create_dialog::TOKC_CAP,
        token_create_dialog::TOKC_ERR,
        token_create_dialog::TOKC_REVEAL,
        token_create_dialog::TOKC_SECRET,
        token_create_dialog::TOKC_KEY_ICO,
        token_create_dialog::TOKC_PLAINTEXT,
        token_create_dialog::TOKC_TOKEN_ID,
        token_create_dialog::TOKC_TRIGGER_ICO,
        token_create_dialog::TOKC_COPY_ICO,
        notification_prefs::NPREF_DIALOG,
        notification_prefs::NPREF_SCROLL,
        notification_prefs::NPREF_TABLE,
        notification_prefs::NPREF_HEADROW,
        notification_prefs::NPREF_TH_EVENT,
        notification_prefs::NPREF_TH_CH,
        notification_prefs::NPREF_ROW_RULED,
        notification_prefs::NPREF_TD_EVENT,
        notification_prefs::NPREF_TD_CH,
        notification_prefs::NPREF_LOADING,
        notification_prefs::NPREF_LOAD_ERR,
        notification_prefs::NPREF_SAVE_ERR,
        topbar::TOP,
        topbar::TOP_BURGER,
        topbar::TOP_BURGER_ICON,
        topbar::TOP_CLUSTER,
        topbar::TOP_WS,
        topbar::TOP_WS_ICON,
        topbar::TOP_WS_BTN,
        topbar::TOP_PILL,
        topbar::TOP_PILL_ICON,
        topbar::TOP_CRUMBS,
        topbar::TOP_CRUMB,
        topbar::TOP_CRUMB_LAST,
        topbar::TOP_CRUMB_ARROW,
        topbar::TOP_SPACER,
        topbar::TOP_SEARCH,
        topbar::TOP_SEARCH_ICON,
        topbar::TOP_SEARCH_TEXT,
        topbar::TOP_KBD,
        topbar::TOP_CMD,
        topbar::TOP_CMD_ICON,
        topbar::TOP_ICONBTN,
        topbar::TOP_ICONBTN_REL,
        topbar::TOP_BTN_ICON,
        topbar::TOP_DOT,
        topbar::TOP_MENU,
        topbar::TOP_ORG_ICON,
        topbar::TOP_ORG_NAME,
        topbar::TOP_ORG_CHECK,
        customize_panel::CPANEL,
        customize_panel::CPANEL_CARD,
        customize_panel::CPANEL_CARD_HEAD,
        customize_panel::CPANEL_HEAD_COL,
        customize_panel::CPANEL_HEAD_TITLE,
        customize_panel::CPANEL_HEAD_SUB,
        customize_panel::CPANEL_EMPTY,
        customize_panel::CPANEL_ROW,
        customize_panel::CPANEL_ROW_DIVIDED,
        customize_panel::CPANEL_DRAG,
        customize_panel::CPANEL_CHECKBOX,
        customize_panel::CPANEL_W150,
        customize_panel::CPANEL_PREP,
        customize_panel::CPANEL_DEL,
        customize_panel::CPANEL_DEL_GLYPH,
        customize_panel::CPANEL_FOOT,
        customize_panel::CPANEL_ADD_GLYPH,
        customize_panel::CPANEL_SUM_BODY,
        customize_panel::CPANEL_SUM_EMPTY,
        customize_panel::CPANEL_SENT_ROW,
        customize_panel::CPANEL_SENT_ICON,
        customize_panel::CPANEL_SENT_ICON_OK,
        customize_panel::CPANEL_SENT_ICON_BAD,
        customize_panel::CPANEL_SENT_ICON_INFO,
        customize_panel::CPANEL_SENT,
        customize_panel::CPANEL_SENT_BAD,
        customize_panel::CPANEL_SEP,
        customize_panel::CPANEL_NOTE,
        dropdown_menu::DD,
        dropdown_menu::DD_ITEM,
        dropdown_menu::DD_ITEM_DISABLED,
        dropdown_menu::DD_LABEL,
        dropdown_menu::DD_SEP,
        popover::POPOVER,
        select::SELECT_TRIGGER,
        select::SELECT_TRIGGER_ICON,
        select::SELECT,
        select::SELECT_BOTTOM,
        select::SELECT_TOP,
        select::SELECT_VIEWPORT,
        select::SELECT_ITEM,
        select::SELECT_ITEM_DISABLED,
        select::SELECT_ITEM_CHECK,
        select::SELECT_ITEM_CHECK_GLYPH,
        select::SELECT_LABEL,
        select::SELECT_SEP,
        tooltip::TOOLTIP,
        tooltip::POPPER,
        tooltip::VISUALLY_HIDDEN,
    ]);
    #[cfg(feature = "calendar")]
    classes.extend([
        calendar::CAL,
        calendar::CAL_MONTHS,
        calendar::CAL_MONTH,
        calendar::CAL_CAPTION,
        calendar::CAL_CAPTION_LABEL,
        calendar::CAL_NAV,
        calendar::CAL_NAV_BTN,
        calendar::CAL_CHEVRON,
        calendar::CAL_GRID,
        calendar::CAL_WEEKDAYS,
        calendar::CAL_WEEKDAY,
        calendar::CAL_WEEK,
        calendar::CAL_DAY,
        calendar::CAL_DAY_TODAY,
        calendar::CAL_DAY_OUTSIDE,
        calendar::CAL_DAY_BTN,
    ]);
    #[cfg(feature = "command")]
    classes.extend([
        command::CMD,
        command::CMD_INPUT_WRAP,
        command::CMD_INPUT_ICON,
        command::CMD_INPUT,
        command::CMD_LIST,
        command::CMD_EMPTY,
        command::CMD_GROUP,
        command::CMD_GROUP_HEADING,
        command::CMD_ITEM,
        command::CMD_SEP,
    ]);
    #[cfg(all(feature = "command", feature = "overlays"))]
    classes.extend([
        dash_shell::SHELL,
        dash_shell::SHELL_DESKTOP_SIDE,
        dash_shell::SHELL_DRAWER,
        dash_shell::SHELL_SCRIM,
        dash_shell::SHELL_DRAWER_PANEL,
        dash_shell::SHELL_MAIN_COL,
        dash_shell::SHELL_MAIN,
        dash_shell::PAGEHEAD,
        dash_shell::PAGEHEAD_COL,
        dash_shell::PAGEHEAD_ROW,
        dash_shell::PAGEHEAD_H1,
        dash_shell::PAGEHEAD_SUB,
        dash_shell::PAGEHEAD_ACTIONS,
        command_palette::CMDP,
        command_palette::CMDP_TITLE,
        command_palette::CMDP_ICON,
        command_palette::CMDP_LABEL,
        command_palette::CMDP_HINT,
    ]);
    #[cfg(feature = "toast")]
    classes.extend([
        toast::TOASTER,
        toast::TOAST,
        toast::TOAST_ICON,
        toast::TOAST_CONTENT,
        toast::TOAST_TITLE,
        toast::TOAST_DESC,
    ]);
    #[cfg(feature = "charts")]
    classes.extend([
        topology::NETMAP,
        topology::NETMAP_SVG,
        topology::NETMAP_HUD,
        topology::NETMAP_HUD_LINE,
        topology::NETMAP_HUD_SUB,
        topology::NETMAP_LIVE,
        topology::NETMAP_LIVE_DOT,
        topology::NETMAP_PANEL,
        topology::NETMAP_PANEL_COL,
        topology::NETMAP_CHIPS,
        topology::NETMAP_FLOW,
        topology::NETMAP_FLOW_SLOW,
        topology::NETMAP_FLOW_MED,
        topology::NETMAP_FLOW_FAST,
        topology::NETMAP_FLOW_IDLE,
        topology::NETMAP_PULSE,
        area_chart::AREA_CHART,
        bar_list::BAR_LIST,
        bar_list::BAR_LIST_ROW,
        bar_list::BAR_LIST_LABEL,
        bar_list::BAR_LIST_TRACK,
        bar_list::BAR_LIST_FILL,
        bar_list::BAR_GROW,
        bar_list::BAR_LIST_VALUE,
        ring::RING,
        ring::RING_ANIM,
        ring::RING_COL,
        ring::RING_LABEL,
        ring::RING_VALUE,
        ring::RING_SUB,
        sparkline::SPARKLINE,
        sparkline::CHART_DRAW,
        screenshot_section::SHOT,
        screenshot_section::SHOT_HEAD,
        screenshot_section::SHOT_EYEBROW,
        screenshot_section::SHOT_H2,
        screenshot_section::SHOT_P,
        screenshot_section::SHOT_FRAME,
        screenshot_section::SHOT_DASH,
        screenshot_section::SHOT_SIDE,
        screenshot_section::SHOT_SIDE_GAP,
        screenshot_section::SHOT_NAVROW,
        screenshot_section::SHOT_NAVROW_ON,
        screenshot_section::SHOT_NAVICON,
        screenshot_section::SHOT_NAVICON_ON,
        screenshot_section::SHOT_MAIN,
        screenshot_section::SHOT_TOP,
        screenshot_section::SHOT_TITLE_COL,
        screenshot_section::SHOT_TITLE,
        screenshot_section::SHOT_SUB,
        screenshot_section::SHOT_ORG_BADGE,
        screenshot_section::SHOT_KPIS,
        screenshot_section::SHOT_KPI,
        screenshot_section::SHOT_KPI_LABEL,
        screenshot_section::SHOT_KPI_VALUE,
        screenshot_section::SHOT_CHART,
        screenshot_section::SHOT_CHART_HEAD,
        screenshot_section::SHOT_CHART_TITLE,
        screenshot_section::SHOT_CHART_SUB,
        screenshot_section::SHOT_HITS,
        screenshot_section::SHOT_HITS_TITLE,
        screenshot_section::SHOT_HITS_COL,
        screenshot_section::SHOT_HIT,
        screenshot_section::SHOT_HIT_LEFT,
        screenshot_section::SHOT_HIT_BADGE,
        screenshot_section::SHOT_HIT_WHO,
        screenshot_section::SHOT_HIT_ARROW,
        screenshot_section::SHOT_HIT_WHERE,
        screenshot_section::SHOT_HIT_WHEN,
        screenshot_section::SHOT_CALLOUT,
        screenshot_section::SHOT_CALLOUT_DOT,
        screenshot_section::SHOT_CALLOUT_TEXT,
    ]);
    classes
}
