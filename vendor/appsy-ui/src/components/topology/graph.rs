//! The network map's graph builder — mirror of `buildGraph` +
//! `useForceLayout` in `platform.map.tsx`, and the pure helpers the
//! render layer shares (`flowClass`, `orgLabel`, `bow`, `ipLabelAt`,
//! the `data/tunnels.ts` `formatBytes`).
//!
//! Time is data: the reference calls `Date.now()`; determinism forbids
//! that here, so freshness takes `now_ms` from the caller (the
//! component receives it as a prop — the consumer owns the clock).
//! JS exactness notes: string lengths are UTF-16 code units, `hash01`
//! is FNV-1a over code units with `Math.imul` (u32 wrapping), the org
//! host set keeps first-seen insertion order (JS `Set`), and the byte
//! unit index comes from `log(bytes)/log(1024)` floored — float warts
//! included.

use std::collections::HashMap;

use super::force::{SimLink, SimNode, Simulation};
use crate::components::host_metrics_card::to_fixed;

/// SVG stage size; the viewBox scales to any screen/projector.
pub const W: f64 = 1600.0;
pub const H: f64 = 900.0;
pub const CX: f64 = W / 2.0;
pub const CY: f64 = H / 2.0;
/// Radius of the infra (server) ring around the control plane.
pub const SERVER_R: f64 = 210.0;
/// Radius of the customer-org ring.
pub const ORG_R: f64 = 350.0;
/// Spring rest length between a tunnel/device leaf and its org hub.
pub const LEAF_R: f64 = 62.0;
/// Parking ring for unconnected inventory.
pub const EDGE_R: f64 = 600.0;

/// A tunnel handshake younger than this is "connected right now".
pub const LIVE_HANDSHAKE_MS: f64 = 180_000.0;
/// A device/server/agent heartbeat younger than this is live.
pub const LIVE_SEEN_MS: f64 = 300_000.0;
/// RTT above this (ms) flags the chip amber in the selection detail.
pub const RTT_WARN_MS: f64 = 150.0;
/// Packet loss above this (%) flags the chip red in the selection detail.
pub const LOSS_WARN_PCT: f64 = 2.0;

// ------------------------------------------------------------- data model

