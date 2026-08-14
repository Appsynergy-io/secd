//! Topology / network-map (`platform.map.tsx`) — the d3-force port.
//! `force` is the exact physics engine, `graph` the buildGraph mirror
//! and shared helpers; `NetworkMap` below is the SVG render layer.
//!
//! Props/callbacks split: `useTopologyMap`/`useFleetTopologyNudge` →
//! the `map` prop; `useFlowRates` → the `rates` prop (snapshot-ordered
//! pairs — order carries into the float sums like the reference's
//! insertion-ordered `Map`); `Date.now()` → the `now_ms` prop (time is
//! data; determinism forbids a clock in the crate). The route's stage
//! chrome (offline toggle, fullscreen, pop-out, kiosk mode) stays with
//! the consumer — `window.open` targets are navigation. Selection
//! state is internal, like the reference.

pub mod force;
pub mod graph;

use graph::{
    bow, flow_class, format_bytes, fresh, ip_label_at, js_to_fixed, layout, org_label, rate_label,
    LayoutPos,
    LayoutState, TopologyMap, Xy, CX, CY, H, LIVE_HANDSHAKE_MS, LIVE_SEEN_MS, LOSS_WARN_PCT, ORG_R,
    RTT_WARN_MS, SERVER_R, W,
};

use crate::components::badge::{Badge, BadgeTone};
use crate::components::host_metrics_card::to_fixed;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use std::collections::HashMap;

pub const NETMAP: &str = "asy-netmap";
pub const NETMAP_SVG: &str = "asy-netmap__svg";
pub const NETMAP_HUD: &str = "asy-netmap__hud";
pub const NETMAP_HUD_LINE: &str = "asy-netmap__hud-line";
pub const NETMAP_HUD_SUB: &str = "asy-netmap__hud-sub";
pub const NETMAP_LIVE: &str = "asy-netmap__live";
pub const NETMAP_LIVE_DOT: &str = "asy-netmap__live-dot";
pub const NETMAP_PANEL: &str = "asy-netmap__panel";
pub const NETMAP_PANEL_COL: &str = "asy-netmap__panel-col";
pub const NETMAP_CHIPS: &str = "asy-netmap__chips";
pub const NETMAP_FLOW: &str = "asy-netmap__flow";
pub const NETMAP_FLOW_SLOW: &str = "asy-netmap__flow--slow";
pub const NETMAP_FLOW_MED: &str = "asy-netmap__flow--med";
pub const NETMAP_FLOW_FAST: &str = "asy-netmap__flow--fast";
pub const NETMAP_FLOW_IDLE: &str = "asy-netmap__flow--idle";
pub const NETMAP_PULSE: &str = "asy-netmap__pulse";

/// One tunnel's live rate (`useFlowRates` output), bytes/s.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct FlowRate {
    pub in_rate: f64,
    pub out_rate: f64,
}

/// The reference's `flowClass` bucket mapped onto crate classes.
fn flow_class_port(rate: f64) -> &'static str {
    match flow_class(rate) {
        "flow flow-fast" => "asy-netmap__flow asy-netmap__flow--fast",
        "flow flow-med" => "asy-netmap__flow asy-netmap__flow--med",
        "flow flow-slow" => "asy-netmap__flow asy-netmap__flow--slow",
        _ => NETMAP_FLOW_IDLE,
    }
}

/// JS number → string for SVG attribute values (shortest round-trip,
/// which Rust's `Display` shares in the coordinate range).
fn num(v: f64) -> String {
    format!("{v}")
}

/// `new Date(iso).toLocaleTimeString()` — browser locale + timezone,
/// exactly what the reference renders; raw ISO on the server (the
/// HUD is repainted client-side, like the billing quote expiry).
fn locale_time(iso: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let t = js_sys::Date::parse(iso);
        if !t.is_nan() {
            let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(t));
            if let Ok(f) = js_sys::Reflect::get(date.as_ref(), &"toLocaleTimeString".into()) {
                let f: js_sys::Function = f.into();
                if let Ok(s) = f.call0(date.as_ref()) {
                    if let Some(s) = s.as_string() {
                        return s;
                    }
                }
            }
        }
        iso.to_owned()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        iso.to_owned()
    }
}

