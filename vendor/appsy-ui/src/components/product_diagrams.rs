//! Product-page diagram visuals — port of `marketing/product-diagrams.tsx`:
//! `DiagramClientPopExit` (VPN hero topology fan), `ContractorDenialDiagram`
//! (Devices denial path), `MiniPath` (tier-card path strip), `IPMockup`
//! (Static-IPs hero card).
//!
//! The two big diagrams are attribute-styled SVG with no classes; every
//! coordinate the reference derives from its data arrays is embedded as the
//! JS-serialized string (integer arithmetic here, so plain integers).
//! `MiniPath` computes coordinates from its `nodes` prop at render time —
//! the arithmetic mirrors the JSX op-for-op so the IEEE results are
//! bit-identical, and Rust's shortest-roundtrip `Display` serializes them
//! exactly like JS `String(number)` in this range (integers print bare).

use leptos::prelude::*;

pub const IPMOCK: &str = "asy-ipmock";
pub const IPMOCK_ROW: &str = "asy-ipmock__row";
pub const IPMOCK_LABEL: &str = "asy-ipmock__label";
pub const IPMOCK_PILL: &str = "asy-ipmock__pill";
pub const IPMOCK_DOT: &str = "asy-ipmock__dot";
pub const IPMOCK_IP_ROW: &str = "asy-ipmock__ip-row";
pub const IPMOCK_IP: &str = "asy-ipmock__ip";
pub const IPMOCK_PORTS: &str = "asy-ipmock__ports";
pub const IPMOCK_PORT: &str = "asy-ipmock__port";
pub const IPMOCK_HR: &str = "asy-ipmock__hr";
pub const IPMOCK_KV: &str = "asy-ipmock__kv";
pub const IPMOCK_KEY: &str = "asy-ipmock__key";
pub const IPMOCK_MONO: &str = "asy-ipmock__mono";

/// (label, y, rect_y = y-14, text_y = y+3) per client row.
const DCP_CLIENTS: [(&str, &str, &str, &str); 4] = [
    ("laptop", "60", "46", "63"),
    ("phone", "140", "126", "143"),
    ("ci", "220", "206", "223"),
    ("nas", "300", "286", "303"),
];

/// (y, rect_y = y-14, label_y = y-1, sub_y = y+10, label, sub, dash,
/// stroke_width) per exit row; the first row is the solid/bold one.
const DCP_EXITS: [(&str, &str, &str, &str, &str, &str, &str, &str); 4] = [
    ("60", "46", "59", "70", "203.0.113.41", "dedicated · stable", "0", "1.6"),
    ("150", "136", "149", "160", "appsynergy fabric", "Simple tier", "3 3", "1"),
    ("230", "216", "229", "240", "tor / partner", "Privacy tier", "3 3", "1"),
    ("310", "296", "309", "320", "customer-owned", "BYO exit", "3 3", "1"),
];

/// client → PoP → exit topology fan (VPN product hero).
#[component]
pub fn DiagramClientPopExit() -> impl IntoView {
    view! {
        <svg viewBox="0 0 540 360" width="100%" style="max-height:360px">
            <defs>
                <radialGradient id="diag-fade" cx="50%" cy="50%" r="50%">
                    <stop offset="0%" stop-color="var(--color-accent)" stop-opacity="0.18"></stop>
                    <stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0"></stop>
                </radialGradient>
            </defs>
            <circle cx="270" cy="180" r="170" fill="url(#diag-fade)"></circle>
            {DCP_CLIENTS
                .iter()
                .map(|(label, y, rect_y, text_y)| {
                    view! {
                        <g>
                            <rect
                                x="50"
                                y=*rect_y
                                width="64"
                                height="28"
                                rx="4"
                                fill="var(--color-surface)"
                                stroke="var(--color-accent)"
                                stroke-width="1.2"
                            ></rect>
                            <text
                                x="82"
                                y=*text_y
                                font-size="9"
                                fill="var(--color-text-muted)"
                                text-anchor="middle"
                                font-family="var(--font-mono)"
                            >
                                {*label}
                            </text>
                            <line
                                x1="114"
                                y1=*y
                                x2="240"
                                y2="180"
                                stroke="var(--color-accent)"
                                stroke-width="0.8"
                                opacity="0.6"
                            ></line>
                        </g>
                    }
                })
                .collect_view()}
            <g>
                <circle
                    cx="270"
                    cy="180"
                    r="48"
                    fill="var(--color-bg)"
                    stroke="var(--color-accent)"
                    stroke-width="1.6"
                    stroke-dasharray="3 3"
                ></circle>
                <circle
                    cx="270"
                    cy="180"
                    r="22"
                    fill="var(--color-accent-soft)"
                    stroke="var(--color-accent)"
                    stroke-width="1.4"
                ></circle>
                <circle cx="270" cy="180" r="8" fill="var(--color-accent)"></circle>
                <text
                    x="270"
                    y="240"
                    font-size="11"
                    fill="var(--color-text-muted)"
                    text-anchor="middle"
                    font-family="var(--font-mono)"
                >
                    "fra-1"
                </text>
                <text x="270" y="254" font-size="9" fill="var(--color-text-dim)" text-anchor="middle">
                    "policy + DNAT + audit"
                </text>
            </g>
            {DCP_EXITS
                .iter()
                .map(|(y, rect_y, label_y, sub_y, label, sub, dash, sw)| {
                    view! {
                        <g>
                            <line
                                x1="318"
                                y1="180"
                                x2="430"
                                y2=*y
                                stroke="var(--color-accent)"
                                stroke-width="0.8"
                                opacity="0.55"
                                stroke-dasharray=*dash
                            ></line>
                            <rect
                                x="430"
                                y=*rect_y
                                width="90"
                                height="28"
                                rx="4"
                                fill="var(--color-surface)"
                                stroke="var(--color-accent)"
                                stroke-width=*sw
                            ></rect>
                            <text
                                x="475"
                                y=*label_y
                                font-size="9"
                                fill="var(--color-text)"
                                text-anchor="middle"
                                font-family="var(--font-mono)"
                            >
                                {*label}
                            </text>
                            <text
                                x="475"
                                y=*sub_y
                                font-size="8"
                                fill="var(--color-text-dim)"
                                text-anchor="middle"
                            >
                                {*sub}
                            </text>
                        </g>
                    }
                })
                .collect_view()}
        </svg>
    }
}

