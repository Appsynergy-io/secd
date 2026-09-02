/** Activity: the audit hash chain. Values are never listed. */

import { auditUrl, req } from "../lib/api.ts";
import { el } from "../lib/dom.ts";
import { currentLogoutGen } from "../lib/gen.ts";
import type { AppState, Host } from "../lib/host.ts";
import { shortHash } from "../lib/time.ts";

export const ZERO_HASH = "0".repeat(64);
export const LOADING_SENTENCE = "Loading activity.";
export const LOAD_FAIL_SENTENCE = "Activity did not load.";
export const VERIFIED_TITLE = "Hash chain verified";
export const BROKEN_TITLE = "Hash chain broken";
export const UNVERIFIED = "Unverified";
export const CHAIN_BREAK = "This row does not follow the chain.";
export const FOOT =
  "The chain fails closed: a write the server cannot make, or a head it cannot read, fails the request being recorded. Values are never part of an event.";
export const EM_DASH = "—";

export type AuditEvent = {
  seq: number;
  action: string;
  names: string[];
  sessionId: string | undefined;
  prev: string;
  hash: string;
};

export type AuditChain = {
  events: AuditEvent[];
  head: string;
  verified: boolean;
};

/** Display fields for one table row, in API order. */
export type AuditRowView = {
  seq: string;
  action: string;
  badgeClass: string;
  names: string;
  session: string;
};

type Mem = {
  chain: AuditChain | undefined;
  error: string | undefined;
  loading: boolean;
  loadGen: number;
  root: HTMLElement | undefined;
  host: Host | undefined;
};

const mem = new WeakMap<object, Mem>();

