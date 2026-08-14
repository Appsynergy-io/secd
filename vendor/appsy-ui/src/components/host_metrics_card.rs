//! HostMetricsCard — port of `platform/host-metrics-card.tsx`: live host
//! telemetry for one fleet node (CPU, memory, disk, RAID, network). Rates
//! need two samples; `previous: None` renders "—" until one lands. All
//! number formatting mirrors JS semantics: `toFixed` rounds ties away from
//! zero (NOT Rust's `{:.1}` half-to-even) and `Math.round` is `+0.5 floor`.

use crate::components::card::CARD;
use crate::components::ip_chip::{Chip, ChipTone};
use crate::icons::{Icon, RI_CPU_LINE, RI_EXCHANGE_2_LINE, RI_HARD_DRIVE_LINE, RI_RAM_2_LINE};
use leptos::either::Either;
use leptos::prelude::*;

pub const HMC: &str = "asy-hmc";
pub const HMC_TITLE: &str = "asy-hmc__title";
pub const HMC_SECTION: &str = "asy-hmc__section";
pub const HMC_SECTION_HEAD: &str = "asy-hmc__section-head";
pub const HMC_SECTION_GLYPH: &str = "asy-hmc__section-glyph";
pub const HMC_SECTION_TITLE: &str = "asy-hmc__section-title";
pub const HMC_STAT: &str = "asy-hmc__stat";
pub const HMC_STAT_LABEL: &str = "asy-hmc__stat-label";
pub const HMC_BAR: &str = "asy-hmc__bar";
pub const HMC_BAR_FILL: &str = "asy-hmc__bar-fill";
pub const HMC_NOTE: &str = "asy-hmc__note";
pub const HMC_ROWS: &str = "asy-hmc__rows";
pub const HMC_ROW: &str = "asy-hmc__row";
pub const HMC_ROW_HEAD: &str = "asy-hmc__row-head";
pub const HMC_ROW_META: &str = "asy-hmc__row-meta";

const DASH: &str = "\u{2014}";

// ---------------------------------------------------------------- data shapes

#[derive(Clone, PartialEq, Debug, Default)]
pub struct LoadAvg {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

/// A cumulative CPU jiffie counter pair (`idle` folds in iowait).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CpuSample {
    pub total: f64,
    pub idle: f64,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct MemoryMetrics {
    pub total_bytes: f64,
    pub available_bytes: f64,
    pub used_bytes: f64,
    pub swap_total_bytes: f64,
    pub swap_free_bytes: f64,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct FilesystemMetrics {
    pub mount_point: String,
    pub fstype: String,
    pub total_bytes: f64,
    pub available_bytes: f64,
    pub used_bytes: f64,
    pub inodes_total: f64,
    pub inodes_free: f64,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct InterfaceMetrics {
    pub name: String,
    pub rx_bytes: f64,
    pub tx_bytes: f64,
    pub rx_packets: f64,
    pub tx_packets: f64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct MdArrayMetrics {
    pub name: String,
    pub level: String,
    pub degraded: bool,
    pub disks_active: u32,
    pub disks_total: u32,
    pub sync_percent: Option<f64>,
}

/// `AgentHostMetrics` upstream.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct HostMetrics {
    pub uptime_secs: Option<f64>,
    pub load_avg: Option<LoadAvg>,
    pub cpu_cores: Option<u32>,
    pub cpu: Option<CpuSample>,
    pub memory: Option<MemoryMetrics>,
    pub filesystems: Vec<FilesystemMetrics>,
    pub interfaces: Vec<InterfaceMetrics>,
    pub md_arrays: Vec<MdArrayMetrics>,
}

// ------------------------------------------------------------- number maths

/// JS `Number.prototype.toFixed` for the positive domain: round-half-up on
/// the EXACT decimal expansion of the double. A scaled float multiply is
/// wrong here — `1.42/4` is 0.35499…, but `×100` rounds to exactly 35.5 and
/// a naive `+0.5 floor` bumps it; JS reads the true digits and says "0.35".
pub(crate) fn to_fixed(x: f64, digits: u32) -> String {
    let d = digits as usize;
    // 30 guard digits: positions 1..d+1 of this printout are exact.
    let s = format!("{x:.prec$}", prec = d + 30);
    let dot = s.find('.').expect("invariant: fixed-precision format has a dot");
    let (int_part, frac) = (&s[..dot], &s[dot + 1..]);
    let mut n: i128 = format!("{int_part}{}", &frac[..d])
        .parse()
        .expect("invariant: digits parse");
    if frac.as_bytes()[d] >= b'5' {
        n += 1;
    }
    if d == 0 {
        return n.to_string();
    }
    let scale = 10i128.pow(digits);
    format!("{}.{:0width$}", n / scale, n % scale, width = d)
}

const BYTE_UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];

/// Binary-prefixed byte size (`formatBytes` upstream).
pub fn format_bytes(bytes: f64) -> String {
    if !bytes.is_finite() || bytes <= 0.0 {
        return "0 B".to_owned();
    }
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < BYTE_UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", to_fixed(value, 0))
    } else {
        format!("{} {}", to_fixed(value, 1), BYTE_UNITS[unit])
    }
}

/// `formatBps` upstream.
pub fn format_bps(bytes_per_sec: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec))
}