/// Contractor laptop → PoP policy chain → denied target (Devices product).
#[component]
pub fn ContractorDenialDiagram() -> impl IntoView {
    view! {
        <svg viewBox="0 0 880 240" width="100%" style="max-height:240px">
            <g>
                <rect
                    x="20"
                    y="100"
                    width="120"
                    height="48"
                    rx="6"
                    fill="var(--color-surface)"
                    stroke="var(--color-accent)"
                    stroke-width="1.2"
                ></rect>
                <text
                    x="80"
                    y="124"
                    font-size="11"
                    fill="var(--color-text)"
                    text-anchor="middle"
                    font-family="var(--font-mono)"
                >
                    "contractor-6"
                </text>
                <text x="80" y="138" font-size="9" fill="var(--color-text-muted)" text-anchor="middle">
                    "tag:contractor"
                </text>
            </g>
            <line x1="140" y1="124" x2="290" y2="124" stroke="var(--color-accent)" stroke-width="1.4"></line>
            <polygon points="290,120 300,124 290,128" fill="var(--color-accent)"></polygon>
            <text
                x="215"
                y="115"
                font-size="9"
                fill="var(--color-text-muted)"
                text-anchor="middle"
                font-family="var(--font-mono)"
            >
                "wireguard tunnel"
            </text>
            <g>
                <rect
                    x="300"
                    y="80"
                    width="280"
                    height="100"
                    rx="10"
                    fill="var(--color-surface)"
                    stroke="var(--color-accent)"
                    stroke-width="1.3"
                    stroke-dasharray="3 3"
                ></rect>
                <text
                    x="440"
                    y="106"
                    font-size="11"
                    fill="var(--color-text-muted)"
                    text-anchor="middle"
                    font-family="var(--font-mono)"
                >
                    "PoP fra-1 · policy evaluator"
                </text>
                <g font-size="10" font-family="var(--font-mono)">
                    <text x="320" y="128" fill="var(--color-text-dim)">
                        "#70 tag:contractor → staging.internal:443"
                    </text>
                    <text x="540" y="128" fill="var(--color-success)" text-anchor="end">
                        "allow"
                    </text>
                    <text x="320" y="146" fill="var(--color-text)">
                        "#80 tag:contractor → *.internal"
                    </text>
                    <text x="540" y="146" fill="var(--color-danger)" text-anchor="end">
                        "deny ✕"
                    </text>
                    <text x="320" y="164" fill="var(--color-text-dim)">
                        "#200 * → *.internal"
                    </text>
                    <text x="540" y="164" fill="var(--color-danger)" text-anchor="end">
                        "deny"
                    </text>
                </g>
            </g>
            <line
                x1="580"
                y1="124"
                x2="730"
                y2="124"
                stroke="var(--color-danger)"
                stroke-width="1.4"
                stroke-dasharray="4 4"
                opacity="0.5"
            ></line>
            <g transform="translate(655,124)">
                <circle r="14" fill="var(--color-bg)" stroke="var(--color-danger)" stroke-width="1.4"></circle>
                <line x1="-6" y1="-6" x2="6" y2="6" stroke="var(--color-danger)" stroke-width="1.8"></line>
                <line x1="-6" y1="6" x2="6" y2="-6" stroke="var(--color-danger)" stroke-width="1.8"></line>
            </g>
            <g>
                <rect
                    x="740"
                    y="100"
                    width="120"
                    height="48"
                    rx="6"
                    fill="var(--color-surface)"
                    stroke="var(--color-text-dim)"
                    stroke-width="1.2"
                ></rect>
                <text
                    x="800"
                    y="124"
                    font-size="11"
                    fill="var(--color-text-muted)"
                    text-anchor="middle"
                    font-family="var(--font-mono)"
                >
                    "prod.db:5432"
                </text>
                <text x="800" y="138" font-size="9" fill="var(--color-text-dim)" text-anchor="middle">
                    "never reached"
                </text>
            </g>
        </svg>
    }
}

