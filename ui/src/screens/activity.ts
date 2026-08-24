/** Activity: audit metadata, with the hash chain recomputed in this tab. */

import { sha256 } from "@noble/hashes/sha2.js";
import {
  BREAKPOINT_PX,
  FAIL_SENTENCE,
  RATE_SENTENCE,
  auditUrl,
  layoutMode,
  req,
  type LayoutMode,
} from "../lib/api.ts";
import { copyText } from "../lib/clipboard.ts";
import type { Signal } from "../lib/signal.ts";

export const ZERO_HASH = "0".repeat(64);
export const CLIP_FAIL_SENTENCE =
  "The browser refused the clipboard. Select the value and copy it.";
export const COPY_NEED_SENTENCE = "Select an event to copy its hash.";
export const LOADING_SENTENCE = "Loading activity.";
export const EMPTY_TITLE = "No events";
export const EMPTY_BODY = "Actions on this vault will show up here.";
export const SELECT_TITLE = "Select an event";
export const SELECT_BODY = "Choose a row from the list.";
export const SUBTITLE = "Audit metadata. Values are never listed.";
export const VERIFIED = "Verified";
export const UNVERIFIED = "Unverified";
export const CHAIN_BREAK = "This row does not follow the chain.";
export const CHAIN_LATER = "The chain broke at an earlier row.";
export const MALFORMED_REASON = "This row is not a valid event.";

export type ActivityHost = {
  path: Signal<string>;
  error: Signal<string | undefined>;
  pending: Signal<boolean>;
};

export type AuditEvent = {
  action: string;
  names: string[];
  sessionId: string | undefined;
  prev: string | undefined;
  hash: string | undefined;
  ok: boolean;
};

export type ChainRow = {
  action: string;
  names: string[];
  sessionId: string | undefined;
  prev: string;
  hash: string;
  verified: boolean;
  reason: string | undefined;
};

type Mem = {
  rows: ChainRow[] | undefined;
  selected: number | undefined;
  copied: boolean;
  clipFail: boolean;
  loading: boolean;
  attempted: boolean;
  gen: number;
  nav: HTMLElement | undefined;
  widthPx: number | undefined;
};

const mem = new WeakMap<object, Mem>();
const focusHints = new WeakMap<object, "copy" | "hash">();

function memOf(state: object): Mem {
  let m = mem.get(state);
  if (m === undefined) {
    m = {
      rows: undefined,
      selected: undefined,
      copied: false,
      clipFail: false,
      loading: false,
      attempted: false,
      gen: 0,
      nav: undefined,
      widthPx: undefined,
    };
    mem.set(state, m);
  }
  return m;
}

