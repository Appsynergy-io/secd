/** Devices: long-lived device sessions. Approving happens on /device. */

import {
  FAIL_SENTENCE,
  RATE_SENTENCE,
  devicePendingUrl,
  errorMessage,
  req,
  sessionRevokePath,
  sessionsUrl,
} from "../lib/api.ts";
import { el } from "../lib/dom.ts";
import { currentLogoutGen } from "../lib/gen.ts";
import type { AppState, Host } from "../lib/host.ts";
import { ago, countdown, dayLabel } from "../lib/time.ts";

export const POLL_MS = 5_000;
export const LOADING_SENTENCE = "Loading devices.";
export const EMPTY_SENTENCE = "No devices yet.";
export const DEVICES_TITLE = "Authorized devices";
export const DEVICES_HINT = "device sessions expire after 30 days";
export const PENDING_LABEL = "Approval requested";
export const OPEN_LABEL = "Open approval page";

export type PendingRequest = {
  user_code: string;
  hostname: string;
  eph_pub: string;
  created: string;
  expires_in: number;
};

export type SessionRow = {
  id: string;
  kind: string;
  label: string;
  created: string;
  last_seen: string;
  current: boolean;
};

type Mem = {
  pendingRows: PendingRequest[] | undefined;
  sessions: SessionRow[] | undefined;
  revoked: Set<string>;
  error: string | undefined;
  busy: boolean;
  loadingPending: boolean;
  loadingSessions: boolean;
  loadGen: number;
  poll: ReturnType<typeof setInterval> | undefined;
  root: HTMLElement | undefined;
  host: Host | undefined;
};

const mem = new WeakMap<object, Mem>();

function memOf(state: object): Mem {
  let m = mem.get(state);
  if (m === undefined) {
    m = {
      pendingRows: undefined,
      sessions: undefined,
      revoked: new Set(),
      error: undefined,
      busy: false,
      loadingPending: false,
      loadingSessions: false,
      loadGen: 0,
      poll: undefined,
      root: undefined,
      host: undefined,
    };
    mem.set(state, m);
  }
  return m;
}

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return typeof v === "object" && v !== null ? (v as Record<string, unknown>) : undefined;
}

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function secs(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) ? Math.max(0, Math.floor(v)) : 0;
}

