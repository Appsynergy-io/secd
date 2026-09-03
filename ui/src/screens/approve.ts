/** Device approval: seal this tab's DEK to the CLI's ephemeral X25519 key. */

import {
  FAIL_SENTENCE,
  NO_DEK_SENTENCE,
  NO_EPH_SENTENCE,
  RATE_SENTENCE,
  deviceApproveUrl,
  deviceDenyUrl,
  devicePendingUrl,
  errorMessage,
  req,
} from "../lib/api.ts";
import { fromHex, zeroizeBytes } from "../lib/crypto.ts";
import * as keyholder from "../lib/keyholder.ts";
import { el } from "../lib/dom.ts";
import { currentLogoutGen } from "../lib/gen.ts";
import type { AppState, Host, SessionInfo } from "../lib/host.ts";
import { ago, countdown, keyFingerprint } from "../lib/time.ts";
import { parsePending } from "./devices.ts";

export const TICK_MS = 1_000;
export const NO_SESSION_SENTENCE = "Sign in from this browser first.";
export const HEAD_TITLE = "Does this match your terminal?";
export const HEAD_SUB =
  "Approve only if these characters are the ones secd printed on the machine in front of you.";
export const APPROVE_NOTE =
  "Approving seals this browser's vault key to the device's ephemeral X25519 key, so the key travels to that machine and nowhere else. The server relays ciphertext it cannot open.";
export const APPROVE_LABEL = "Approve this device";
export const DENY_LABEL = "This wasn't me — deny";
export const BACK_LABEL = "Back to console";
export const FOOT_ROUTE = "POST /api/v1/device/approve";
export const APPROVED_TITLE = "Device approved";
export const APPROVED_BODY =
  "The vault key was sealed to that device's ephemeral key. Your terminal has already picked it up — you can close this tab.";
export const DENIED_TITLE = "Request denied";
export const DENIED_BODY =
  "Nothing was sent. The pending request was dropped; the CLI will report that the approval was refused.";
export const MISSING_TITLE = "Request not found";
export const MISSING_BODY = "The code has expired or was already handled.";

export type ApproveResult = "approved" | "denied" | "missing";

export type Fact = { label: string; value: string };

type Mem = {
  result: ApproveResult | undefined;
  error: string | undefined;
  busy: boolean;
  loading: boolean;
  resolved: boolean;
  hostname: string;
  created: string;
  expiresAt: number | undefined;
  eph: string;
  nowMs: number;
  tick: ReturnType<typeof setInterval> | undefined;
  unwatch: (() => void) | undefined;
  loadGen: number;
  root: HTMLElement | undefined;
  host: Host | undefined;
};

const mem = new WeakMap<object, Mem>();

function memOf(state: object): Mem {
  let m = mem.get(state);
  if (m === undefined) {
    m = {
      result: undefined,
      error: undefined,
      busy: false,
      loading: false,
      resolved: false,
      hostname: "",
      created: "",
      expiresAt: undefined,
      eph: "",
      nowMs: Date.now(),
      tick: undefined,
      unwatch: undefined,
      loadGen: 0,
      root: undefined,
      host: undefined,
    };
    mem.set(state, m);
  }
  return m;
}

/** 32-byte X25519 public key as 64 hex chars. */
export function ephOk(eph: string): boolean {
  if (eph.length !== 64) {
    return false;
  }
  try {
    const bytes = fromHex(eph);
    const ok = bytes.length === 32;
    zeroizeBytes(bytes);
    return ok;
  } catch {
    return false;
  }
}

export function deviceDisabledReason(q: {
  userCode: string;
  eph: string;
  session: SessionInfo | undefined;
  hasDek: boolean;
}): string | undefined {
  if (q.userCode === "" || !ephOk(q.eph)) {
    return NO_EPH_SENTENCE;
  }
  if (q.session === undefined) {
    return NO_SESSION_SENTENCE;
  }
  if (!q.hasDek) {
    return NO_DEK_SENTENCE;
  }
  return undefined;
}

export function splitCode(code: string): string[] {
  return [...code];
}

export function approveFacts(q: {
  hostname: string;
  created: string;
  expiresIn: number | undefined;
  eph: string;
  email: string;
  nowMs?: number;
}): Fact[] {
  const now = q.nowMs ?? Date.now();
  return [
    { label: "Requesting host", value: q.hostname },
    { label: "Requested", value: q.created === "" ? "" : ago(q.created, now) },
    {
      label: "Expires",
      value: q.expiresIn === undefined ? "" : `in ${countdown(q.expiresIn)}`,
    },
    { label: "Ephemeral key", value: q.eph === "" ? "" : keyFingerprint(q.eph) },
    { label: "Signed in as", value: q.email },
  ];
}

export function failSentence(status: number, data?: unknown): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  return errorMessage(data) ?? FAIL_SENTENCE;
}

export function resultCopy(result: ApproveResult): { title: string; body: string } {
  if (result === "approved") {
    return { title: APPROVED_TITLE, body: APPROVED_BODY };
  }
  if (result === "denied") {
    return { title: DENIED_TITLE, body: DENIED_BODY };
  }
  return { title: MISSING_TITLE, body: MISSING_BODY };
}