export function leaveActivity(state: object): void {
  const m = memOf(state);
  m.gen += 1;
  m.rows = undefined;
  m.selected = undefined;
  m.copied = false;
  m.clipFail = false;
  m.loading = false;
  m.attempted = false;
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | boolean | undefined> = {},
  children: Array<Node | string> = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === false) {
      continue;
    }
    if ((tag === "input" || tag === "textarea") && k === "value") {
      continue;
    }
    if (v === true) {
      node.setAttribute(k, "");
      if (k === "disabled" && "disabled" in node) {
        (node as HTMLButtonElement).disabled = true;
      }
      if (k === "readonly" && "readOnly" in node) {
        (node as HTMLInputElement).readOnly = true;
      }
    } else {
      node.setAttribute(k, v);
    }
  }
  if (tag === "input" || tag === "select" || tag === "textarea") {
    if (typeof attrs.value === "string") {
      (node as HTMLInputElement).value = attrs.value;
    }
  }
  for (const child of children) {
    if (child === "") {
      continue;
    }
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

function toHex(bytes: Uint8Array): string {
  const HEX = "0123456789abcdef";
  let out = "";
  for (const b of bytes) {
    out += HEX.charAt(b >> 4);
    out += HEX.charAt(b & 0x0f);
  }
  return out;
}

export function namesJson(names: readonly string[]): string {
  return JSON.stringify(names);
}

export function eventHash(
  prev: string,
  action: string,
  sessionId: string | undefined,
  names: readonly string[],
): string {
  const enc = new TextEncoder();
  const p = enc.encode(prev);
  const a = enc.encode(action);
  const s = sessionId === undefined ? new Uint8Array() : enc.encode(sessionId);
  const n = enc.encode(namesJson(names));
  const buf = new Uint8Array(p.length + a.length + s.length + n.length + 3);
  let i = 0;
  buf.set(p, i);
  i += p.length;
  buf[i++] = 0x1f;
  buf.set(a, i);
  i += a.length;
  buf[i++] = 0x1f;
  buf.set(s, i);
  i += s.length;
  buf[i++] = 0x1f;
  buf.set(n, i);
  return toHex(sha256(buf));
}

export function parseAudit(data: unknown): AuditEvent[] | undefined {
  if (typeof data !== "object" || data === null) {
    return undefined;
  }
  const events = (data as { events?: unknown }).events;
  if (!Array.isArray(events)) {
    return undefined;
  }
  return events.map(parseEvent);
}

function parseEvent(v: unknown): AuditEvent {
  if (typeof v !== "object" || v === null) {
    return {
      action: "",
      names: [],
      sessionId: undefined,
      prev: undefined,
      hash: undefined,
      ok: false,
    };
  }
  const rec = v as Record<string, unknown>;
  const action = rec["action"];
  const namesRaw = rec["names"];
  const namesOk =
    Array.isArray(namesRaw) && namesRaw.every((n) => typeof n === "string");
  const names = namesOk ? (namesRaw as string[]) : [];
  const sessionRaw = rec["session_id"];
  const sessionId = typeof sessionRaw === "string" ? sessionRaw : undefined;
  const prevRaw = rec["prev"];
  const hashRaw = rec["hash"];
  const ok = typeof action === "string" && namesOk;
  return {
    action: typeof action === "string" ? action : "",
    names,
    sessionId,
    prev: typeof prevRaw === "string" ? prevRaw : undefined,
    hash: typeof hashRaw === "string" ? hashRaw : undefined,
    ok,
  };
}

export function verifyChain(events: readonly AuditEvent[]): ChainRow[] {
  let prev = ZERO_HASH;
  let broken = false;
  const out: ChainRow[] = [];
  for (const ev of events) {
    const hash = eventHash(prev, ev.action, ev.sessionId, ev.names);
    let verified = !broken && ev.ok;
    let reason: string | undefined;
    if (!ev.ok) {
      verified = false;
      reason = MALFORMED_REASON;
      broken = true;
    } else if (broken) {
      verified = false;
      reason = CHAIN_LATER;
    } else {
      if (ev.prev !== undefined && ev.prev !== prev) {
        verified = false;
        reason = CHAIN_BREAK;
        broken = true;
      }
      if (ev.hash !== undefined && ev.hash !== hash) {
        verified = false;
        reason = CHAIN_BREAK;
        broken = true;
      }
    }
    out.push({
      action: ev.action,
      names: ev.names,
      sessionId: ev.sessionId,
      prev,
      hash,
      verified,
      reason,
    });
    prev = hash;
  }
  return out;
}

export function chainVerified(rows: readonly ChainRow[]): boolean {
  return rows.every((r) => r.verified);
}

function failSentence(status: number): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  return FAIL_SENTENCE;
}

function viewKind(q: {
  loading: boolean;
  error: string | undefined;
  rows: ChainRow[] | undefined;
}): "loading" | "empty" | "error" | "ready" {
  if (q.loading || q.rows === undefined) {
    return q.error !== undefined ? "error" : "loading";
  }
  if (q.error !== undefined) {
    return "error";
  }
  if (q.rows.length === 0) {
    return "empty";
  }
  return "ready";
}

function layoutFor(m: Mem): LayoutMode {
  const width =
    typeof m.widthPx === "number" && Number.isFinite(m.widthPx) && m.widthPx > 0
      ? m.widthPx
      : typeof globalThis.innerWidth === "number" &&
          Number.isFinite(globalThis.innerWidth) &&
          globalThis.innerWidth > 0
        ? globalThis.innerWidth
        : BREAKPOINT_PX;
  return layoutMode(width);
}

export function renderActivity(
  state: ActivityHost,
  root: HTMLElement,
  nav?: HTMLElement,
  widthPx?: number,
): void {
  const m = memOf(state);
  if (nav !== undefined) {
    m.nav = nav;
  }
  if (widthPx !== undefined) {
    m.widthPx = widthPx;
  }
  if (m.rows === undefined && !m.loading && !m.attempted) {
    void loadActivity(state, root);
    return;
  }
  paint(state, root);
}