/// JS `String(number)` for MiniPath's coordinate range: Rust's
/// shortest-roundtrip `Display` produces the identical string (both print
/// the shortest decimal that round-trips; integral values print bare).
fn fmt(v: f64) -> String {
    format!("{v}")
}

/// Small horizontal path of labeled nodes; `direct` colors it success-green.
#[component]
pub fn MiniPath(nodes: Vec<String>, #[prop(optional)] direct: bool) -> impl IntoView {
    let n_count = nodes.len();
    let step = 200.0 / (n_count as f64 - 1.0);
    let stroke = if direct { "var(--color-success)" } else { "var(--color-accent)" };
    view! {
        <svg viewBox="0 0 200 36" width="100%" height="36">
            {nodes
                .into_iter()
                .enumerate()
                .map(|(i, n)| {
                    let node_stroke = if direct && i > 0 {
                        "var(--color-success)"
                    } else {
                        "var(--color-accent)"
                    };
                    let x = fmt(step * i as f64);
                    // Inset end labels so middle-anchored text stays inside the
                    // 200-wide viewBox (M11); circles stay on the true path.
                    let label_x = if i == 0 {
                        "12".to_string()
                    } else if i + 1 == n_count {
                        "188".to_string()
                    } else {
                        x.clone()
                    };
                    view! {
                        <g>
                            {(i > 0)
                                .then(|| {
                                    view! {
                                        <line
                                            x1=fmt(step * (i as f64 - 1.0) + 6.0)
                                            y1="18"
                                            x2=fmt(step * i as f64 - 6.0)
                                            y2="18"
                                            stroke=stroke
                                            stroke-width="1.2"
                                        ></line>
                                    }
                                })}
                            <circle
                                cx=x
                                cy="18"
                                r="4"
                                fill="var(--color-bg)"
                                stroke=node_stroke
                                stroke-width="1.4"
                            ></circle>
                            <text
                                x=label_x
                                y="34"
                                font-size="7.5"
                                fill="var(--color-text-muted)"
                                text-anchor="middle"
                                font-family="var(--font-mono)"
                            >
                                {n}
                            </text>
                        </g>
                    }
                })
                .collect_view()}
        </svg>
    }
}

