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
import {
  LOADING_SENTENCE,
  PENDING_LABEL,
  POLL_MS,
  parsePending,
  pendingMeta,
  type PendingRequest,
} from "./devices.ts";

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
export const PICK_TITLE = "Which device is waiting?";
export const PICK_SUB =
  "Pick the request whose code matches the one secd printed on the machine in front of you.";
export const PICK_LABEL = "Review this request";
export const PICK_EMPTY = "No device is waiting for approval.";

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
  choices: PendingRequest[] | undefined;
  nowMs: number;
  tick: ReturnType<typeof setInterval> | undefined;
  poll: ReturnType<typeof setInterval> | undefined;
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
      choices: undefined,
      nowMs: Date.now(),
      tick: undefined,
      poll: undefined,
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
  if (state.userCode.get() === "") {
    // The CLI's link carries the code. Reached without one, this page lists
    // the requests waiting and loads the picked one into the card below.
    void loadChoices(state);
    ensurePoll(state, m);
  } else if (!m.resolved) {
    if (ephOk(state.eph.get())) {
      m.eph = state.eph.get();
    }
    void loadPending(state);
  }
  paint(state);
}

export function leaveApprove(state: object): void {
  const m = memOf(state);
  m.loadGen += 1;
  stopTick(m);
  stopPoll(m);
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
  m.choices = undefined;
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

function stopPoll(m: Mem): void {
  if (m.poll !== undefined) {
    clearInterval(m.poll);
    m.poll = undefined;
  }
}

function ensurePoll(state: AppState, m: Mem): void {
  if (m.poll !== undefined) {
    return;
  }
  m.poll = setInterval(() => {
    void loadChoices(state, true);
  }, POLL_MS);
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

/** Every request waiting, for the pick list this page shows without a code. */
async function loadChoices(state: AppState, force = false): Promise<void> {
  const m = memOf(state);
  if (m.loading || (!force && m.choices !== undefined)) {
    return;
  }
  m.loading = true;
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
    m.choices = parsePending(res.data);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.loading = false;
      paint(state);
    }
  }
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

/** The pick carries the whole row, so the card below needs no second GET. */
function onPick(state: AppState, row: PendingRequest): void {
  const m = memOf(state);
  stopPoll(m);
  state.userCode.set(row.user_code);
  state.eph.set(row.eph_pub);
  m.eph = row.eph_pub;
  m.hostname = row.hostname;
  m.created = row.created;
  m.expiresAt = Date.now() + row.expires_in * 1000;
  m.choices = undefined;
  m.error = undefined;
  m.resolved = true;
  paint(state);
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

function focusKey(root: HTMLElement): { action: string; row: string } | undefined {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement) || !root.contains(active)) {
    return undefined;
  }
  const action = active.getAttribute("data-action");
  if (action === null) {
    return undefined;
  }
  const row = active.closest("[data-code]")?.getAttribute("data-code") ?? "";
  return { action, row };
}

function restoreFocus(root: HTMLElement, key: { action: string; row: string } | undefined): void {
  if (key === undefined) {
    return;
  }
  const scope = key.row === "" ? root : (root.querySelector(`[data-code="${key.row}"]`) ?? root);
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
  const prev = focusKey(root);
  const picking = m.result === undefined && state.userCode.get() === "";
  if (m.result !== undefined || picking) {
    stopTick(m);
  } else {
    ensureTick(state, m);
  }
  const host = globalThis.location?.host ?? "";
  const wrapChildren: HTMLElement[] = [];
  if (m.result !== undefined) {
    wrapChildren.push(resultCard(state, m.result));
  } else if (picking) {
    wrapChildren.push(pickCard(state, m));
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

function pickCard(state: AppState, m: Mem): HTMLElement {
  const body: HTMLElement[] = [];
  if (m.error !== undefined) {
    body.push(
      el("div", { class: "alert alert-danger", role: "alert", "data-error": "" }, [m.error]),
    );
  }
  const rows = m.choices;
  if (rows === undefined) {
    if (m.error === undefined) {
      body.push(el("div", { class: "empty", "data-state": "loading" }, [LOADING_SENTENCE]));
    }
  } else if (rows.length === 0) {
    body.push(el("div", { class: "empty", "data-state": "empty" }, [PICK_EMPTY]));
  } else {
    const now = Date.now();
    for (const row of rows) {
      body.push(choiceCard(state, row, now));
    }
  }
  return el("div", { class: "approve-card" }, [
    el("div", { class: "approve-head" }, [
      el("div", { class: "gate-title" }, [PICK_TITLE]),
      el("p", {}, [PICK_SUB]),
    ]),
    el("div", { class: "approve-actions" }, body),
  ]);
}

function choiceCard(state: AppState, row: PendingRequest, nowMs: number): HTMLElement {
  const pick = el("button", { type: "button", class: "btn btn-primary", "data-action": "pick" }, [
    PICK_LABEL,
  ]);
  pick.addEventListener("click", () => {
    onPick(state, row);
  });
  return el("div", { class: "pending-card", "data-code": row.user_code }, [
    el("div", {}, [
      el("div", { class: "pending-label" }, [PENDING_LABEL]),
      el("div", { class: "pending-code" }, [row.user_code]),
      el("div", { class: "pending-meta" }, [pendingMeta(row, nowMs)]),
    ]),
    el("div", { class: "pending-actions" }, [pick]),
  ]);
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