export function renderApprove(state: AppState, root: HTMLElement, host: Host): void {
  const m = memOf(state);
  m.root = root;
  m.host = host;
  watchDek(state, m);
  if (!m.resolved) {
    if (ephOk(state.eph.get())) {
      m.eph = state.eph.get();
    }
    if (state.userCode.get() === "" && m.eph === "") {
      m.result = "missing";
      m.resolved = true;
    } else {
      void loadPending(state);
    }
  }
  paint(state);
}

export function leaveApprove(state: object): void {
  const m = memOf(state);
  m.loadGen += 1;
  stopTick(m);
  m.unwatch?.();
  m.unwatch = undefined;
  m.result = undefined;
  m.error = undefined;
  m.busy = false;
  m.loading = false;
  m.resolved = false;
  m.hostname = "";
  m.created = "";
  m.expiresAt = undefined;
  m.eph = "";
  m.root = undefined;
  m.host = undefined;
}

function watchDek(state: AppState, m: Mem): void {
  if (m.unwatch !== undefined) {
    return;
  }
  m.unwatch = keyholder.subscribe(() => {
    if (m.root !== undefined) {
      paint(state);
    }
  });
}

function stopTick(m: Mem): void {
  if (m.tick !== undefined) {
    clearInterval(m.tick);
    m.tick = undefined;
  }
}

function ensureTick(state: AppState, m: Mem): void {
  if (m.tick !== undefined || m.result !== undefined) {
    return;
  }
  m.tick = setInterval(() => {
    m.nowMs = Date.now();
    paint(state);
  }, TICK_MS);
}

function guard(m: Mem): () => boolean {
  const gen = currentLogoutGen();
  const loadGen = m.loadGen;
  return () => gen !== currentLogoutGen() || loadGen !== m.loadGen;
}

function isAuthFail(status: number): boolean {
  return status === 401 || status === 403;
}

function remainingSecs(expiresAt: number, nowMs: number): number {
  return Math.max(0, Math.floor((expiresAt - nowMs) / 1000));
}

async function loadPending(state: AppState): Promise<void> {
  const m = memOf(state);
  if (m.resolved || m.loading) {
    return;
  }
  const code = state.userCode.get();
  if (code === "") {
    m.resolved = true;
    paint(state);
    return;
  }
  m.loading = true;
  paint(state);
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
    const match = parsePending(res.data).find((p) => p.user_code === code);
    if (match !== undefined) {
      m.hostname = match.hostname;
      m.created = match.created;
      m.expiresAt = Date.now() + match.expires_in * 1000;
      if (m.eph === "") {
        m.eph = match.eph_pub;
      }
    } else if (m.eph === "") {
      m.result = "missing";
    }
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.loading = false;
      m.resolved = true;
      paint(state);
    }
  }
}