/// `pct` upstream: `Math.round((used/total)*1000)/10`, `None` without a
/// denominator.
pub fn pct(used: f64, total: f64) -> Option<f64> {
    if !used.is_finite() || !total.is_finite() || total <= 0.0 {
        return None;
    }
    Some(((used / total) * 1000.0 + 0.5).floor() / 10.0)
}

/// `cpuUtilPct` upstream: `(1 - Δidle/Δtotal) * 100` clamped `0..100`.
pub fn cpu_util_pct(prev: Option<CpuSample>, cur: Option<CpuSample>) -> Option<f64> {
    let (prev, cur) = (prev?, cur?);
    let d_total = cur.total - prev.total;
    let d_idle = cur.idle - prev.idle;
    if d_total <= 0.0 || d_idle < 0.0 {
        return None;
    }
    Some(((1.0 - d_idle / d_total) * 100.0).clamp(0.0, 100.0))
}

/// `ifaceThroughput` upstream: `Δbytes / dtSeconds`, `None` on reset/missing.
pub fn iface_throughput(prev_bytes: Option<f64>, cur_bytes: Option<f64>, dt_seconds: f64) -> Option<f64> {
    let (prev, cur) = (prev_bytes?, cur_bytes?);
    if dt_seconds <= 0.0 {
        return None;
    }
    let delta = cur - prev;
    if delta < 0.0 {
        return None;
    }
    Some(delta / dt_seconds)
}

fn pct_label(value: Option<f64>) -> String {
    match value {
        None => DASH.to_owned(),
        Some(v) => format!("{}%", to_fixed(v, 1)),
    }
}

// ------------------------------------------------------------------- pieces

fn section(icon: &'static str, title: &'static str, body: AnyView) -> AnyView {
    (view! {
        <div class=HMC_SECTION>
            <div class=HMC_SECTION_HEAD>
                <Icon d=icon class=HMC_SECTION_GLYPH />
                <span class=HMC_SECTION_TITLE>{title}</span>
            </div>
            {body}
        </div>
    })
    .into_any()
}

fn stat(label: impl IntoView + 'static, value: String) -> AnyView {
    (view! {
        <div class=HMC_STAT>
            <span class=HMC_STAT_LABEL>{label}</span>
            <span class="mono">{value}</span>
        </div>
    })
    .into_any()
}

fn usage_bar(percent: Option<f64>) -> AnyView {
    let width = percent.map_or(0.0, |p| p.clamp(0.0, 100.0));
    (view! {
        <div class=HMC_BAR>
            <div class=HMC_BAR_FILL style:width=format!("{width}%")></div>
        </div>
    })
    .into_any()
}