async function loadActivity(state: ActivityHost, root: HTMLElement): Promise<void> {
  const m = memOf(state);
  if (m.loading || m.rows !== undefined) {
    return;
  }
  m.loading = true;
  m.attempted = true;
  const my = m.gen;
  state.pending.set(true);
  state.error.set(undefined);
  paint(state, root);
  try {
    const res = await req("GET", auditUrl());
    if (my !== m.gen || state.path.get() !== "/activity") {
      return;
    }
    if (res.status !== 200) {
      state.error.set(failSentence(res.status));
      return;
    }
    const parsed = parseAudit(res.data);
    if (parsed === undefined) {
      state.error.set(FAIL_SENTENCE);
      return;
    }
    m.rows = verifyChain(parsed);
    state.error.set(undefined);
  } catch {
    if (my !== m.gen || state.path.get() !== "/activity") {
      return;
    }
    state.error.set(FAIL_SENTENCE);
  } finally {
    if (my === m.gen) {
      m.loading = false;
      state.pending.set(false);
      if (state.path.get() === "/activity") {
        paint(state, root);
      }
    }
  }
}

function paint(state: ActivityHost, root: HTMLElement): void {
  const m = memOf(state);
  const hint = focusHints.get(state);
  focusHints.delete(state);
  const err = state.error.get();
  const kind = viewKind({ loading: m.loading, error: err, rows: m.rows });
  const layout = layoutFor(m);
  const rows = m.rows ?? [];
  const selected =
    m.selected !== undefined && m.selected >= 0 && m.selected < rows.length
      ? rows[m.selected]
      : undefined;
  const verified = kind === "ready" && chainVerified(rows);
  const children: Array<Node | string> = [];
  if (m.nav) {
    children.push(m.nav);
  }
  children.push(el("h1", {}, ["Activity"]));
  children.push(el("p", {}, [SUBTITLE]));
  if (kind === "loading") {
    children.push(el("p", { "data-state": "loading" }, [LOADING_SENTENCE]));
  } else if (kind === "error") {
    children.push(el("p", { class: "error", "data-reason": "" }, [err ?? FAIL_SENTENCE]));
  } else if (kind === "ready") {
    children.push(
      el(
        "p",
        {
          "data-chain-status": verified ? "verified" : "unverified",
          "data-verified": verified ? "1" : "0",
        },
        [verified ? VERIFIED : UNVERIFIED],
      ),
    );
  }

  const listBody: Array<Node | string> = [];
  if (kind === "empty") {
    listBody.push(el("p", {}, [EMPTY_TITLE]));
    listBody.push(el("p", {}, [EMPTY_BODY]));
  } else if (kind === "ready") {
    for (let i = 0; i < rows.length; i += 1) {
      const row = rows[i];
      if (row === undefined) {
        continue;
      }
      const current = m.selected === i;
      const names = row.names.length > 0 ? row.names.join(" ") : "";
      const btn = el(
        "button",
        {
          type: "button",
          class: "secondary",
          "data-seq": String(i),
          "data-verified": row.verified ? "1" : "0",
          "aria-current": current ? "true" : undefined,
        },
        [
          el("span", {}, [row.action === "" ? "event" : row.action]),
          names === "" ? "" : el("span", { class: "mono", "data-name": "" }, [names]),
          el("span", { "data-verified": row.verified ? "1" : "0" }, [
            row.verified ? VERIFIED : UNVERIFIED,
          ]),
        ],
      );
      const seq = i;
      btn.addEventListener("click", () => {
        m.selected = seq;
        m.copied = false;
        m.clipFail = false;
        focusHints.set(state, "copy");
        paint(state, root);
      });
      listBody.push(btn);
    }
  }

  const inspector = detailsEl(state, root, selected, kind, false);
  const workspace = el("div", { class: "workspace" }, [
    el("div", { class: "card", "data-pane": "list" }, [
      el("div", { class: "list", "data-list": "audit" }, listBody),
    ]),
    inspector,
  ]);
  children.push(workspace);
  if (selected !== undefined) {
    children.push(detailsEl(state, root, selected, kind, true));
  }
  if (m.clipFail) {
    children.push(el("p", { class: "error", "data-reason": "" }, [CLIP_FAIL_SENTENCE]));
    if (selected !== undefined) {
      children.push(
        el("input", {
          class: "mono",
          readonly: true,
          autocomplete: "off",
          "data-select-copy": "",
          "aria-label": "Event hash",
          value: selected.hash,
        }),
      );
    }
  }

  const page = el(
    "div",
    {
      class: "app",
      "data-page": "activity",
      "data-layout": layout,
      "data-state": kind,
    },
    children,
  );
  root.replaceChildren(page);
  let target: HTMLElement | null = null;
  if (hint === "hash" || m.clipFail) {
    const found = page.querySelector("[data-select-copy]");
    target = found instanceof HTMLElement ? found : null;
  } else if (hint === "copy") {
    const sel =
      layout === "list-only"
        ? '[data-pane="sheet"] [data-action="copy"]'
        : '[data-pane="inspector"] [data-action="copy"]';
    const found = page.querySelector(sel) ?? page.querySelector('[data-action="copy"]');
    target = found instanceof HTMLElement ? found : null;
  }
  if (target !== null && typeof target.focus === "function") {
    target.focus();
  }
}