async function onApprove(state: AppState): Promise<void> {
  const m = memOf(state);
  if (m.busy || m.result !== undefined) {
    return;
  }
  const reason = deviceDisabledReason({
    userCode: state.userCode.get(),
    eph: m.eph,
    session: state.session.get(),
    hasDek: keyholder.isUnlocked(),
  });
  if (reason !== undefined) {
    m.error = reason;
    paint(state);
    return;
  }
  if (!keyholder.isUnlocked()) {
    m.error = NO_DEK_SENTENCE;
    paint(state);
    return;
  }
  let ephBytes: Uint8Array;
  try {
    ephBytes = fromHex(m.eph);
  } catch {
    m.error = NO_EPH_SENTENCE;
    paint(state);
    return;
  }
  if (ephBytes.length !== 32) {
    zeroizeBytes(ephBytes);
    m.error = NO_EPH_SENTENCE;
    paint(state);
    return;
  }
  m.busy = true;
  m.error = undefined;
  paint(state);
  const stale = guard(m);
  try {
    const sealed = await keyholder.sealToEph(m.eph);
    if (sealed === undefined) {
      if (!stale()) {
        m.error = FAIL_SENTENCE;
      }
      return;
    }
    zeroizeBytes(ephBytes);
    const res = await req("POST", deviceApproveUrl(), {
      user_code: state.userCode.get(),
      sealed_dek: sealed,
    });
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
    m.result = "approved";
    stopTick(m);
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

async function onDeny(state: AppState): Promise<void> {
  const m = memOf(state);
  if (m.busy || m.result !== undefined) {
    return;
  }
  const code = state.userCode.get();
  if (code === "") {
    m.result = "missing";
    paint(state);
    return;
  }
  m.busy = true;
  m.error = undefined;
  paint(state);
  const stale = guard(m);
  try {
    const res = await req("POST", deviceDenyUrl(), { user_code: code });
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
    m.result = "denied";
    stopTick(m);
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

function backToConsole(state: AppState): void {
  const m = memOf(state);
  state.userCode.set("");
  state.eph.set("");
  m.host?.navigate("/devices");
}

function focusAction(root: HTMLElement): string | undefined {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement) || !root.contains(active)) {
    return undefined;
  }
  return active.getAttribute("data-action") ?? undefined;
}

function restoreFocus(root: HTMLElement, action: string | undefined): void {
  if (action === undefined) {
    return;
  }
  const found = root.querySelector(`[data-action="${action}"]`);
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
  const prev = focusAction(root);
  if (m.result !== undefined) {
    stopTick(m);
  } else {
    ensureTick(state, m);
  }
  const host = globalThis.location?.host ?? "";
  const wrapChildren: HTMLElement[] = [];
  if (m.result !== undefined) {
    wrapChildren.push(resultCard(state, m.result));
  } else {
    wrapChildren.push(openCard(state, m));
  }
  wrapChildren.push(footEl(state));
  const page = el("div", { class: "approve", "data-page": "approve" }, [
    el("div", { class: "approve-top" }, [
      el("div", { class: "brand-mark brand-mark-sm", "aria-hidden": "true" }, ["s"]),
      el("div", {}, ["Device approval"]),
      el("div", { class: "chip spacer", "data-host": "" }, [
        el("span", { class: "dot", "aria-hidden": "true" }),
        host,
      ]),
    ]),
    el("div", { class: "approve-body" }, [
      el("div", { class: "approve-wrap" }, wrapChildren),
    ]),
  ]);
  root.replaceChildren(page);
  restoreFocus(root, prev);
}

function openCard(state: AppState, m: Mem): HTMLElement {
  const code = state.userCode.get();
  const expiresIn =
    m.expiresAt === undefined ? undefined : remainingSecs(m.expiresAt, m.nowMs);
  const facts = approveFacts({
    hostname: m.hostname,
    created: m.created,
    expiresIn,
    eph: m.eph,
    email: state.session.get()?.email ?? "",
    nowMs: m.nowMs,
  });
  const reason = m.loading
    ? undefined
    : deviceDisabledReason({
        userCode: code,
        eph: m.eph,
        session: state.session.get(),
        hasDek: keyholder.isUnlocked(),
      });
  const err = m.error ?? reason;
  const approveOff = m.busy || reason !== undefined;
  const approve = el(
    "button",
    {
      type: "button",
      class: "btn btn-primary btn-xl btn-block",
      "data-action": "approve",
      disabled: approveOff ? true : undefined,
    },
    [APPROVE_LABEL],
  );
  approve.disabled = approveOff;
  approve.addEventListener("click", () => {
    void onApprove(state);
  });
  const deny = el(
    "button",
    {
      type: "button",
      class: "btn btn-lg btn-block",
      "data-action": "deny",
      disabled: m.busy ? true : undefined,
    },
    [DENY_LABEL],
  );
  deny.disabled = m.busy;
  deny.addEventListener("click", () => {
    void onDeny(state);
  });
  const actions: HTMLElement[] = [];
  if (err !== undefined) {
    actions.push(el("div", { class: "alert alert-danger", role: "alert", "data-error": "" }, [err]));
  }
  actions.push(approve, deny);
  const sub: Array<Node | string> = [
    "Approve only if these characters are the ones ",
    el("span", { class: "mono" }, ["secd"]),
    " printed on the machine in front of you.",
  ];
  return el("div", { class: "approve-card" }, [
    el("div", { class: "approve-head" }, [el("div", { class: "gate-title" }, [HEAD_TITLE]), el("p", {}, sub)]),
    el("div", { class: "code-boxes" }, splitCode(code).map(codeCell)),
    el(
      "div",
      { class: "facts" },
      facts.map((f) =>
        el("div", { class: "fact" }, [
          el("div", { class: "fact-label" }, [f.label]),
          el("div", { class: "fact-value" }, [f.value]),
        ]),
      ),
    ),
    el("div", { class: "note" }, [APPROVE_NOTE]),
    el("div", { class: "approve-actions" }, actions),
  ]);
}

function codeCell(ch: string): HTMLElement {
  if (ch === "-") {
    return el("div", { class: "code-gap" }, []);
  }
  return el("div", { class: "code-box" }, [ch]);
}

function resultCard(state: AppState, result: ApproveResult): HTMLElement {
  const copy = resultCopy(result);
  const back = el("button", { type: "button", class: "btn", "data-action": "back" }, [BACK_LABEL]);
  back.addEventListener("click", () => {
    backToConsole(state);
  });
  return el("div", { class: "approve-result" }, [
    el("div", {
      class: "result-dot",
      "data-result": result === "missing" ? undefined : result,
    }),
    el("div", { class: "result-title" }, [copy.title]),
    el("div", { class: "result-body" }, [copy.body]),
    back,
  ]);
}

function footEl(state: AppState): HTMLElement {
  const consoleLink = el("a", { href: "/devices", "data-action": "console" }, ["console"]);
  consoleLink.addEventListener("click", (ev) => {
    ev.preventDefault();
    backToConsole(state);
  });
  return el("div", { class: "approve-foot" }, [
    el("span", {}, [FOOT_ROUTE]),
    el("span", { class: "spacer" }, [consoleLink]),
  ]);
}