fn muted_note(children: AnyView) -> AnyView {
    (view! { <span class=HMC_NOTE>{children}</span> }).into_any()
}

#[component]
pub fn HostMetricsCard(
    current: HostMetrics,
    #[prop(optional)] previous: Option<HostMetrics>,
    dt_seconds: f64,
) -> impl IntoView {
    // CPU
    let util = cpu_util_pct(previous.as_ref().and_then(|p| p.cpu), current.cpu);
    let load_label = current.load_avg.as_ref().map_or(DASH.to_owned(), |l| {
        format!("{} / {} / {}", to_fixed(l.one, 2), to_fixed(l.five, 2), to_fixed(l.fifteen, 2))
    });
    let cores_label = current.cpu_cores.map_or(DASH.to_owned(), |c| c.to_string());
    let per_core = match (&current.load_avg, current.cpu_cores) {
        (Some(load), Some(cores)) if cores > 0 => {
            Some(format!("{} / core", to_fixed(load.one / f64::from(cores), 2)))
        }
        _ => None,
    };
    let cpu_body = (view! {
        {stat("Utilisation", util.map_or(DASH.to_owned(), |u| format!("{}%", to_fixed(u, 1))))}
        {stat("Load (1 / 5 / 15m)", load_label)}
        {stat("Cores", cores_label)}
        {per_core.map(|v| stat("Per-core load", v))}
    })
    .into_any();

    // Memory
    let memory_body = match &current.memory {
        None => muted_note(DASH.into_any()),
        Some(m) => {
            let used_pct = pct(m.used_bytes, m.total_bytes);
            let swap_used = m.swap_total_bytes - m.swap_free_bytes;
            (view! {
                {stat(
                    "Used",
                    format!(
                        "{} / {} ({})",
                        format_bytes(m.used_bytes),
                        format_bytes(m.total_bytes),
                        pct_label(used_pct),
                    ),
                )}
                {usage_bar(used_pct)}
                {stat("Available", format_bytes(m.available_bytes))}
                {(m.swap_total_bytes > 0.0)
                    .then(|| stat(
                        "Swap",
                        format!("{} / {}", format_bytes(swap_used), format_bytes(m.swap_total_bytes)),
                    ))}
            })
            .into_any()
        }
    };

    // Disk
    let disk_body = if current.filesystems.is_empty() {
        muted_note(DASH.into_any())
    } else {
        (view! {
            <div class=HMC_ROWS>
                {current
                    .filesystems
                    .iter()
                    .map(|fs| {
                        let used_pct = pct(fs.used_bytes, fs.total_bytes);
                        let inode_pct = pct(fs.inodes_total - fs.inodes_free, fs.inodes_total);
                        view! {
                            <div class=HMC_ROW>
                                <div class=HMC_ROW_HEAD>
                                    <span class="mono">{fs.mount_point.clone()}</span>
                                    <span class=HMC_ROW_META>{fs.fstype.clone()}</span>
                                </div>
                                {stat(
                                    "Used",
                                    format!(
                                        "{} / {} ({})",
                                        format_bytes(fs.used_bytes),
                                        format_bytes(fs.total_bytes),
                                        pct_label(used_pct),
                                    ),
                                )}
                                {usage_bar(used_pct)}
                                {stat("Inodes", pct_label(inode_pct))}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        })
        .into_any()
    };

    // RAID — only rendered when the node reports arrays.
    let md_arrays = current.md_arrays.clone();
    let raid = (!md_arrays.is_empty()).then(move || {
        let rows = md_arrays
            .into_iter()
            .map(|array| {
                let level = if array.level.is_empty() { DASH.to_owned() } else { array.level.clone() };
                let name = array.name.clone();
                let chip = if array.degraded {
                    Either::Left(view! {
                        <Chip tone=ChipTone::Bad>
                            "degraded " {array.disks_active.to_string()} "/"
                            {array.disks_total.to_string()}
                        </Chip>
                    })
                } else {
                    Either::Right(view! {
                        <Chip tone=ChipTone::Ok>
                            "healthy " {array.disks_active.to_string()} "/"
                            {array.disks_total.to_string()}
                        </Chip>
                    })
                };
                view! {
                    <div class=HMC_ROW>
                        <div class=HMC_ROW_HEAD>
                            <span class="mono">{name} " \u{b7} " {level}</span>
                            {chip}
                        </div>
                        {array
                            .sync_percent
                            .map(|_| {
                                view! {
                                    {stat("Resync", pct_label(array.sync_percent))}
                                    {usage_bar(array.sync_percent)}
                                }
                            })}
                    </div>
                }
            })
            .collect_view();
        section(
            RI_HARD_DRIVE_LINE,
            "RAID",
            (view! { <div class=HMC_ROWS>{rows}</div> }).into_any(),
        )
    });

    // Network
    let prev_ifaces: Vec<InterfaceMetrics> =
        previous.as_ref().map(|p| p.interfaces.clone()).unwrap_or_default();
    let network_body = if current.interfaces.is_empty() {
        muted_note(DASH.into_any())
    } else {
        (view! {
            <div class=HMC_ROWS>
                {current
                    .interfaces
                    .iter()
                    .map(|iface| {
                        let prev = prev_ifaces.iter().find(|p| p.name == iface.name);
                        let rx = iface_throughput(prev.map(|p| p.rx_bytes), Some(iface.rx_bytes), dt_seconds);
                        let tx = iface_throughput(prev.map(|p| p.tx_bytes), Some(iface.tx_bytes), dt_seconds);
                        let errors = iface.rx_errors + iface.tx_errors;
                        let dropped = iface.rx_dropped + iface.tx_dropped;
                        let faulty = errors > 0 || dropped > 0;
                        view! {
                            <div class=HMC_ROW>
                                <div class=HMC_ROW_HEAD>
                                    <span class="mono">{iface.name.clone()}</span>
                                    {faulty
                                        .then(|| {
                                            view! {
                                                <Chip tone=ChipTone::Bad>
                                                    {errors.to_string()} " err \u{b7} "
                                                    {dropped.to_string()} " drop"
                                                </Chip>
                                            }
                                        })}
                                </div>
                                {stat(
                                    "Down / Up",
                                    format!(
                                        "{} / {}",
                                        rx.map_or(DASH.to_owned(), format_bps),
                                        tx.map_or(DASH.to_owned(), format_bps),
                                    ),
                                )}
                                {faulty
                                    .then(|| {
                                        muted_note(
                                            (view! {
                                                {errors.to_string()} " errors, "
                                                {dropped.to_string()} " dropped (cumulative)"
                                            })
                                                .into_any(),
                                        )
                                    })}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        })
        .into_any()
    };

    view! {
        <div class=format!("{CARD} {HMC}")>
            <span class=HMC_TITLE>"Host metrics"</span>
            {section(RI_CPU_LINE, "CPU", cpu_body)}
            {section(RI_RAM_2_LINE, "Memory", memory_body)}
            {section(RI_HARD_DRIVE_LINE, "Disk", disk_body)}
            {raid}
            {section(RI_EXCHANGE_2_LINE, "Network", network_body)}
        </div>
    }
}

/// Card `flex flex-col gap-4 p-4`; sections `flex flex-col gap-2` with
/// `size-3.5` muted glyph + 14px/500 title; stat rows `flex items-center
/// justify-between text-[12.5px]` (muted label, mono value); usage bar
/// `h-1.5 w-full overflow-hidden rounded-full bg-surface-2` with accent
/// fill; sub-rows `gap-1.5 border-t border-border-soft pt-2
/// first:border-t-0 first:pt-0`.
pub fn css() -> String {
    format!(
        ".{HMC}{{display:flex;flex-direction:column;gap:1rem;padding:1rem}}\
.{HMC_TITLE}{{font-size:14px;font-weight:500}}\
.{HMC_SECTION}{{display:flex;flex-direction:column;gap:.5rem}}\
.{HMC_SECTION_HEAD}{{display:flex;align-items:center;gap:.375rem}}\
.{HMC_SECTION_GLYPH}{{width:.875rem;height:.875rem;color:var(--color-text-muted)}}\
.{HMC_SECTION_TITLE}{{font-size:14px;font-weight:500}}\
.{HMC_STAT}{{display:flex;align-items:center;justify-content:space-between;\
gap:.5rem;font-size:12.5px}}\
.{HMC_STAT_LABEL}{{flex-shrink:0;color:var(--color-text-muted)}}\
.{HMC_STAT}>.mono{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap}}\
.{HMC_BAR}{{height:.375rem;width:100%;overflow:hidden;\
border-radius:calc(infinity * 1px);background-color:var(--color-surface-2)}}\
.{HMC_BAR_FILL}{{height:100%;border-radius:calc(infinity * 1px);\
background-color:var(--color-accent)}}\
.{HMC_NOTE}{{font-size:12.5px;color:var(--color-text-muted)}}\
.{HMC_ROWS}{{display:flex;flex-direction:column;gap:.5rem}}\
.{HMC_ROW}{{display:flex;flex-direction:column;gap:.375rem;\
border-color:var(--color-border-soft);border-top-width:1px;padding-top:.5rem}}\
.{HMC_ROW}:first-child{{border-top-width:0;padding-top:0}}\
.{HMC_ROW_HEAD}{{display:flex;align-items:center;justify-content:space-between;\
gap:.5rem;font-size:12.5px}}\
.{HMC_ROW_HEAD}>.mono{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap}}\
.{HMC_ROW_META}{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;color:var(--color-text-muted)}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            HMC,
            HMC_TITLE,
            HMC_SECTION,
            HMC_SECTION_HEAD,
            HMC_SECTION_GLYPH,
            HMC_SECTION_TITLE,
            HMC_STAT,
            HMC_STAT_LABEL,
            HMC_BAR,
            HMC_BAR_FILL,
            HMC_NOTE,
            HMC_ROWS,
            HMC_ROW,
            HMC_ROW_HEAD,
            HMC_ROW_META,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn to_fixed_matches_js_ties_away_from_zero() {
        assert_eq!(to_fixed(781.25, 1), "781.3"); // Rust {:.1} would say 781.2
        assert_eq!(to_fixed(1.1, 2), "1.10");
        assert_eq!(to_fixed(0.25, 1), "0.3");
        assert_eq!(to_fixed(30.0, 1), "30.0");
    }

    #[test]
    fn helpers_match_reference_vectors() {
        assert_eq!(format_bytes(0.0), "0 B");
        assert_eq!(format_bytes(512.0), "512 B");
        assert_eq!(format_bytes(16.0 * 1024.0 * 1024.0 * 1024.0), "16.0 GiB");
        assert_eq!(format_bps(2_400_000.0), "2.3 MiB/s");
        assert_eq!(format_bps(800_000.0), "781.3 KiB/s");
        assert_eq!(pct(10.0, 16.0), Some(62.5));
        assert_eq!(pct(460_000.0, 2_560_000.0), Some(18.0));
        assert_eq!(pct(1.0, 0.0), None);
        assert_eq!(
            cpu_util_pct(
                Some(CpuSample { total: 3_000_000.0, idle: 2_100_000.0 }),
                Some(CpuSample { total: 4_000_000.0, idle: 2_800_000.0 }),
            ),
            Some(30.000000000000004) // (1 - 700000/1000000)*100 in f64, as JS computes it
        );
        assert_eq!(iface_throughput(Some(4_988_000_000.0), Some(5_000_000_000.0), 5.0), Some(2_400_000.0));
        assert_eq!(iface_throughput(None, Some(1.0), 5.0), None);
    }
}