/// The live system map: every server, agent, customer, tunnel, and
/// device, force-laid-out with real-time flow styling.
#[component]
pub fn NetworkMap(
    #[prop(into)] map: Signal<TopologyMap>,
    /// `(tunnel id, rate)` in snapshot order.
    #[prop(into)]
    rates: Signal<Vec<(String, FlowRate)>>,
    /// Freshness clock (the reference's `Date.now()`).
    now_ms: f64,
) -> impl IntoView {
    let selected = RwSignal::new(None::<String>);
    // One layout state per component lifetime — positions, alpha and
    // the LCG stream persist across snapshots like the reference's
    // simulation ref. SendWrapper: single-threaded render contexts.
    let state =
        StoredValue::new(SendWrapper::new(std::cell::RefCell::new(LayoutState::default())));
    let pos = Memo::new(move |_| {
        map.with(|m| state.with_value(|s| layout(m, &mut s.borrow_mut(), now_ms)))
    });

    let rate_by_id = Memo::new(move |_| {
        rates.with(|r| r.iter().map(|(id, fr)| (id.clone(), *fr)).collect::<HashMap<_, _>>())
    });
    let total_rate =
        move || rates.with(|r| r.iter().fold(0.0, |s, (_, fr)| s + fr.in_rate + fr.out_rate));
    let server_rate = |m: &TopologyMap, by_id: &HashMap<String, FlowRate>, server_id: &str| {
        m.tunnels
            .iter()
            .filter(|t| t.server_id.as_deref() == Some(server_id))
            .fold(0.0, |s, t| by_id.get(&t.id).map_or(s, |r| s + r.in_rate + r.out_rate))
    };

    let svg = move || {
        let m = map.get();
        let by_id = rate_by_id.get();
        let pos: LayoutPos = pos.get();

        // Fit the camera to occupied space (16:9-locked); the server
        // ring is the floor so the view never over-zooms.
        let mut min_x = CX - SERVER_R;
        let mut max_x = CX + SERVER_R;
        let mut min_y = CY - SERVER_R;
        let mut max_y = CY + SERVER_R;
        for p in pos
            .servers
            .values()
            .chain(pos.agents.values())
            .chain(pos.orgs.values())
            .chain(pos.leaves.values())
        {
            if p.x < min_x {
                min_x = p.x;
            }
            if p.x > max_x {
                max_x = p.x;
            }
            if p.y < min_y {
                min_y = p.y;
            }
            if p.y > max_y {
                max_y = p.y;
            }
        }
        const PAD: f64 = 130.0;
        let mut vw = max_x - min_x + 2.0 * PAD;
        let mut vh = max_y - min_y + 2.0 * PAD;
        if vw / vh > W / H {
            vh = vw * H / W;
        } else {
            vw = vh * W / H;
        }
        let vx = (min_x + max_x) / 2.0 - vw / 2.0;
        let vy = (min_y + max_y) / 2.0 - vh / 2.0;

        let spines = m
            .servers
            .iter()
            .filter_map(|s| {
                let p = *pos.servers.get(&s.id)?;
                let rate = server_rate(&m, &by_id, &s.id);
                Some(view! {
                    <path
                        d=bow(p, Xy { x: CX, y: CY }, 0.18)
                        fill="none"
                        stroke="var(--color-accent)"
                        stroke-opacity=if s.status == "online" { "0.45" } else { "0.12" }
                        stroke-width="1.4"
                        class=flow_class_port(rate)
                    ></path>
                })
            })
            .collect_view();

        let edges = m
            .tunnels
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let leaf = *pos.leaves.get(&format!("t:{}", t.id))?;
                let server = *pos.servers.get(t.server_id.as_deref()?)?;
                let rate = by_id.get(&t.id).map_or(0.0, |r| r.in_rate + r.out_rate);
                let live = fresh(t.last_handshake.as_deref(), LIVE_HANDSHAKE_MS, now_ms);
                let stroke =
                    if live { "var(--color-success)" } else { "var(--color-text-muted)" };
                Some(view! {
                    <path
                        d=bow(leaf, server, 0.06 + (i % 4) as f64 * 0.05)
                        fill="none"
                        stroke=stroke
                        stroke-opacity=if live { "0.8" } else { "0.18" }
                        stroke-width=if live { "1.8" } else { "1" }
                        stroke-dasharray=(t.tunnel_type == "quic").then_some("8 4")
                        class=flow_class_port(rate)
                    ></path>
                })
            })
            .collect_view();

        // Org hub ↔ leaf ties: tunnels then devices — publish order,
        // which is how the reference's position map iterates.
        let tie = |key: String, org_id: &str| -> Option<AnyView> {
            let p = *pos.leaves.get(&key)?;
            let hub = *pos.orgs.get(org_id)?;
            Some(
                view! {
                    <line
                        x1=num(hub.x)
                        y1=num(hub.y)
                        x2=num(p.x)
                        y2=num(p.y)
                        stroke="var(--color-border)"
                        stroke-opacity="0.6"
                    ></line>
                }
                .into_any(),
            )
        };
        let ties_t = m
            .tunnels
            .iter()
            .filter_map(|t| tie(format!("t:{}", t.id), &t.org_id))
            .collect_view();
        let ties_d = m
            .devices
            .iter()
            .filter_map(|d| tie(format!("d:{}", d.id), &d.org_id))
            .collect_view();

        let agents = m
            .agents
            .iter()
            .filter_map(|a| {
                let p = *pos.agents.get(&a.id)?;
                let live =
                    a.status == "joined" && fresh(a.last_seen_at.as_deref(), LIVE_SEEN_MS, now_ms);
                let tone = if a.status == "joined" {
                    if live {
                        "var(--color-success)"
                    } else {
                        "var(--color-warn, var(--color-text-muted))"
                    }
                } else {
                    "var(--color-text-muted)"
                };
                let caps = a.capabilities.join(",");
                let title = format!(
                    "agent {} ({}) · {} · v{} · {}",
                    a.name,
                    a.hostname.as_deref().unwrap_or("?"),
                    a.status,
                    a.reported_version.as_deref().unwrap_or("?"),
                    if caps.is_empty() { "no caps" } else { &caps },
                );
                Some(view! {
                    <g opacity=if a.status == "retired" { "0.35" } else { "1" }>
                        <title>{title}</title>
                        <rect
                            x=num(p.x - 9.0)
                            y=num(p.y - 9.0)
                            width="18"
                            height="18"
                            rx="4"
                            fill="var(--color-surface-2)"
                            stroke=tone
                            stroke-width="1.5"
                            class=live.then_some(NETMAP_PULSE)
                        ></rect>
                        <text
                            x=num(p.x)
                            y=num(p.y + 4.0)
                            text-anchor="middle"
                            fill=tone
                            font-size="10"
                            font-weight="700"
                        >
                            "A"
                        </text>
                    </g>
                })
            })
            .collect_view();

        let servers = m
            .servers
            .iter()
            .filter_map(|s| {
                let p = *pos.servers.get(&s.id)?;
                let online =
                    s.status == "online" && fresh(s.last_seen_at.as_deref(), LIVE_SEEN_MS, now_ms);
                let tone = if online {
                    "var(--color-success)"
                } else if s.status == "maintenance" {
                    "var(--color-warn, var(--color-text-muted))"
                } else {
                    "var(--color-danger)"
                };
                let rate = server_rate(&m, &by_id, &s.id);
                let glyph = if s.server_type == "wireguard" {
                    "WG".to_owned()
                } else {
                    let units: Vec<u16> = s.server_type.encode_utf16().take(3).collect();
                    String::from_utf16_lossy(&units).to_uppercase()
                };
                let id = s.id.clone();
                Some(view! {
                    <g
                        on:click=move |_| selected.set(Some(format!("s:{id}")))
                        style="cursor: pointer"
                    >
                        <title>
                            {format!(
                                "{} · {} · {} · {}",
                                s.name,
                                s.server_type,
                                s.status,
                                s.endpoint,
                            )}
                        </title>
                        <circle
                            cx=num(p.x)
                            cy=num(p.y)
                            r="26"
                            fill="var(--color-surface-2)"
                            stroke=tone
                            stroke-width="2"
                            class=online.then_some(NETMAP_PULSE)
                        ></circle>
                        <text
                            x=num(p.x)
                            y=num(p.y + 4.0)
                            text-anchor="middle"
                            fill=tone
                            font-size="12"
                            font-weight="700"
                        >
                            {glyph}
                        </text>
                        <text
                            x=num(p.x)
                            y=num(p.y - 34.0)
                            text-anchor="middle"
                            fill="currentColor"
                            font-size="14"
                            font-weight="600"
                        >
                            {s.name.clone()}
                        </text>
                        <text
                            x=num(p.x)
                            y=num(p.y + 44.0)
                            text-anchor="middle"
                            fill="var(--color-text-muted)"
                            font-size="11.5"
                        >
                            {if rate > 0.0 { rate_label(rate) } else { s.status.clone() }}
                        </text>
                        <text
                            x=num(p.x)
                            y=num(p.y + 58.0)
                            text-anchor="middle"
                            fill="var(--color-text-muted)"
                            font-size="10"
                        >
                            {s.endpoint.clone()}
                        </text>
                    </g>
                })
            })
            .collect_view();

        let orgs = m
            .orgs
            .iter()
            .filter_map(|o| {
                let p = *pos.orgs.get(&o.id)?;
                let tunnels_live = m
                    .tunnels
                    .iter()
                    .filter(|t| {
                        t.org_id == o.id
                            && fresh(t.last_handshake.as_deref(), LIVE_HANDSHAKE_MS, now_ms)
                    })
                    .count();
                let has_members = m.tunnels.iter().any(|t| t.org_id == o.id)
                    || m.devices.iter().any(|d| d.org_id == o.id);
                let label = org_label(&o.name);
                let initials = {
                    let units: Vec<u16> = label.encode_utf16().take(2).collect();
                    String::from_utf16_lossy(&units).to_uppercase()
                };
                let id = o.id.clone();
                let hub_stroke = if tunnels_live > 0 {
                    "var(--color-accent)"
                } else {
                    "var(--color-border)"
                };
                Some(view! {
                    <g
                        on:click=move |_| selected.set(Some(format!("o:{id}")))
                        style="cursor: pointer"
                        opacity=if has_members { "1" } else { "0.45" }
                    >
                        <title>
                            {format!(
                                "{} · period ↓{} ↑{}",
                                label,
                                format_bytes(o.period_bytes_in),
                                format_bytes(o.period_bytes_out),
                            )}
                        </title>
                        <circle
                            cx=num(p.x)
                            cy=num(p.y)
                            r="16"
                            fill="var(--color-surface-2)"
                            stroke=hub_stroke
                            stroke-width="1.8"
                            class=(tunnels_live > 0).then_some(NETMAP_PULSE)
                        ></circle>
                        <text
                            x=num(p.x)
                            y=num(p.y + 4.0)
                            text-anchor="middle"
                            fill="var(--color-accent)"
                            font-size="10"
                            font-weight="700"
                        >
                            {initials}
                        </text>
                        <text
                            x=num(p.x)
                            y=num(p.y - 24.0)
                            text-anchor="middle"
                            fill="currentColor"
                            font-size="13"
                            font-weight="600"
                        >
                            {label.clone()}
                        </text>
                        <text
                            x=num(p.x)
                            y=num(p.y + 32.0)
                            text-anchor="middle"
                            fill="var(--color-text-muted)"
                            font-size="11"
                        >
                            {format!(
                                "{} this period",
                                format_bytes(o.period_bytes_in + o.period_bytes_out),
                            )}
                        </text>
                    </g>
                })
            })
            .collect_view();

        let leaves = m
            .tunnels
            .iter()
            .filter_map(|t| {
                let p = *pos.leaves.get(&format!("t:{}", t.id))?;
                let live = fresh(t.last_handshake.as_deref(), LIVE_HANDSHAKE_MS, now_ms);
                let sp = t.server_id.as_deref().and_then(|sid| pos.servers.get(sid)).copied();
                let lp = ip_label_at(p, sp);
                let id = t.id.clone();
                let leaf_fill =
                    if live { "var(--color-success)" } else { "var(--color-surface-2)" };
                let leaf_stroke =
                    if live { "var(--color-success)" } else { "var(--color-text-muted)" };
                Some(view! {
                    <g
                        on:click=move |_| selected.set(Some(format!("t:{id}")))
                        style="cursor: pointer"
                    >
                        <title>
                            {format!(
                                "{} ({}) · {}",
                                t.profile_name.as_deref().unwrap_or(&t.tunnel_type),
                                t.platform.as_deref().unwrap_or("?"),
                                if live { "connected" } else { &t.status },
                            )}
                        </title>
                        <circle
                            cx=num(p.x)
                            cy=num(p.y)
                            r="8"
                            fill=leaf_fill
                            fill-opacity=if live { "0.9" } else { "1" }
                            stroke=leaf_stroke
                            stroke-width="1.4"
                            class=live.then_some(NETMAP_PULSE)
                        ></circle>
                        {t
                            .allocated_ip
                            .clone()
                            .map(|ip| {
                                view! {
                                    <text
                                        x=num(lp.x)
                                        y=num(lp.y)
                                        text-anchor="middle"
                                        fill="var(--color-text-muted)"
                                        font-size="9.5"
                                    >
                                        {ip}
                                    </text>
                                }
                            })}
                    </g>
                })
            })
            .collect_view();

        let devices = m
            .devices
            .iter()
            .filter_map(|d| {
                let p = *pos.leaves.get(&format!("d:{}", d.id))?;
                let live =
                    d.status == "joined" && fresh(d.last_seen_at.as_deref(), LIVE_SEEN_MS, now_ms);
                let dev_stroke =
                    if live { "var(--color-accent)" } else { "var(--color-text-muted)" };
                Some(view! {
                    <g>
                        <title>{format!("device {} · {}", d.device_name, d.status)}</title>
                        <rect
                            x=num(p.x - 6.0)
                            y=num(p.y - 6.0)
                            width="12"
                            height="12"
                            rx="3"
                            fill="var(--color-surface-2)"
                            stroke=dev_stroke
                            stroke-width="1.4"
                            opacity=if live { "1" } else { "0.5" }
                        ></rect>
                    </g>
                })
            })
            .collect_view();

        view! {
            <svg
                viewBox=format!(
                    "{} {} {} {}",
                    js_to_fixed(vx, 1),
                    js_to_fixed(vy, 1),
                    js_to_fixed(vw, 1),
                    js_to_fixed(vh, 1),
                )
                preserveAspectRatio="xMidYMid meet"
                role="img"
                aria-label="live network topology map"
                class=NETMAP_SVG
            >
                <defs>
                    <radialGradient id="map-bg" cx="50%" cy="50%" r="75%">
                        <stop
                            offset="0%"
                            stop-color="var(--color-accent-soft)"
                            stop-opacity="0.16"
                        ></stop>
                        <stop
                            offset="55%"
                            stop-color="var(--color-surface)"
                            stop-opacity="0"
                        ></stop>
                    </radialGradient>
                </defs>
                <rect
                    x=num(-W)
                    y=num(-H)
                    width=num(3.0 * W)
                    height=num(3.0 * H)
                    fill="url(#map-bg)"
                ></rect>
                <circle
                    cx=num(CX)
                    cy=num(CY)
                    r=num(SERVER_R)
                    fill="none"
                    stroke="var(--color-border)"
                    stroke-opacity="0.5"
                    stroke-dasharray="3 7"
                ></circle>
                <circle
                    cx=num(CX)
                    cy=num(CY)
                    r=num(ORG_R)
                    fill="none"
                    stroke="var(--color-border)"
                    stroke-opacity="0.5"
                    stroke-dasharray="3 7"
                ></circle>
                {spines}
                {edges}
                {ties_t}
                {ties_d}
                <g on:click=move |_| selected.set(None) style="cursor: pointer">
                    <circle
                        cx=num(CX)
                        cy=num(CY)
                        r="46"
                        fill="var(--color-surface-2)"
                        stroke="var(--color-accent)"
                        stroke-width="2"
                        class=NETMAP_PULSE
                    ></circle>
                    <text
                        x=num(CX)
                        y=num(CY - 4.0)
                        text-anchor="middle"
                        fill="var(--color-accent)"
                        font-size="15"
                        font-weight="700"
                    >
                        "CONTROL"
                    </text>
                    <text
                        x=num(CX)
                        y=num(CY + 14.0)
                        text-anchor="middle"
                        fill="var(--color-text-muted)"
                        font-size="12"
                    >
                        "plane"
                    </text>
                </g>
                {agents}
                {servers}
                {orgs}
                {leaves}
                {devices}
            </svg>
        }
    };

    let hud = move || {
        let m = map.get();
        let servers_online = m.servers.iter().filter(|s| s.status == "online").count();
        let agents_joined = m.agents.iter().filter(|a| a.status == "joined").count();
        let connected = m
            .tunnels
            .iter()
            .filter(|t| fresh(t.last_handshake.as_deref(), LIVE_HANDSHAKE_MS, now_ms))
            .count();
        // Text-node segmentation mirrors the JSX (`{n}/{total} servers …`):
        // the rendered string is "2/2 servers …" while the snapshot's
        // space-joined own-text reads "2 / 2 servers …", same as React's.
        let total = m.servers.len();
        view! {
            <div class=NETMAP_HUD>
                <span class=NETMAP_HUD_LINE>
                    {servers_online.to_string()} "/" {total.to_string()}
                    " servers · " {agents_joined.to_string()} " agents · "
                    {connected.to_string()} " connected tunnels"
                </span>
                <span class=NETMAP_HUD_SUB>
                    {format!(
                        "fleet live rate {} · {} customers",
                        rate_label(total_rate()),
                        m.orgs.len(),
                    )}
                </span>
            </div>
        }
    };

    let panel = move || -> Option<AnyView> {
        let sel = selected.get()?;
        let m = map.get();
        let by_id = rate_by_id.get();
        if let Some(tid) = sel.strip_prefix("t:") {
            let t = m.tunnels.iter().find(|t| t.id == tid)?;
            let r = by_id.get(&t.id);
            let org = m.orgs.iter().find(|o| o.id == t.org_id);
            let srv =
                t.server_id.as_deref().and_then(|sid| m.servers.iter().find(|s| s.id == sid));
            let line = [
                format!(
                    "{} · {} ({})",
                    org_label(org.map_or("org", |o| o.name.as_str())),
                    t.profile_name.as_deref().unwrap_or(&t.tunnel_type),
                    t.platform.as_deref().unwrap_or("?"),
                ),
                format!(
                    "server {} · ip {}",
                    srv.map_or("—", |s| s.name.as_str()),
                    t.allocated_ip.as_deref().unwrap_or("—"),
                ),
                format!(
                    "↓{} ↑{} lifetime",
                    format_bytes(t.last_bytes_in.unwrap_or(0.0)),
                    format_bytes(t.last_bytes_out.unwrap_or(0.0)),
                ),
                match r {
                    Some(r) => {
                        format!("live ↓{} ↑{}", rate_label(r.in_rate), rate_label(r.out_rate))
                    }
                    None => "live —".to_owned(),
                },
            ]
            .join("  ·  ");
            let mut chips: Vec<AnyView> = Vec::new();
            if let Some(rtt) = t.rtt_ms {
                let tone = if rtt > RTT_WARN_MS { BadgeTone::Warn } else { BadgeTone::Default };
                chips.push(view! { <Badge tone=tone>{num(rtt)} " ms"</Badge> }.into_any());
            }
            if let Some(loss) = t.packet_loss_pct {
                let tone = if loss > LOSS_WARN_PCT { BadgeTone::Bad } else { BadgeTone::Default };
                chips.push(
                    view! { <Badge tone=tone>{to_fixed(loss, 2)} "% loss"</Badge> }.into_any(),
                );
            }
            if let Some(pq) = t.pq_negotiated {
                let tone = if pq { BadgeTone::Ok } else { BadgeTone::Warn };
                chips.push(
                    view! {
                        <Badge tone=tone with_dot=true>
                            {if pq { "PQ hybrid" } else { "PQ off" }}
                        </Badge>
                    }
                    .into_any(),
                );
            }
            if t.datagrams_in.is_some() || t.datagrams_out.is_some() {
                chips.push(
                    view! {
                        <Badge tone=BadgeTone::Accent>
                            {format!(
                                "↓{} ↑{} dg",
                                num(t.datagrams_in.unwrap_or(0.0)),
                                num(t.datagrams_out.unwrap_or(0.0)),
                            )}
                        </Badge>
                    }
                    .into_any(),
                );
            }
            if let Some(streams) = t.streams_open {
                chips.push(
                    view! { <Badge tone=BadgeTone::Default>{num(streams)} " streams"</Badge> }
                        .into_any(),
                );
            }
            let has_chips = !chips.is_empty();
            return Some(
                view! {
                    <div class=NETMAP_PANEL>
                        <div class=NETMAP_PANEL_COL>
                            <span>{line}</span>
                            {has_chips.then(|| view! { <div class=NETMAP_CHIPS>{chips}</div> })}
                        </div>
                    </div>
                }
                .into_any(),
            );
        }
        let text = if let Some(sid) = sel.strip_prefix("s:") {
            let s = m.servers.iter().find(|s| s.id == sid)?;
            let n = m.tunnels.iter().filter(|t| t.server_id.as_deref() == Some(sid)).count();
            let pop = match &s.pop_slug {
                Some(slug) => match &s.pop_location {
                    Some(loc) => format!(" · pop {slug} ({loc})"),
                    None => format!(" · pop {slug}"),
                },
                None => String::new(),
            };
            format!(
                "{} · {} · {} · {} · {} tunnels · live {}{}",
                s.name,
                s.server_type,
                s.location,
                s.endpoint,
                n,
                rate_label(server_rate(&m, &by_id, sid)),
                pop,
            )
        } else if let Some(oid) = sel.strip_prefix("o:") {
            let o = m.orgs.iter().find(|o| o.id == oid)?;
            let cap =
                o.monthly_limit_bytes.map(|b| format!(" of {}", format_bytes(b))).unwrap_or_default();
            let mine: Vec<_> = m.tunnels.iter().filter(|t| t.org_id == o.id).collect();
            let connected = mine
                .iter()
                .filter(|t| fresh(t.last_handshake.as_deref(), LIVE_HANDSHAKE_MS, now_ms))
                .count();
            let mut hosts: Vec<&str> = Vec::new();
            for t in &mine {
                if let Some(sid) = t.server_id.as_deref() {
                    if !hosts.contains(&sid) {
                        hosts.push(sid);
                    }
                }
            }
            let org_rate = mine
                .iter()
                .fold(0.0, |s, t| by_id.get(&t.id).map_or(s, |r| s + r.in_rate + r.out_rate));
            format!(
                "{} · {} connected on {} server{} · live {} · period ↓{} ↑{}{}",
                org_label(&o.name),
                connected,
                hosts.len(),
                if hosts.len() == 1 { "" } else { "s" },
                rate_label(org_rate),
                format_bytes(o.period_bytes_in),
                format_bytes(o.period_bytes_out),
                cap,
            )
        } else {
            return None;
        };
        Some(view! { <div class=NETMAP_PANEL>{text}</div> }.into_any())
    };

    // The LIVE timestamp is locale-rendered in the browser; SSR emits the
    // raw ISO and a client Effect swaps in `toLocaleTimeString()` after
    // hydration (Leptos reuses server text, so a render-time call would
    // never re-run — the reference is CSR and always formats client-side).
    let live_time = RwSignal::new(String::new());
    Effect::new(move |_| {
        live_time.set(map.with(|m| locale_time(&m.generated_at)));
    });
    view! {
        <div class=NETMAP>
            {svg}
            {hud}
            <div class=NETMAP_LIVE>
                <span class=NETMAP_LIVE_DOT></span>
                "LIVE · "
                {move || {
                    let t = live_time.get();
                    if t.is_empty() { map.with(|m| m.generated_at.clone()) } else { t }
                }}
            </div>
            {panel}
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{root}{{position:relative;height:100%;width:100%;overflow:hidden}}",
            ".{svg}{{display:block;height:100%;width:100%}}",
            ".{hud}{{pointer-events:none;position:absolute;left:1rem;top:1rem;",
            "display:flex;flex-direction:column;gap:.25rem;font-size:13px}}",
            ".{hud_line}{{font-size:15px;font-weight:600}}",
            ".{hud_sub}{{color:var(--color-text-muted)}}",
            ".{live}{{pointer-events:none;position:absolute;right:1rem;",
            "bottom:1rem;display:inline-flex;align-items:center;gap:.5rem;",
            "font-size:12.5px;color:var(--color-text-muted)}}",
            ".{live_dot}{{display:inline-block;width:.5rem;height:.5rem;",
            "border-radius:calc(infinity * 1px);",
            "background-color:var(--color-success);",
            "animation:asy-map-pulse 1.6s ease-in-out infinite}}",
            // Tailwind v4 `-translate-x-1/2` rides the CSS `translate`
            // property — computed `transform` stays none.
            ".{panel}{{position:absolute;bottom:1rem;left:50%;",
            "translate:-50%;max-width:calc(100% - 2rem);overflow-wrap:anywhere;",
            "border-radius:var(--radius-md);",
            "border:1px solid var(--color-border);",
            "background-color:var(--color-surface-2);",
            "padding:.5rem 1rem;font-size:12.5px}}",
            ".{panel_col}{{display:flex;flex-direction:column;",
            "align-items:center;gap:.375rem}}",
            ".{chips}{{display:flex;flex-wrap:wrap;align-items:center;",
            "justify-content:center;gap:.375rem}}",
            // The reference's inline <style> block, asy-scoped: flow
            // dash animation buckets + node pulse, all disabled under
            // reduced motion (the harness's mode).
            ".{flow}{{stroke-dasharray:6 10;",
            "animation:asy-map-flow 3s linear infinite}}",
            ".{flow_slow}{{animation-duration:6s}}",
            ".{flow_med}{{animation-duration:2.5s}}",
            ".{flow_fast}{{animation-duration:1s}}",
            ".{flow_idle}{{animation:none}}",
            ".{pulse}{{animation:asy-map-pulse 2.4s ease-in-out infinite}}",
            "@keyframes asy-map-flow{{to{{stroke-dashoffset:-64}}}}",
            "@keyframes asy-map-pulse{{",
            "0%,100%{{opacity:1}}50%{{opacity:.55}}}}",
            "@media (prefers-reduced-motion: reduce){{",
            ".{flow},.{flow_slow},.{flow_med},.{flow_fast},.{pulse},",
            ".{live_dot}{{animation:none}}}}",
        ),
        root = NETMAP,
        svg = NETMAP_SVG,
        hud = NETMAP_HUD,
        hud_line = NETMAP_HUD_LINE,
        hud_sub = NETMAP_HUD_SUB,
        live = NETMAP_LIVE,
        live_dot = NETMAP_LIVE_DOT,
        panel = NETMAP_PANEL,
        panel_col = NETMAP_PANEL_COL,
        chips = NETMAP_CHIPS,
        flow = NETMAP_FLOW,
        flow_slow = NETMAP_FLOW_SLOW,
        flow_med = NETMAP_FLOW_MED,
        flow_fast = NETMAP_FLOW_FAST,
        flow_idle = NETMAP_FLOW_IDLE,
        pulse = NETMAP_PULSE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_class_port_buckets_match_reference() {
        assert_eq!(flow_class_port(2_000_000.0), "asy-netmap__flow asy-netmap__flow--fast");
        assert_eq!(flow_class_port(50_000.0), "asy-netmap__flow asy-netmap__flow--med");
        assert_eq!(flow_class_port(1.0), "asy-netmap__flow asy-netmap__flow--slow");
        assert_eq!(flow_class_port(0.0), NETMAP_FLOW_IDLE);
    }

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [
            NETMAP, NETMAP_SVG, NETMAP_HUD, NETMAP_HUD_LINE, NETMAP_HUD_SUB, NETMAP_LIVE,
            NETMAP_LIVE_DOT, NETMAP_PANEL, NETMAP_PANEL_COL, NETMAP_CHIPS, NETMAP_FLOW,
            NETMAP_FLOW_SLOW, NETMAP_FLOW_MED, NETMAP_FLOW_FAST, NETMAP_FLOW_IDLE, NETMAP_PULSE,
        ] {
            assert!(css.contains(&format!(".{class}")), "missing rule for {class}");
        }
    }
}