/// Dedicated-IP card mockup (Static IPs product hero visual). Content is the
/// reference's hardcoded copy (ALLOW-HARDCODE).
#[component]
pub fn IPMockup() -> impl IntoView {
    view! {
        <div class=IPMOCK>
            <div class=IPMOCK_ROW>
                <span class=IPMOCK_LABEL>"your dedicated IP"</span>
                <span class=IPMOCK_PILL>
                    <span class=IPMOCK_DOT></span>
                    " bound"
                </span>
            </div>
            <div class=IPMOCK_IP_ROW>
                <span class=IPMOCK_IP>"203.0.113.74"</span>
            </div>
            <div class=IPMOCK_PORTS>
                <span class=IPMOCK_PORT>"tcp/22"</span>
                <span class=IPMOCK_PORT>"tcp/443"</span>
            </div>
            <div class=IPMOCK_HR></div>
            <div class=IPMOCK_KV>
                <div class=IPMOCK_ROW>
                    <span class=IPMOCK_KEY>"bound to"</span>
                    <span class=IPMOCK_MONO>"ci-runner-3"</span>
                </div>
                <div class=IPMOCK_ROW>
                    <span class=IPMOCK_KEY>"region"</span>
                    <span>"EU-West / fra-1"</span>
                </div>
                <div class=IPMOCK_ROW>
                    <span class=IPMOCK_KEY>"survived"</span>
                    <span class=IPMOCK_MONO>"4 reconnects · 12 d"</span>
                </div>
            </div>
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{root}{{display:flex;flex-direction:column;gap:.875rem;",
            "border-radius:var(--radius-lg);border:1px solid var(--color-border);",
            "background-color:var(--color-surface);padding:1.25rem}}",
            ".{row}{{display:flex;flex-wrap:wrap;align-items:center;",
            "justify-content:space-between;gap:.375rem;min-width:0}}",
            ".{label}{{font-size:11px;font-weight:600;text-transform:uppercase;",
            "letter-spacing:.05em;color:var(--color-text-muted)}}",
            // border-[oklch(70%_0.15_145_/_0.3)] compiles to rgba (TT-2),
            // same serialization as badge ok / protected_badge.
            ".{pill}{{display:inline-flex;align-items:center;gap:.375rem;",
            "border-radius:calc(infinity * 1px);",
            "border:1px solid rgba(91,182,97,.3);",
            "background-color:var(--color-success-soft);",
            "padding-inline:.5rem;padding-block:.125rem;font-size:11px;",
            "color:var(--color-success)}}",
            ".{dot}{{width:.375rem;height:.375rem;",
            "border-radius:calc(infinity * 1px);background-color:currentcolor}}",
            ".{ip_row}{{display:flex;align-items:baseline;gap:.5rem;min-width:0}}",
            ".{ip}{{font-family:var(--font-mono);font-size:28px;font-weight:600;",
            "letter-spacing:-0.01em;color:var(--color-accent);min-width:0;",
            "overflow-wrap:anywhere}}",
            "@media (width < 40rem){{.{ip}{{font-size:20px}}}}",
            ".{ports}{{display:flex;flex-wrap:wrap;gap:.625rem;font-size:12px;min-width:0}}",
            ".{port}{{border-radius:calc(infinity * 1px);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);",
            "padding-inline:.5rem;padding-block:.125rem;",
            "font-family:var(--font-mono)}}",
            ".{hr}{{height:1px;background-color:var(--color-border)}}",
            ".{kv}{{display:flex;flex-direction:column;gap:.375rem;font-size:12px}}",
            ".{key}{{color:var(--color-text-muted)}}",
            ".{mono}{{font-family:var(--font-mono)}}",
        ),
        root = IPMOCK,
        row = IPMOCK_ROW,
        label = IPMOCK_LABEL,
        pill = IPMOCK_PILL,
        dot = IPMOCK_DOT,
        ip_row = IPMOCK_IP_ROW,
        ip = IPMOCK_IP,
        ports = IPMOCK_PORTS,
        port = IPMOCK_PORT,
        hr = IPMOCK_HR,
        kv = IPMOCK_KV,
        key = IPMOCK_KEY,
        mono = IPMOCK_MONO,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            IPMOCK, IPMOCK_ROW, IPMOCK_LABEL, IPMOCK_PILL, IPMOCK_DOT, IPMOCK_IP_ROW, IPMOCK_IP,
            IPMOCK_PORTS, IPMOCK_PORT, IPMOCK_HR, IPMOCK_KV, IPMOCK_KEY, IPMOCK_MONO,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }

    #[test]
    fn diagram_tables_derive_from_reference_y() {
        for (_, y, rect_y, text_y) in DCP_CLIENTS {
            let y: i32 = y.parse().unwrap();
            assert_eq!(rect_y.parse::<i32>().unwrap(), y - 14);
            assert_eq!(text_y.parse::<i32>().unwrap(), y + 3);
        }
        for (y, rect_y, label_y, sub_y, ..) in DCP_EXITS {
            let y: i32 = y.parse().unwrap();
            assert_eq!(rect_y.parse::<i32>().unwrap(), y - 14);
            assert_eq!(label_y.parse::<i32>().unwrap(), y - 1);
            assert_eq!(sub_y.parse::<i32>().unwrap(), y + 10);
        }
    }

    /// The five page node-lists (2/3/4 nodes) serialized exactly as the JS
    /// runtime serializes them — values cross-checked against
    /// `String(200/(n-1) * i)` etc.
    #[test]
    fn minipath_serializes_like_js() {
        let step3 = 200.0 / 2.0;
        assert_eq!(fmt(step3 * 0.0), "0");
        assert_eq!(fmt(step3 * 1.0), "100");
        assert_eq!(fmt(step3 * 1.0 - 6.0), "94");
        let step4 = 200.0 / 3.0;
        assert_eq!(fmt(step4), "66.66666666666667");
        assert_eq!(fmt(step4 * 2.0), "133.33333333333334");
        assert_eq!(fmt(step4 * 2.0 - 6.0), "127.33333333333334");
        assert_eq!(fmt(step4 * 1.0 + 6.0), "72.66666666666667");
        assert_eq!(fmt(step4 * 3.0), "200");
        assert_eq!(fmt(step4 * 3.0 - 6.0), "194");
        let step2 = 200.0 / 1.0;
        assert_eq!(fmt(step2 * 1.0 - 6.0), "194");
    }
}
