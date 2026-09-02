/** Human time for lists. Every formatter is pure over (iso, now). */

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function parse(iso: string): Date | undefined {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? undefined : d;
}

/** "2026-08-28 09:12" in local time; "" for an unparsable stamp. */
export function stamp(iso: string): string {
  const d = parse(iso);
  if (!d) {
    return "";
  }
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** "2026-08-28" in local time; "" for an unparsable stamp. */
export function day(iso: string): string {
  return stamp(iso).slice(0, 10);
}

/** "just now", "40s ago", "12 min ago", "1 hour ago", "2 days ago". */
export function ago(iso: string, nowMs = Date.now()): string {
  const d = parse(iso);
  if (!d) {
    return "";
  }
  const s = Math.max(0, Math.floor((nowMs - d.getTime()) / 1000));
  if (s < 10) {
    return "just now";
  }
  if (s < 60) {
    return `${s}s ago`;
  }
  const m = Math.floor(s / 60);
  if (m < 60) {
    return `${m} min ago`;
  }
  const h = Math.floor(m / 60);
  if (h < 24) {
    return h === 1 ? "1 hour ago" : `${h} hours ago`;
  }
  const days = Math.floor(h / 24);
  return days === 1 ? "1 day ago" : `${days} days ago`;
}

/** "today 08:14", "Aug 30", or "Aug 30, 2025" across a year boundary. */
export function dayLabel(iso: string, nowMs = Date.now()): string {
  const d = parse(iso);
  if (!d) {
    return "";
  }
  const now = new Date(nowMs);
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  if (sameDay) {
    return `today ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
  const md = `${MONTHS[d.getMonth()]} ${d.getDate()}`;
  return d.getFullYear() === now.getFullYear() ? md : `${md}, ${d.getFullYear()}`;
}

/** "4m 20s", "45s", "0s". */
export function countdown(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const m = Math.floor(s / 60);
  return m === 0 ? `${s}s` : `${m}m ${pad(s % 60)}s`;
}

/** "11h 42m left", "42m left", "under a minute left", "expired". */
export function remainingLabel(ms: number): string {
  if (ms <= 0) {
    return "expired";
  }
  const m = Math.floor(ms / 60_000);
  if (m < 1) {
    return "under a minute left";
  }
  const h = Math.floor(m / 60);
  return h === 0 ? `${m}m left` : `${h}h ${m % 60}m left`;
}

/** "a91f…7c4e" for a 64-hex hash; short input is returned as is. */
export function shortHash(hex: string): string {
  return hex.length > 12 ? `${hex.slice(0, 4)}…${hex.slice(-4)}` : hex;
}

/** "x25519 · 4f0c 91ab d7e2 3b58": the first 16 hex of an ephemeral key in groups of four. */
export function keyFingerprint(hex: string): string {
  const head = hex.slice(0, 16);
  const groups = head.match(/.{1,4}/g) ?? [];
  return `x25519 · ${groups.join(" ")}`;
}