function memOf(state: object): Mem {
  let m = mem.get(state);
  if (m === undefined) {
    m = {
      chain: undefined,
      error: undefined,
      loading: false,
      loadGen: 0,
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

function parseEvent(v: unknown): AuditEvent {
  const rec = asRecord(v);
  if (rec === undefined) {
    return { seq: 0, action: "", names: [], sessionId: undefined, prev: "", hash: "" };
  }
  const seqRaw = rec["seq"];
  const namesRaw = rec["names"];
  const namesOk = Array.isArray(namesRaw) && namesRaw.every((n) => typeof n === "string");
  const sessionRaw = rec["session_id"];
  const actionRaw = rec["action"];
  const prevRaw = rec["prev"];
  const hashRaw = rec["hash"];
  return {
    seq: typeof seqRaw === "number" && Number.isFinite(seqRaw) ? seqRaw : 0,
    action: typeof actionRaw === "string" ? actionRaw : "",
    names: namesOk ? (namesRaw as string[]) : [],
    sessionId: typeof sessionRaw === "string" ? sessionRaw : undefined,
    prev: typeof prevRaw === "string" ? prevRaw : "",
    hash: typeof hashRaw === "string" ? hashRaw : "",
  };
}

/** GET /api/v1/audit body. A missing `events` array is malformed; `value` is ignored. */
export function parseAudit(data: unknown): AuditChain | undefined {
  const rec = asRecord(data);
  const events = rec?.["events"];
  if (rec === undefined || !Array.isArray(events)) {
    return undefined;
  }
  const headRaw = rec["head"];
  return {
    events: events.map(parseEvent),
    head: typeof headRaw === "string" ? headRaw : ZERO_HASH,
    verified: rec["verified"] === true,
  };
}

export function namesLabel(names: readonly string[]): string {
  return names.length > 0 ? names.join(", ") : EM_DASH;
}

export function sessionLabel(sessionId: string | undefined): string {
  return sessionId === undefined || sessionId === "" ? EM_DASH : sessionId;
}

export function actionBadgeClass(action: string): string {
  if (action.startsWith("vault.rollback")) {
    return "badge badge-warn";
  }
  if (action.startsWith("session") || action.startsWith("device")) {
    return "badge badge-accent";
  }
  return "badge";
}

export function auditRow(ev: AuditEvent): AuditRowView {
  return {
    seq: String(ev.seq),
    action: ev.action,
    badgeClass: actionBadgeClass(ev.action),
    names: namesLabel(ev.names),
    session: sessionLabel(ev.sessionId),
  };
}

export function renderActivity(state: AppState, root: HTMLElement, host: Host): void {
  const m = memOf(state);
  m.root = root;
  m.host = host;
  paint(state);
  void loadActivity(state);
}

export function leaveActivity(state: object): void {
  const m = memOf(state);
  m.loadGen += 1;
  m.chain = undefined;
  m.error = undefined;
  m.loading = false;
  m.root = undefined;
  m.host = undefined;
}

function guard(m: Mem): () => boolean {
  const gen = currentLogoutGen();
  const loadGen = m.loadGen;
  return () => gen !== currentLogoutGen() || loadGen !== m.loadGen;
}

async function loadActivity(state: AppState): Promise<void> {
  const m = memOf(state);
  if (m.chain !== undefined || m.loading) {
    return;
  }
  m.loading = true;
  m.error = undefined;
  paint(state);
  const stale = guard(m);
  try {
    const res = await req("GET", auditUrl());
    if (stale()) {
      return;
    }
    if (res.status === 401 || res.status === 403) {
      void m.host?.signOut();
      return;
    }
    if (res.status !== 200) {
      m.error = LOAD_FAIL_SENTENCE;
      return;
    }
    const parsed = parseAudit(res.data);
    if (parsed === undefined) {
      m.error = LOAD_FAIL_SENTENCE;
      return;
    }
    m.chain = parsed;
    m.error = undefined;
    state.counts.set({ ...state.counts.get(), activity: parsed.events.length });
  } catch {
    if (!stale()) {
      m.error = LOAD_FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.loading = false;
      paint(state);
    }
  }
}

function paint(state: AppState): void {
  const m = memOf(state);
  const root = m.root;
  if (root === undefined) {
    return;
  }
  if (m.chain === undefined) {
    if (m.error !== undefined) {
      root.replaceChildren(
        el("div", { class: "page", "data-width": "1000", "data-state": "error" }, [
          el("div", { class: "alert alert-danger", role: "alert", "data-error": "" }, [m.error]),
        ]),
      );
      return;
    }
    root.replaceChildren(
      el("div", { class: "page", "data-width": "1000", "data-state": "loading" }, [
        el("div", { class: "empty", "data-state": "loading" }, [LOADING_SENTENCE]),
      ]),
    );
    return;
  }
  root.replaceChildren(
    el("div", { class: "page", "data-width": "1000", "data-state": "ready" }, [
      bannerEl(m.chain),
      tableEl(m.chain.events),
      el("div", { class: "page-foot" }, [FOOT]),
    ]),
  );
}

function bannerEl(chain: AuditChain): HTMLElement {
  const n = chain.events.length;
  const sub = `${n} events · head ${shortHash(chain.head)}`;
  return el(
    "div",
    {
      class: "verified-bar",
      "data-verified": chain.verified ? "true" : "false",
      "data-chain-status": chain.verified ? "verified" : "unverified",
    },
    [
      el("span", { class: "verified-dot" }),
      el("div", { class: "verified-title" }, [chain.verified ? VERIFIED_TITLE : BROKEN_TITLE]),
      el("div", { class: "verified-sub" }, [chain.verified ? sub : `${UNVERIFIED}. ${sub}`]),
    ],
  );
}

function tableEl(events: readonly AuditEvent[]): HTMLElement {
  const rows: Array<Node | string> = [
    el("div", { class: "grid grid-head cols-audit" }, [
      el("div", {}, ["Seq"]),
      el("div", {}, ["Action"]),
      el("div", {}, ["Names"]),
      el("div", {}, ["Session"]),
    ]),
  ];
  for (const ev of events) {
    const row = auditRow(ev);
    rows.push(
      el("div", { class: "grid cols-audit", "data-seq": row.seq }, [
        el("div", { class: "cell-seq" }, [row.seq]),
        el("div", {}, [el("span", { class: row.badgeClass }, [row.action])]),
        el("div", { class: "cell-names truncate" }, [row.names]),
        el("div", { class: "cell-mono-xs truncate" }, [row.session]),
      ]),
    );
  }
  return el("div", { class: "card", "data-list": "audit" }, rows);
}