export function parsePending(v: unknown): PendingRequest[] {
  const rows = asRecord(v)?.["pending"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: PendingRequest[] = [];
  for (const row of rows) {
    const r = asRecord(row);
    const user_code = r === undefined ? "" : str(r["user_code"]);
    if (user_code === "") {
      continue;
    }
    out.push({
      user_code,
      hostname: str(r?.["hostname"]),
      eph_pub: str(r?.["eph_pub"]),
      created: str(r?.["created"]),
      expires_in: secs(r?.["expires_in"]),
    });
  }
  return out;
}

export function parseSessions(v: unknown): SessionRow[] {
  const rows = asRecord(v)?.["sessions"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: SessionRow[] = [];
  for (const row of rows) {
    const r = asRecord(row);
    const id = r === undefined ? "" : str(r["id"]);
    if (id === "") {
      continue;
    }
    out.push({
      id,
      kind: str(r?.["kind"]),
      label: str(r?.["label"]),
      created: str(r?.["created"]),
      last_seen: str(r?.["last_seen"]),
      current: r?.["current"] === true,
    });
  }
  return out;
}

/** Device sessions only; console sessions live on Access. */
export function deviceSessions(rows: readonly SessionRow[]): SessionRow[] {
  return rows.filter((s) => s.kind === "device");
}

export function pendingMeta(row: PendingRequest, nowMs = Date.now()): string {
  return `${row.hostname} · requested ${ago(row.created, nowMs)} · expires in ${countdown(row.expires_in)}`;
}

/** The banner on this screen; the requests themselves are listed on /device. */
export function pendingSentence(count: number): string {
  return count === 1
    ? "1 device is waiting for approval."
    : `${count} devices are waiting for approval.`;
}

export function sessionLabel(row: SessionRow): string {
  return row.label === "" ? row.id : row.label;
}

export function revokedToast(row: SessionRow): string {
  return `Revoked device session ${sessionLabel(row)}`;
}

export function failSentence(status: number, data?: unknown): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  return errorMessage(data) ?? FAIL_SENTENCE;
}

export function renderDevices(state: AppState, root: HTMLElement, host: Host): void {
  const m = memOf(state);
  m.root = root;
  m.host = host;
  paint(state);
  void loadPending(state);
  void loadSessions(state);
  if (m.poll === undefined) {
    m.poll = setInterval(() => {
      void loadPending(state, true);
    }, POLL_MS);
  }
}

export function leaveDevices(state: object): void {
  const m = memOf(state);
  m.loadGen += 1;
  if (m.poll !== undefined) {
    clearInterval(m.poll);
    m.poll = undefined;
  }
  m.pendingRows = undefined;
  m.sessions = undefined;
  m.revoked = new Set();
  m.error = undefined;
  m.busy = false;
  m.loadingPending = false;
  m.loadingSessions = false;
  m.root = undefined;
  m.host = undefined;
}

function guard(m: Mem): () => boolean {
  const gen = currentLogoutGen();
  const loadGen = m.loadGen;
  return () => gen !== currentLogoutGen() || loadGen !== m.loadGen;
}

function isAuthFail(status: number): boolean {
  return status === 401 || status === 403;
}

function applySessions(state: AppState, m: Mem, rows: SessionRow[]): void {
  const kept = rows.filter((r) => !m.revoked.has(r.id));
  m.sessions = kept;
  state.counts.set({ ...state.counts.get(), devices: deviceSessions(kept).length });
}

async function loadPending(state: AppState, force = false): Promise<void> {
  const m = memOf(state);
  if (m.loadingPending) {
    return;
  }
  if (!force && m.pendingRows !== undefined) {
    return;
  }
  m.loadingPending = true;
  const stale = guard(m);
  try {
    const res = await req("GET", devicePendingUrl());
    if (stale()) {
      return;
    }
    if (isAuthFail(res.status)) {
      void m.host?.signOut();
      return;
    }
    if (res.status !== 200) {
      m.error = failSentence(res.status, res.data);
      return;
    }
    m.pendingRows = parsePending(res.data);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.loadingPending = false;
      paint(state);
    }
  }
}

async function loadSessions(state: AppState, force = false): Promise<void> {
  const m = memOf(state);
  if (m.loadingSessions) {
    return;
  }
  if (!force && m.sessions !== undefined) {
    return;
  }
  m.loadingSessions = true;
  if (m.sessions === undefined) {
    paint(state);
  }
  const stale = guard(m);
  try {
    const res = await req("GET", sessionsUrl());
    if (stale()) {
      return;
    }
    if (isAuthFail(res.status)) {
      void m.host?.signOut();
      return;
    }
    if (res.status !== 200) {
      m.error = failSentence(res.status, res.data);
      return;
    }
    applySessions(state, m, parseSessions(res.data));
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.loadingSessions = false;
      paint(state);
    }
  }
}

async function onRevoke(state: AppState, row: SessionRow): Promise<void> {
  const m = memOf(state);
  if (m.busy) {
    return;
  }
  m.busy = true;
  m.error = undefined;
  paint(state);
  const stale = guard(m);
  try {
    const res = await req("DELETE", sessionRevokePath(row.id));
    if (stale()) {
      return;
    }
    if (isAuthFail(res.status)) {
      void m.host?.signOut();
      return;
    }
    if (res.status !== 200) {
      m.error = failSentence(res.status, res.data);
      return;
    }
    m.revoked.add(row.id);
    applySessions(state, m, m.sessions ?? []);
    m.host?.flash(revokedToast(row));
    m.busy = false;
    await loadSessions(state, true);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.busy = false;
      paint(state);
    }
  }
}