function detailsEl(
  state: ActivityHost,
  root: HTMLElement,
  row: ChainRow | undefined,
  kind: "loading" | "empty" | "error" | "ready",
  sheet: boolean,
): HTMLElement {
  const m = memOf(state);
  const pending = state.pending.get() || m.loading;
  const disabled = row === undefined || pending || kind === "loading";
  const copyLabel = m.copied && row !== undefined ? "Copied" : "Copy";
  const copy = el(
    "button",
    {
      type: "button",
      "data-action": "copy",
      disabled: disabled ? true : undefined,
    },
    [copyLabel],
  );
  copy.addEventListener("click", () => {
    void onCopy(state, root);
  });
  const body: Array<Node | string> = [];
  if (row === undefined) {
    body.push(el("h2", {}, [kind === "empty" ? EMPTY_TITLE : SELECT_TITLE]));
    body.push(el("p", {}, [kind === "empty" ? EMPTY_BODY : SELECT_BODY]));
  } else {
    body.push(el("h2", { class: "mono", "data-field": "action" }, [row.action]));
    if (row.names.length === 0) {
      body.push(el("p", {}, ["No names."]));
    } else {
      for (const name of row.names) {
        body.push(el("p", { class: "mono", "data-name": "" }, [name]));
      }
    }
    if (row.sessionId === undefined) {
      body.push(el("p", {}, ["No session."]));
    } else {
      body.push(el("p", { class: "mono", "data-field": "session" }, [row.sessionId]));
    }
    body.push(el("p", {}, ["Hash"]));
    body.push(el("code", { class: "hex", "data-field": "hash" }, [row.hash]));
    body.push(el("p", {}, ["Previous"]));
    body.push(el("code", { class: "hex", "data-field": "prev" }, [row.prev]));
    body.push(
      el("p", { "data-verified": row.verified ? "1" : "0" }, [
        row.verified ? VERIFIED : UNVERIFIED,
      ]),
    );
    if (row.reason !== undefined) {
      body.push(el("p", { class: "error", "data-reason": "" }, [row.reason]));
    }
  }
  body.push(copy);
  if (disabled) {
    const reason = pending || kind === "loading" ? LOADING_SENTENCE : COPY_NEED_SENTENCE;
    body.push(el("p", { "data-reason": "" }, [reason]));
  }
  if (sheet) {
    const close = el("button", { type: "button", class: "secondary", "data-action": "close" }, [
      "Close",
    ]);
    close.addEventListener("click", () => {
      m.selected = undefined;
      m.copied = false;
      m.clipFail = false;
      paint(state, root);
    });
    body.push(close);
    return el("div", { class: "secd-overlay", "data-pane": "sheet", "data-sheet": "open" }, [
      el("div", { class: "secd-modal" }, [el("div", { class: "card" }, body)]),
    ]);
  }
  return el("div", { class: "card", "data-pane": "inspector" }, body);
}

async function onCopy(state: ActivityHost, root: HTMLElement): Promise<void> {
  const m = memOf(state);
  if (m.loading || state.pending.get()) {
    return;
  }
  const rows = m.rows;
  const row =
    m.selected !== undefined && rows !== undefined ? rows[m.selected] : undefined;
  if (row === undefined) {
    return;
  }
  const ok = await copyText(row.hash);
  m.copied = ok;
  m.clipFail = !ok;
  focusHints.set(state, ok ? "copy" : "hash");
  paint(state, root);
}