/// `PlatformTopologyMapResponse` — the snapshot the consumer polls.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TopologyMap {
    pub generated_at: String,
    pub servers: Vec<TopologyServer>,
    pub agents: Vec<TopologyAgent>,
    pub orgs: Vec<TopologyOrg>,
    pub tunnels: Vec<TopologyTunnel>,
    pub devices: Vec<TopologyDevice>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TopologyServer {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub location: String,
    pub server_type: String,
    pub status: String,
    pub mode_mismatch: bool,
    pub agent_id: Option<String>,
    pub last_seen_at: Option<String>,
    pub quic_mode: Option<String>,
    pub backend_tier: Option<String>,
    pub pop_id: Option<String>,
    pub pop_slug: Option<String>,
    pub pop_location: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TopologyAgent {
    pub id: String,
    pub name: String,
    pub status: String,
    pub capabilities: Vec<String>,
    pub hostname: Option<String>,
    pub last_seen_at: Option<String>,
    pub reported_version: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TopologyOrg {
    pub id: String,
    pub name: String,
    pub period_bytes_in: f64,
    pub period_bytes_out: f64,
    pub monthly_limit_bytes: Option<f64>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TopologyTunnel {
    pub id: String,
    pub org_id: String,
    pub status: String,
    pub tunnel_type: String,
    pub server_id: Option<String>,
    pub allocated_ip: Option<String>,
    pub last_handshake: Option<String>,
    pub profile_name: Option<String>,
    pub platform: Option<String>,
    pub last_bytes_in: Option<f64>,
    pub last_bytes_out: Option<f64>,
    pub rtt_ms: Option<f64>,
    pub packet_loss_pct: Option<f64>,
    pub pq_negotiated: Option<bool>,
    pub streams_open: Option<f64>,
    pub datagrams_in: Option<f64>,
    pub datagrams_out: Option<f64>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TopologyDevice {
    pub id: String,
    pub org_id: String,
    pub device_name: String,
    pub status: String,
    pub last_seen_at: Option<String>,
}

// ------------------------------------------------------------- helpers

/// `data/tunnels.ts` `formatBytes` (decimal-labeled 1024 steps; unit
/// index from `floor(log(bytes)/log(1024))`, float warts and all).
pub fn format_bytes(bytes: f64) -> String {
    if bytes <= 0.0 || bytes.is_nan() {
        return "0 B".to_owned();
    }
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let i = ((bytes.ln() / 1024f64.ln()).floor() as i64).min(UNITS.len() as i64 - 1);
    let i = if i < 0 { 0 } else { i as usize };
    let value = bytes / 1024f64.powi(i as i32);
    let digits = if i == 0 || value >= 100.0 { 0 } else { 1 };
    format!("{} {}", to_fixed(value, digits), UNITS[i])
}

/// `Date.parse` for the backend's RFC3339 subset
/// (`YYYY-MM-DDTHH:MM:SS[.fff][Z|±HH:MM]`); NaN on anything else.
/// An offset-less timestamp is refused rather than read in local time —
/// the API always sends an offset, and local time is nondeterminism.
pub fn parse_date_ms(iso: &str) -> f64 {
    fn digits(s: &str, range: std::ops::Range<usize>) -> Option<i64> {
        s.get(range).and_then(|d| d.parse::<i64>().ok())
    }
    let b = iso.as_bytes();
    if b.len() < 20 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') || b.get(10) != Some(&b'T')
    {
        return f64::NAN;
    }
    let (Some(year), Some(month), Some(day), Some(hour), Some(min), Some(sec)) = (
        digits(iso, 0..4),
        digits(iso, 5..7),
        digits(iso, 8..10),
        digits(iso, 11..13),
        digits(iso, 14..16),
        digits(iso, 17..19),
    ) else {
        return f64::NAN;
    };
    let mut idx = 19;
    let mut frac_ms = 0.0;
    if b.get(idx) == Some(&b'.') {
        let start = idx + 1;
        let mut end = start;
        while b.get(end).is_some_and(|c| c.is_ascii_digit()) {
            end += 1;
        }
        if end == start {
            return f64::NAN;
        }
        let frac: String = iso[start..end].chars().take(9).collect();
        let scale = 10f64.powi(frac.len() as i32);
        frac_ms = frac.parse::<f64>().unwrap_or(0.0) / scale * 1000.0;
        idx = end;
    }
    let offset_min: i64 = match b.get(idx) {
        Some(&b'Z') if idx + 1 == b.len() => 0,
        Some(&sign) if sign == b'+' || sign == b'-' => {
            if b.len() != idx + 6 || b.get(idx + 3) != Some(&b':') {
                return f64::NAN;
            }
            let (Some(oh), Some(om)) = (digits(iso, idx + 1..idx + 3), digits(iso, idx + 4..idx + 6))
            else {
                return f64::NAN;
            };
            let m = oh * 60 + om;
            if sign == b'-' { -m } else { m }
        }
        _ => return f64::NAN,
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return f64::NAN;
    }
    // Howard Hinnant's days-from-civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let secs = days * 86400 + hour * 3600 + min * 60 + sec - offset_min * 60;
    secs as f64 * 1000.0 + frac_ms
}

/// The reference's `fresh` with the clock passed in.
pub fn fresh(iso: Option<&str>, window_ms: f64, now_ms: f64) -> bool {
    let Some(iso) = iso else { return false };
    let t = parse_date_ms(iso);
    t.is_finite() && now_ms - t < window_ms
}

/// Bucket a bytes/s figure into an edge-animation speed class.
pub fn flow_class(rate: f64) -> &'static str {
    if rate > 1_000_000.0 {
        "flow flow-fast"
    } else if rate > 10_000.0 {
        "flow flow-med"
    } else if rate > 0.0 {
        "flow flow-slow"
    } else {
        "flow-idle"
    }
}

pub fn rate_label(rate: f64) -> String {
    format!("{}/s", format_bytes(rate))
}

/// Customer display name: the auto-generated "(personal)" suffix is
/// org-model noise (`/\s*\(personal\)\s*$/i`).
pub fn org_label(name: &str) -> String {
    let no_trail = name.trim_end();
    let lower = no_trail.to_lowercase();
    if lower.ends_with("(personal)") {
        no_trail[..no_trail.len() - "(personal)".len()].trim_end().to_owned()
    } else {
        name.to_owned()
    }
}

/// UTF-16 length — JS `String.prototype.length`.
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Estimated half-width (stage px) of one rendered SVG text run.
pub fn half_w(chars: usize, font_size: f64) -> f64 {
    (chars as f64 * font_size * 0.62) / 2.0 + 10.0
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Xy {
    pub x: f64,
    pub y: f64,
}

pub fn polar(cx: f64, cy: f64, r: f64, angle: f64) -> Xy {
    Xy { x: cx + r * angle.cos(), y: cy + r * angle.sin() }
}

/// JS `Number.prototype.toFixed` over the full domain — the shared
/// `to_fixed` is positive-only; JS negatives round on the magnitude.
pub(crate) fn js_to_fixed(v: f64, digits: u32) -> String {
    if v < 0.0 {
        format!("-{}", to_fixed(-v, digits))
    } else {
        to_fixed(v, digits)
    }
}

/// Quadratic bezier from a→b bowed toward the stage centre.
pub fn bow(a: Xy, b: Xy, k: f64) -> String {
    let mx = (a.x + b.x) / 2.0;
    let my = (a.y + b.y) / 2.0;
    let qx = mx + (CX - mx) * k;
    let qy = my + (CY - my) * k;
    format!(
        "M {} {} Q {} {} {} {}",
        js_to_fixed(a.x, 1),
        js_to_fixed(a.y, 1),
        js_to_fixed(qx, 1),
        js_to_fixed(qy, 1),
        js_to_fixed(b.x, 1),
        js_to_fixed(b.y, 1),
    )
}

/// FNV-1a over UTF-16 code units → [0,1); `Math.imul` semantics.
pub fn hash01(s: &str) -> f64 {
    let mut h: u32 = 2166136261;
    for cu in s.encode_utf16() {
        h ^= cu as u32;
        h = h.wrapping_mul(16777619);
    }
    h as f64 / 4294967296.0
}

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    hi.min(lo.max(v))
}

/// IP labels sit perpendicular to the leaf's edge axis, on the side
/// facing away from the stage centre.
pub fn ip_label_at(leaf: Xy, toward: Option<Xy>) -> Xy {
    let t = toward.unwrap_or(Xy { x: CX, y: CY });
    let dx = t.x - leaf.x;
    let dy = t.y - leaf.y;
    let mut px = -dy;
    let mut py = dx;
    let len = {
        let h = px.hypot(py);
        if h != 0.0 { h } else { 1.0 }
    };
    px /= len;
    py /= len;
    if px * (leaf.x - CX) + py * (leaf.y - CY) < 0.0 {
        px = -px;
        py = -py;
    }
    Xy { x: leaf.x + px * 22.0, y: leaf.y + py * 22.0 + 3.0 }
}

// ------------------------------------------------------------- layout

/// One cached node — position and velocity persist across snapshots
/// (the reference keeps `MapNode` objects alive in a ref).
#[derive(Clone, Copy, Debug)]
struct CachedNode {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
}

/// Persistent layout state: the node cache plus the simulation's alpha
/// and LCG stream, which survive across snapshots (one d3 simulation
/// object lives for the component's lifetime).
pub struct LayoutState {
    cache: HashMap<String, CachedNode>,
    alpha: f64,
    lcg_s: f64,
}

impl Default for LayoutState {
    fn default() -> Self {
        LayoutState { cache: HashMap::new(), alpha: 1.0, lcg_s: 1.0 }
    }
}

/// Settled positions keyed the way the render code addresses nodes.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct LayoutPos {
    pub servers: HashMap<String, Xy>,
    pub agents: HashMap<String, Xy>,
    pub orgs: HashMap<String, Xy>,
    /// Keyed by the prefixed id (`t:{id}` / `d:{id}`), like the reference.
    pub leaves: HashMap<String, Xy>,
}

struct BuildNode {
    id: String,
    sim: SimNode,
}

/// `buildGraph` + the reduced-motion `useForceLayout` pass: build the
/// physics graph from one snapshot, settle 300 ticks, clamp, publish.
pub fn layout(map: &TopologyMap, state: &mut LayoutState, now_ms: f64) -> LayoutPos {
    let mut nodes: Vec<BuildNode> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut links: Vec<(String, String, f64, f64)> = Vec::new();

    // upsert: cached position/velocity wins; seeds only place new nodes.
    let upsert = |nodes: &mut Vec<BuildNode>,
                      index: &mut HashMap<String, usize>,
                      cache: &mut HashMap<String, CachedNode>,
                      id: &str,
                      collide: f64,
                      seed: Xy,
                      fixed: Option<Xy>| {
        let cached = *cache
            .entry(id.to_owned())
            .or_insert(CachedNode { x: seed.x, y: seed.y, vx: 0.0, vy: 0.0 });
        let mut sim = SimNode::new(cached.x, cached.y);
        sim.vx = cached.vx;
        sim.vy = cached.vy;
        sim.collide = collide;
        if let Some(f) = fixed {
            sim.fx = Some(f.x);
            sim.fy = Some(f.y);
        }
        index.insert(id.to_owned(), nodes.len());
        nodes.push(BuildNode { id: id.to_owned(), sim });
        nodes.len() - 1
    };

    upsert(
        &mut nodes,
        &mut index,
        &mut state.cache,
        "core",
        64.0,
        Xy { x: CX, y: CY },
        Some(Xy { x: CX, y: CY }),
    );

    let mut server_angle: HashMap<String, f64> = HashMap::new();
    for (i, s) in map.servers.iter().enumerate() {
        let a = 2.0 * std::f64::consts::PI * i as f64 / (map.servers.len().max(1) as f64)
            - std::f64::consts::PI / 2.0;
        server_angle.insert(s.id.clone(), a);
        let p = polar(CX, CY, SERVER_R, a);
        let r = 72f64.max(half_w(js_len(&s.name), 14.0)).max(half_w(js_len(&s.endpoint), 10.0));
        upsert(&mut nodes, &mut index, &mut state.cache, &format!("s:{}", s.id), r, p, Some(p));
    }

    for a in &map.agents {
        let owner = map.servers.iter().find(|s| s.agent_id.as_deref() == Some(a.id.as_str()));
        let seed = match owner.and_then(|o| server_angle.get(&o.id)) {
            Some(&angle) => polar(CX, CY, SERVER_R + 44.0, angle + 0.3),
            None => polar(CX, CY, 118.0, 2.0 * std::f64::consts::PI * hash01(&a.id)),
        };
        upsert(&mut nodes, &mut index, &mut state.cache, &format!("a:{}", a.id), 20.0, seed, None);
        match owner {
            Some(o) => links.push((format!("a:{}", a.id), format!("s:{}", o.id), 52.0, 0.9)),
            None => links.push((format!("a:{}", a.id), "core".to_owned(), 118.0, 0.4)),
        }
    }

    let mut radial: Vec<(usize, f64, f64)> = Vec::new();
    for o in &map.orgs {
        // Distinct hosting servers, first-seen order (JS Set).
        let mut hosts: Vec<String> = Vec::new();
        for t in &map.tunnels {
            if t.org_id == o.id {
                if let Some(sid) = &t.server_id {
                    if !hosts.contains(sid) {
                        hosts.push(sid.clone());
                    }
                }
            }
        }
        hosts.retain(|sid| server_angle.contains_key(sid));
        let mut sx = 0.0;
        let mut sy = 0.0;
        for sid in &hosts {
            let a = server_angle[sid];
            sx += a.cos();
            sy += a.sin();
        }
        let seed_angle = if !hosts.is_empty() && (sx != 0.0 || sy != 0.0) {
            sy.atan2(sx)
        } else {
            2.0 * std::f64::consts::PI * hash01(&o.id)
        };
        let label = org_label(&o.name);
        let usage = format!("{} this period", format_bytes(o.period_bytes_in + o.period_bytes_out));
        let collide =
            48f64.max(half_w(js_len(&label), 13.0)).max(half_w(js_len(&usage), 11.0));
        let connected = map
            .tunnels
            .iter()
            .any(|t| t.org_id == o.id && fresh(t.last_handshake.as_deref(), LIVE_HANDSHAKE_MS, now_ms))
            || map.devices.iter().any(|d| {
                d.org_id == o.id
                    && d.status == "joined"
                    && fresh(d.last_seen_at.as_deref(), LIVE_SEEN_MS, now_ms)
            });
        let ring = if connected { ORG_R } else { EDGE_R };
        let ni = upsert(
            &mut nodes,
            &mut index,
            &mut state.cache,
            &format!("o:{}", o.id),
            collide,
            polar(CX, CY, ring, seed_angle),
            None,
        );
        if connected {
            for sid in &hosts {
                links.push((
                    format!("o:{}", o.id),
                    format!("s:{sid}"),
                    150.0,
                    0.55 / hosts.len() as f64,
                ));
            }
        } else {
            radial.push((ni, EDGE_R, 0.45));
        }
    }

    // seedNear reads the org hub's CURRENT cached position (orgs were
    // upserted above, so fresh orgs have their seeds in the cache).
    let seed_near = |cache: &HashMap<String, CachedNode>, org_id: &str, id: &str| {
        let (hx, hy) = cache
            .get(&format!("o:{org_id}"))
            .map_or((CX, CY), |h| (h.x, h.y));
        Xy {
            x: hx + LEAF_R * (2.0 * std::f64::consts::PI * hash01(id)).cos(),
            y: hy + LEAF_R * (2.0 * std::f64::consts::PI * hash01(id)).sin(),
        }
    };

    for t in &map.tunnels {
        let live = fresh(t.last_handshake.as_deref(), LIVE_HANDSHAKE_MS, now_ms);
        let collide = match &t.allocated_ip {
            Some(ip) => 22.0 + half_w(js_len(ip), 9.5),
            None => 16.0,
        };
        let seed = seed_near(&state.cache, &t.org_id, &t.id);
        let ni = upsert(
            &mut nodes,
            &mut index,
            &mut state.cache,
            &format!("t:{}", t.id),
            collide,
            seed,
            None,
        );
        if live {
            links.push((format!("t:{}", t.id), format!("o:{}", t.org_id), 56.0, 0.6));
            if let Some(sid) = &t.server_id {
                if server_angle.contains_key(sid) {
                    links.push((format!("t:{}", t.id), format!("s:{sid}"), 100.0, 0.35));
                }
            }
        } else {
            links.push((format!("t:{}", t.id), format!("o:{}", t.org_id), LEAF_R, 0.15));
            radial.push((ni, EDGE_R, 0.35));
        }
    }

    for d in &map.devices {
        let live = d.status == "joined" && fresh(d.last_seen_at.as_deref(), LIVE_SEEN_MS, now_ms);
        let seed = seed_near(&state.cache, &d.org_id, &d.id);
        let ni = upsert(
            &mut nodes,
            &mut index,
            &mut state.cache,
            &format!("d:{}", d.id),
            18.0,
            seed,
            None,
        );
        links.push((
            format!("d:{}", d.id),
            format!("o:{}", d.org_id),
            LEAF_R - 8.0,
            if live { 0.85 } else { 0.15 },
        ));
        if !live {
            radial.push((ni, EDGE_R, 0.35));
        }
    }

    for (ni, r, strength) in radial {
        nodes[ni].sim.radial = r;
        nodes[ni].sim.radial_strength = strength;
    }

    // Prune vanished nodes from the cache.
    let alive: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    state.cache.retain(|id, _| alive.contains(id.as_str()));

    let sim_links: Vec<SimLink> = links
        .iter()
        .map(|(s, t, dist, strength)| SimLink {
            source: index[s.as_str()],
            target: index[t.as_str()],
            dist: *dist,
            strength: *strength,
        })
        .collect();
    let sim_nodes: Vec<SimNode> = nodes.iter().map(|n| n.sim.clone()).collect();
    let mut sim = Simulation::with_state(sim_nodes, sim_links, CX, CY, state.alpha, state.lcg_s);
    sim.settle();
    state.alpha = sim.alpha();
    state.lcg_s = sim.lcg_state();

    // publish(): clamp into the stage and write back (the clamped
    // coordinates persist in the cache, like the reference's nodes).
    let mut pos = LayoutPos::default();
    for (n, sn) in nodes.iter().zip(sim.nodes.iter()) {
        let x = clamp(sn.x, 30.0, W - 30.0);
        let y = clamp(sn.y, 26.0, H - 26.0);
        state.cache.insert(
            n.id.clone(),
            CachedNode { x, y, vx: sn.vx, vy: sn.vy },
        );
        let p = Xy { x, y };
        if let Some(rest) = n.id.strip_prefix("s:") {
            pos.servers.insert(rest.to_owned(), p);
        } else if let Some(rest) = n.id.strip_prefix("a:") {
            pos.agents.insert(rest.to_owned(), p);
        } else if let Some(rest) = n.id.strip_prefix("o:") {
            pos.orgs.insert(rest.to_owned(), p);
        } else if n.id.starts_with("t:") || n.id.starts_with("d:") {
            pos.leaves.insert(n.id.clone(), p);
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;


    /// End-to-end parity against the reference's own `buildGraph` run
    /// verbatim in Node (type-stripped extract of `platform.map.tsx`
    /// + real d3-force, `Date.now` pinned to 2026-05-18T12:00:00Z,
    /// `stop(); tick(300)`, clamp). 2 servers / 2 agents (one orphan) /
    /// 3 orgs (one "(personal)", one dormant) / 4 tunnels (one stale,
    /// one without ip) / 2 devices (one stale). The CLAUDE.md bar is
    /// 1px per coordinate; this asserts far tighter.
    #[test]
    fn layout_matches_reference_run() {
        let map = TopologyMap {
            generated_at: "2026-05-18T12:00:00Z".into(),
            servers: vec![
                TopologyServer {
                    id: "sv1".into(),
                    name: "fra-wg-02".into(),
                    endpoint: "203.0.113.40:51820".into(),
                    location: "Frankfurt".into(),
                    server_type: "wireguard".into(),
                    status: "online".into(),
                    mode_mismatch: false,
                    agent_id: Some("ag1".into()),
                    last_seen_at: Some("2100-01-01T00:00:00Z".into()),
                    quic_mode: None,
                    backend_tier: None,
                    pop_id: None,
                    pop_slug: None,
                    pop_location: None,
                },
                TopologyServer {
                    id: "sv2".into(),
                    name: "ams-quic-01".into(),
                    endpoint: "198.51.100.9:4433".into(),
                    location: "Amsterdam".into(),
                    server_type: "quic".into(),
                    status: "online".into(),
                    mode_mismatch: false,
                    agent_id: None,
                    last_seen_at: Some("2100-01-01T00:00:00Z".into()),
                    quic_mode: None,
                    backend_tier: None,
                    pop_id: None,
                    pop_slug: None,
                    pop_location: None,
                },
            ],
            agents: vec![
                TopologyAgent {
                    id: "ag1".into(),
                    name: "agent-fra".into(),
                    status: "online".into(),
                    capabilities: vec![],
                    hostname: None,
                    last_seen_at: Some("2100-01-01T00:00:00Z".into()),
                    reported_version: Some("1.4.2".into()),
                },
                TopologyAgent {
                    id: "ag2".into(),
                    name: "agent-orphan".into(),
                    status: "online".into(),
                    capabilities: vec![],
                    hostname: None,
                    last_seen_at: Some("2100-01-01T00:00:00Z".into()),
                    reported_version: None,
                },
            ],
            orgs: vec![
                TopologyOrg {
                    id: "org1".into(),
                    name: "Acme Robotics".into(),
                    period_bytes_in: 88_400_000_000.0,
                    period_bytes_out: 1_900_000_000.0,
                    monthly_limit_bytes: None,
                },
                TopologyOrg {
                    id: "org2".into(),
                    name: "lena (personal)".into(),
                    period_bytes_in: 12_000.0,
                    period_bytes_out: 0.0,
                    monthly_limit_bytes: None,
                },
                TopologyOrg {
                    id: "org3".into(),
                    name: "Dormant Corp".into(),
                    period_bytes_in: 0.0,
                    period_bytes_out: 0.0,
                    monthly_limit_bytes: None,
                },
            ],
            tunnels: vec![
                TopologyTunnel {
                    id: "tn1".into(),
                    org_id: "org1".into(),
                    status: "active".into(),
                    tunnel_type: "wireguard".into(),
                    server_id: Some("sv1".into()),
                    allocated_ip: Some("203.0.113.41".into()),
                    last_handshake: Some("2100-01-01T00:00:00Z".into()),
                    profile_name: Some("ci-runner-3".into()),
                    platform: Some("linux".into()),
                    last_bytes_in: Some(10.0),
                    last_bytes_out: Some(10.0),
                    rtt_ms: Some(12.0),
                    packet_loss_pct: Some(0.0),
                    pq_negotiated: Some(true),
                    streams_open: None,
                    datagrams_in: None,
                    datagrams_out: None,
                },
                TopologyTunnel {
                    id: "tn2".into(),
                    org_id: "org1".into(),
                    status: "active".into(),
                    tunnel_type: "quic".into(),
                    server_id: Some("sv2".into()),
                    allocated_ip: None,
                    last_handshake: Some("2100-01-01T00:00:00Z".into()),
                    profile_name: Some("lena-mbp".into()),
                    platform: Some("macos".into()),
                    last_bytes_in: Some(5.0),
                    last_bytes_out: Some(2.0),
                    rtt_ms: Some(31.0),
                    packet_loss_pct: Some(0.4),
                    pq_negotiated: Some(false),
                    streams_open: None,
                    datagrams_in: None,
                    datagrams_out: None,
                },
                TopologyTunnel {
                    id: "tn3".into(),
                    org_id: "org2".into(),
                    status: "active".into(),
                    tunnel_type: "wireguard".into(),
                    server_id: Some("sv1".into()),
                    allocated_ip: Some("203.0.113.55".into()),
                    last_handshake: Some("2100-01-01T00:00:00Z".into()),
                    profile_name: None,
                    platform: None,
                    last_bytes_in: Some(0.0),
                    last_bytes_out: Some(0.0),
                    rtt_ms: None,
                    packet_loss_pct: None,
                    pq_negotiated: None,
                    streams_open: None,
                    datagrams_in: None,
                    datagrams_out: None,
                },
                TopologyTunnel {
                    id: "tn4".into(),
                    org_id: "org3".into(),
                    status: "idle".into(),
                    tunnel_type: "wireguard".into(),
                    server_id: None,
                    allocated_ip: None,
                    last_handshake: Some("2000-01-01T00:00:00Z".into()),
                    profile_name: Some("old-box".into()),
                    platform: None,
                    last_bytes_in: None,
                    last_bytes_out: None,
                    rtt_ms: None,
                    packet_loss_pct: None,
                    pq_negotiated: None,
                    streams_open: None,
                    datagrams_in: None,
                    datagrams_out: None,
                },
            ],
            devices: vec![
                TopologyDevice {
                    id: "dv1".into(),
                    org_id: "org1".into(),
                    device_name: "lena-mbp".into(),
                    status: "joined".into(),
                    last_seen_at: Some("2100-01-01T00:00:00Z".into()),
                },
                TopologyDevice {
                    id: "dv2".into(),
                    org_id: "org3".into(),
                    device_name: "stale-nas".into(),
                    status: "joined".into(),
                    last_seen_at: Some("2000-01-01T00:00:00Z".into()),
                },
            ],
        };
        let now = parse_date_ms("2026-05-18T12:00:00Z");
        let mut state = LayoutState::default();
        let pos = layout(&map, &mut state, now);
        let expected: &[(&str, f64, f64)] = &[
            ("s:sv1", 800.0, 240.0),
            ("s:sv2", 800.0, 660.0),
            ("a:ag1", 812.282445867956, 148.8382070850495),
            ("a:ag2", 800.6522393511182, 334.9143825100763),
            ("o:org1", 932.3259432662023, 491.8534276788569),
            ("o:org2", 646.6673380942733, 225.34990879701422),
            ("o:org3", 1063.1252528630644, 874.0),
            ("t:tn1", 891.4560918595605, 355.7263169827535),
            ("t:tn2", 881.5687820828201, 567.1297125675774),
            ("t:tn3", 714.6541354180417, 350.1647007373917),
            ("t:tn4", 982.0154761964379, 874.0),
            ("d:dv1", 1024.1135857434688, 478.25163735709253),
            ("d:dv2", 1129.6270878111852, 874.0),
        ];
        let lookup = |id: &str| -> Xy {
            let (kind, rest) = id.split_at(2);
            match kind {
                "s:" => pos.servers[rest],
                "a:" => pos.agents[rest],
                "o:" => pos.orgs[rest],
                _ => pos.leaves[id],
            }
        };
        let mut max = 0.0f64;
        for &(id, ex, ey) in expected {
            let p = lookup(id);
            let d = (p.x - ex).abs().max((p.y - ey).abs());
            if d > max {
                max = d;
            }
            assert!(
                d < 1.0,
                "{id}: got ({}, {}), want ({ex}, {ey}) — off by {d}",
                p.x,
                p.y
            );
        }
        // Bit-for-bit is expected on x86 (same f64 ops); the 1px bar
        // only exists for cross-runtime transcendental drift.
        assert!(max < 1e-6, "max coordinate drift {max}");
    }

    #[test]
    fn format_bytes_matches_tunnels_ts() {
        assert_eq!(format_bytes(0.0), "0 B");
        assert_eq!(format_bytes(-5.0), "0 B");
        assert_eq!(format_bytes(512.0), "512 B");
        assert_eq!(format_bytes(1024.0), "1.0 KB");
        assert_eq!(format_bytes(84.1 * 1024.0 * 1024.0 * 1024.0), "84.1 GB");
        // ≥100 in-unit drops the decimal, like `toFixed(0)`.
        assert_eq!(format_bytes(150.0 * 1024.0), "150 KB");
    }

    #[test]
    fn parse_date_handles_rfc3339() {
        assert_eq!(parse_date_ms("1970-01-01T00:00:00Z"), 0.0);
        assert_eq!(parse_date_ms("2026-05-18T12:00:00Z"), 1_779_105_600_000.0);
        assert_eq!(parse_date_ms("2026-05-18T14:00:00+02:00"), 1_779_105_600_000.0);
        assert_eq!(parse_date_ms("2026-05-18T12:00:00.500Z"), 1_779_105_600_500.0);
        assert!(parse_date_ms("2026-05-18T12:00:00").is_nan());
        assert!(parse_date_ms("garbage").is_nan());
    }

    #[test]
    fn fresh_uses_supplied_clock() {
        let now = parse_date_ms("2026-05-18T12:00:00Z");
        assert!(fresh(Some("2026-05-18T11:59:00Z"), LIVE_HANDSHAKE_MS, now));
        assert!(!fresh(Some("2026-05-18T11:00:00Z"), LIVE_HANDSHAKE_MS, now));
        // Future handshakes are trivially fresh (negative age).
        assert!(fresh(Some("2100-01-01T00:00:00Z"), LIVE_HANDSHAKE_MS, now));
        assert!(!fresh(None, LIVE_HANDSHAKE_MS, now));
    }

    #[test]
    fn org_label_strips_personal_suffix() {
        assert_eq!(org_label("Acme Robotics"), "Acme Robotics");
        assert_eq!(org_label("lena (personal)"), "lena");
        assert_eq!(org_label("lena (Personal)  "), "lena");
        assert_eq!(org_label("personal trainer"), "personal trainer");
    }

    #[test]
    fn hash01_matches_js_fnv1a() {
        // Verified against the JS implementation in Node:
        // hash01("core") = 4235539593 >>> 0 / 2^32.
        let h = |s: &str| {
            let mut h: u32 = 2166136261;
            for c in s.encode_utf16() {
                h ^= c as u32;
                h = h.wrapping_mul(16777619);
            }
            h
        };
        assert_eq!(hash01("abc"), h("abc") as f64 / 4294967296.0);
        assert!(hash01("x") >= 0.0 && hash01("x") < 1.0);
        assert_ne!(hash01("a"), hash01("b"));
    }

    #[test]
    fn bow_serializes_like_the_reference() {
        let d = bow(Xy { x: 100.0, y: 100.0 }, Xy { x: 300.0, y: 200.0 }, 0.18);
        assert_eq!(d, "M 100.0 100.0 Q 308.0 204.0 300.0 200.0");
    }

    fn tiny_map() -> TopologyMap {
        TopologyMap {
            generated_at: "2026-05-18T12:00:00Z".into(),
            servers: vec![TopologyServer {
                id: "sv1".into(),
                name: "fra-wg-02".into(),
                endpoint: "203.0.113.40:51820".into(),
                location: "Frankfurt".into(),
                server_type: "wireguard".into(),
                status: "online".into(),
                mode_mismatch: false,
                agent_id: Some("ag1".into()),
                last_seen_at: Some("2100-01-01T00:00:00Z".into()),
                quic_mode: None,
                backend_tier: None,
                pop_id: None,
                pop_slug: None,
                pop_location: None,
            }],
            agents: vec![TopologyAgent {
                id: "ag1".into(),
                name: "agent-fra".into(),
                status: "online".into(),
                capabilities: vec![],
                hostname: None,
                last_seen_at: Some("2100-01-01T00:00:00Z".into()),
                reported_version: None,
            }],
            orgs: vec![TopologyOrg {
                id: "org1".into(),
                name: "Acme Robotics".into(),
                period_bytes_in: 1024.0 * 1024.0,
                period_bytes_out: 2048.0,
                monthly_limit_bytes: None,
            }],
            tunnels: vec![TopologyTunnel {
                id: "tn1".into(),
                org_id: "org1".into(),
                status: "active".into(),
                tunnel_type: "wireguard".into(),
                server_id: Some("sv1".into()),
                allocated_ip: Some("203.0.113.41".into()),
                last_handshake: Some("2100-01-01T00:00:00Z".into()),
                profile_name: Some("ci-runner-3".into()),
                platform: None,
                last_bytes_in: Some(0.0),
                last_bytes_out: Some(0.0),
                rtt_ms: Some(12.0),
                packet_loss_pct: Some(0.0),
                pq_negotiated: Some(true),
                streams_open: None,
                datagrams_in: None,
                datagrams_out: None,
            }],
            devices: vec![TopologyDevice {
                id: "dv1".into(),
                org_id: "org1".into(),
                device_name: "lena-mbp".into(),
                status: "joined".into(),
                last_seen_at: Some("2000-01-01T00:00:00Z".into()),
            }],
        }
    }

    #[test]
    fn layout_is_deterministic_and_pins_servers() {
        let now = parse_date_ms("2026-05-18T12:00:00Z");
        let run = || {
            let mut state = LayoutState::default();
            layout(&tiny_map(), &mut state, now)
        };
        let a = run();
        let b = run();
        assert_eq!(a, b);
        // The lone server is pinned at angle -π/2 on the inner ring.
        let sv = a.servers["sv1"];
        assert_eq!(sv.x, CX);
        assert_eq!(sv.y, CY - SERVER_R);
        assert!(a.orgs.contains_key("org1"));
        assert!(a.leaves.contains_key("t:tn1"));
        assert!(a.leaves.contains_key("d:dv1"));
        // The stale device parks toward the rim, well outside the org ring.
        let d = a.leaves["d:dv1"];
        let r = ((d.x - CX).powi(2) + (d.y - CY).powi(2)).sqrt();
        assert!(r > ORG_R, "parked device radius {r}");
    }

    #[test]
    fn second_snapshot_reuses_cached_positions() {
        let now = parse_date_ms("2026-05-18T12:00:00Z");
        let mut state = LayoutState::default();
        let first = layout(&tiny_map(), &mut state, now);
        // Same snapshot again: alpha is spent, nodes barely move.
        let second = layout(&tiny_map(), &mut state, now);
        let a = first.orgs["org1"];
        let b = second.orgs["org1"];
        assert!((a.x - b.x).abs() < 1.0 && (a.y - b.y).abs() < 1.0);
    }
}