function focusKey(root: HTMLElement): { action: string; row: string } | undefined {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement) || !root.contains(active)) {
    return undefined;
  }
  const action = active.getAttribute("data-action");
  if (action === null) {
    return undefined;
  }
  const row = active.closest("[data-session-id]")?.getAttribute("data-session-id") ?? "";
  return { action, row };
}

function restoreFocus(root: HTMLElement, key: { action: string; row: string } | undefined): void {
  if (key === undefined) {
    return;
  }
  const scope =
    key.row === "" ? root : (root.querySelector(`[data-session-id="${key.row}"]`) ?? root);
  const found = scope.querySelector(`[data-action="${key.action}"]`);
  if (found instanceof HTMLElement && !(found instanceof HTMLButtonElement && found.disabled)) {
    found.focus();
  }
}

function paint(state: AppState): void {
  const m = memOf(state);
  const root = m.root;
  if (root === undefined) {
    return;
  }
  const key = focusKey(root);
  const now = Date.now();
  const children: HTMLElement[] = [];
  if (m.error !== undefined) {
    children.push(
      el("div", { class: "alert alert-danger", role: "alert", "data-error": "" }, [m.error]),
    );
  }
  const waiting = m.pendingRows ?? [];
  if (waiting.length > 0) {
    children.push(pendingBanner(m, waiting.length));
  }
  children.push(devicesCard(state, m, now));
  root.replaceChildren(
    el("div", { class: "page", "data-width": "900" }, [el("div", { class: "stack" }, children)]),
  );
  restoreFocus(root, key);
}

/** No request is approved or denied here: the banner only points at /device. */
function pendingBanner(m: Mem, count: number): HTMLElement {
  const open = el("button", { type: "button", class: "btn btn-primary", "data-action": "open" }, [
    OPEN_LABEL,
  ]);
  open.addEventListener("click", () => {
    m.host?.navigate("/device");
  });
  return el("div", { class: "pending-card", "data-pending": "" }, [
    el("div", {}, [
      el("div", { class: "pending-label" }, [PENDING_LABEL]),
      el("div", { class: "pending-meta" }, [pendingSentence(count)]),
    ]),
    el("div", { class: "pending-actions" }, [open]),
  ]);
}

function devicesCard(state: AppState, m: Mem, nowMs: number): HTMLElement {
  const body: HTMLElement[] = [
    el("div", { class: "card-head" }, [
      el("div", { class: "card-title" }, [DEVICES_TITLE]),
      el("div", { class: "card-hint" }, [DEVICES_HINT]),
    ]),
  ];
  if (m.sessions === undefined) {
    if (m.error === undefined) {
      body.push(el("div", { class: "empty", "data-state": "loading" }, [LOADING_SENTENCE]));
    }
  } else {
    const rows = deviceSessions(m.sessions);
    if (rows.length === 0) {
      body.push(el("div", { class: "empty", "data-state": "empty" }, [EMPTY_SENTENCE]));
    } else {
      for (const row of rows) {
        body.push(sessionRowEl(state, m, row, nowMs));
      }
    }
  }
  return el(
    "section",
    { class: "card", "data-card": "devices", "aria-label": DEVICES_TITLE },
    body,
  );
}

function sessionRowEl(state: AppState, m: Mem, row: SessionRow, nowMs: number): HTMLElement {
  const revoke = el(
    "button",
    {
      type: "button",
      class: "btn btn-sm btn-danger",
      "data-action": "revoke",
      disabled: m.busy ? true : undefined,
    },
    ["Revoke"],
  );
  revoke.disabled = m.busy;
  revoke.addEventListener("click", () => {
    void onRevoke(state, row);
  });
  return el("div", { class: "grid cols-sessions", "data-session-id": row.id }, [
    el("div", { class: "cell-mono truncate" }, [sessionLabel(row)]),
    el("div", { class: "cell-muted" }, [`approved ${dayLabel(row.created, nowMs)}`]),
    el("div", { class: "cell-muted" }, [`last seen ${ago(row.last_seen, nowMs)}`]),
    revoke,
  ]);
}
