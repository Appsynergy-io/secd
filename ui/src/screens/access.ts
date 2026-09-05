/** Access: passkeys, browser sessions, the password factor, and the key this browser holds. */

import {
  FAIL_SENTENCE,
  LAST_FACTOR_SENTENCE,
  NO_DEK_SENTENCE,
  RATE_SENTENCE,
  errorMessage,
  passkeyDeletePath,
  passkeyRegisterFinishUrl,
  passkeyRegisterStartUrl,
  passkeysUrl,
  passwordRegisterUrl,
  removePasskeyEnabled,
  req,
  sessionRevokePath,
  sessionsUrl,
} from "../lib/api.ts";
import { passwordOk, toHex, zeroizeBytes } from "../lib/crypto.ts";
import * as keyholder from "../lib/keyholder.ts";
import { asInput, el } from "../lib/dom.ts";
import { currentLogoutGen } from "../lib/gen.ts";
import type { AppState, Host } from "../lib/host.ts";
import { ago, day, dayLabel, remainingLabel } from "../lib/time.ts";
import {
  coercePublicKey,
  createPasskey,
  prfBytes,
  serializeCredential,
} from "../lib/webauthn.ts";

export const LOADING_PASSKEYS = "Loading passkeys.";
export const EMPTY_PASSKEYS = "No passkeys.";
export const LOADING_SESSIONS = "Loading sessions.";
export const EMPTY_SESSIONS = "No sessions.";
export const PASSKEY_NOTE_OK =
  "Removing a passkey drops its wrap of the vault key. Other factors keep working.";
export const PASSKEY_NOTE_LAST =
  "At least one factor must remain — add a second passkey or a password first.";
export const PASSWORD_SUB =
  "A second factor for the vault key. Set or change it here.";
export const PASSWORD_LENGTH_SENTENCE = "Use 12 to 256 characters.";
export const KEY_TITLE = "Vault key in this browser";
export const KEY_BODY =
  "Held in memory only, dropped after 12 hours or on sign-out. Shared across this origin's tabs. Nothing on disk holds it, and the server has never seen it.";
export const NO_KEY_LABEL = "no vault key in this browser";
export const PASSKEY_ADDED_TOAST = "Passkey added";
export const PASSWORD_SET_TOAST = "Password set";
export const SIGNED_OUT_TOAST = "This browser signed out";
export const KEY_TICK_MS = 30_000;

export type PasskeyRow = {
  id: string;
  created: string;
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
  passkeys: PasskeyRow[] | undefined;
  sessions: SessionRow[] | undefined;
  error: string | undefined;
  pending: boolean;
  loadingPasskeys: boolean;
  loadingSessions: boolean;
  loadGen: number;
  keyTimer: ReturnType<typeof setInterval> | undefined;
  root: HTMLElement | undefined;
  host: Host | undefined;
};

const mem = new WeakMap<object, Mem>();

function memOf(state: object): Mem {
  let m = mem.get(state);
  if (m === undefined) {
    m = {
      passkeys: undefined,
      sessions: undefined,
      error: undefined,
      pending: false,
      loadingPasskeys: false,
      loadingSessions: false,
      loadGen: 0,
      keyTimer: undefined,
      root: undefined,
      host: undefined,
    };
    mem.set(state, m);
  }
  return m;
}

/* Pure helpers */

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return typeof v === "object" && v !== null ? (v as Record<string, unknown>) : undefined;
}

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

export function parsePasskeys(v: unknown): PasskeyRow[] {
  const rows = asRecord(v)?.["passkeys"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: PasskeyRow[] = [];
  for (const row of rows) {
    const r = asRecord(row);
    const id = r?.["id"];
    if (r === undefined || typeof id !== "string" || id === "") {
      continue;
    }
    out.push({ id, created: str(r["created"]) });
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
    const id = r?.["id"];
    if (r === undefined || typeof id !== "string" || id === "") {
      continue;
    }
    out.push({
      id,
      kind: str(r["kind"]),
      label: str(r["label"]),
      created: str(r["created"]),
      last_seen: str(r["last_seen"]),
      current: r["current"] === true,
    });
  }
  return out;
}

/** The rows the Browser sessions card shows: console sessions only; devices live on Devices. */
export function consoleRows(rows: readonly SessionRow[]): SessionRow[] {
  return rows.filter((s) => s.kind === "console");
}

export function passkeyNote(count: number, hasPassword: boolean): string {
  return removePasskeyEnabled(count, hasPassword) ? PASSKEY_NOTE_OK : PASSKEY_NOTE_LAST;
}

export function passwordAction(hasPassword: boolean): string {
  return hasPassword ? "Change password" : "Set password";
}

export function passkeyAdded(row: PasskeyRow): string {
  return `added ${day(row.created)}`;
}

export function sessionLabels(
  row: SessionRow,
  nowMs = Date.now(),
): { signedIn: string; lastSeen: string } {
  return {
    signedIn: `signed in ${dayLabel(row.created, nowMs)}`,
    lastSeen: `last seen ${ago(row.last_seen, nowMs)}`,
  };
}

export function keyLabel(remainingMs: number): string {
  return remainingMs > 0 ? `vault key · ${remainingLabel(remainingMs)}` : NO_KEY_LABEL;
}

export function revokedToast(row: SessionRow): string {
  return row.current ? SIGNED_OUT_TOAST : `Revoked ${row.label === "" ? row.id : row.label}`;
}

export function failSentence(status: number, data?: unknown): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  if (errorMessage(data) === "last factor") {
    return LAST_FACTOR_SENTENCE;
  }
  return FAIL_SENTENCE;
}

/* Screen */

export function renderAccess(state: AppState, root: HTMLElement, host: Host): void {
  const m = memOf(state);
  m.root = root;
  m.host = host;
  paint(state);
  armKeyTimer(m);
  void loadPasskeys(state);
  void loadSessions(state);
}

export function leaveAccess(state: object): void {
  const m = memOf(state);
  m.loadGen += 1;
  if (m.keyTimer !== undefined) {
    clearInterval(m.keyTimer);
    m.keyTimer = undefined;
  }
  m.passkeys = undefined;
  m.sessions = undefined;
  m.error = undefined;
  m.pending = false;
  m.loadingPasskeys = false;
  m.loadingSessions = false;
  m.root = undefined;
  m.host = undefined;
}

function armKeyTimer(m: Mem): void {
  if (m.keyTimer !== undefined) {
    clearInterval(m.keyTimer);
  }
  m.keyTimer = setInterval(() => {
    const node = m.root?.querySelector("[data-key-label]");
    if (node) {
      node.textContent = keyLabel(keyholder.remainingMs());
    }
  }, KEY_TICK_MS);
}

type Guard = () => boolean;

/** Captures the sign-out and leave generations; `stale()` is true once either moved. */
function guard(m: Mem): Guard {
  const gen = currentLogoutGen();
  const loadGen = m.loadGen;
  return () => gen !== currentLogoutGen() || loadGen !== m.loadGen;
}

function isAuthFail(status: number): boolean {
  return status === 401 || status === 403;
}

async function loadPasskeys(state: AppState): Promise<void> {
  const m = memOf(state);
  if (m.passkeys !== undefined || m.loadingPasskeys || state.session.get() === undefined) {
    return;
  }
  m.loadingPasskeys = true;
  paint(state);
  const stale = guard(m);
  try {
    const res = await req("GET", passkeysUrl());
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
    m.passkeys = parsePasskeys(res.data);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.loadingPasskeys = false;
      paint(state);
    }
  }
}

async function loadSessions(state: AppState): Promise<void> {
  const m = memOf(state);
  if (m.sessions !== undefined || m.loadingSessions || state.session.get() === undefined) {
    return;
  }
  m.loadingSessions = true;
  paint(state);
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
    m.sessions = parseSessions(res.data);
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

/** A factor changed: the session's has_* flags and the passkey list are reloaded together. */
async function refreshFactors(state: AppState): Promise<void> {
  const m = memOf(state);
  m.passkeys = undefined;
  await m.host?.loadSession();
  await loadPasskeys(state);
}

async function onRemove(state: AppState, id: string): Promise<void> {
  const m = memOf(state);
  const session = state.session.get();
  if (
    m.pending ||
    m.passkeys === undefined ||
    session === undefined ||
    !removePasskeyEnabled(m.passkeys.length, session.has_password)
  ) {
    return;
  }
  const stale = guard(m);
  m.pending = true;
  m.error = undefined;
  paint(state);
  try {
    const res = await req("DELETE", passkeyDeletePath(id));
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
    m.host?.flash(`Passkey ${id} removed`);
    m.pending = false;
    await refreshFactors(state);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.pending = false;
      paint(state);
    }
  }
}

async function onRevoke(state: AppState, row: SessionRow): Promise<void> {
  const m = memOf(state);
  if (m.pending) {
    return;
  }
  const stale = guard(m);
  m.pending = true;
  m.error = undefined;
  paint(state);
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
    m.host?.flash(revokedToast(row));
    if (row.current) {
      void m.host?.signOut();
      return;
    }
    m.sessions = undefined;
    m.pending = false;
    await loadSessions(state);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.pending = false;
      paint(state);
    }
  }
}

function handleOf(data: unknown): string {
  return str(asRecord(data)?.["handle"]);
}

async function onAdd(state: AppState): Promise<void> {
  const m = memOf(state);
  const email = state.session.get()?.email ?? "";
  if (m.pending || email === "") {
    return;
  }
  if (!keyholder.isUnlocked()) {
    m.error = NO_DEK_SENTENCE;
    paint(state);
    return;
  }
  const stale = guard(m);
  m.pending = true;
  m.error = undefined;
  paint(state);
  let prf: Uint8Array | undefined;
  try {
    const start = await req("POST", passkeyRegisterStartUrl(), { email });
    if (stale()) {
      return;
    }
    if (isAuthFail(start.status)) {
      void m.host?.signOut();
      return;
    }
    if (start.status !== 200) {
      m.error = failSentence(start.status, start.data);
      return;
    }
    const pk = coercePublicKey(start.data) as unknown as PublicKeyCredentialCreationOptions;
    const cred = await createPasskey(pk);
    prf = prfBytes(cred);
    if (stale()) {
      return;
    }
    if (prf === undefined) {
      m.error = FAIL_SENTENCE;
      return;
    }
    const wrap = await keyholder.wrapPasskey(toHex(prf), toHex(new Uint8Array(cred.rawId)));
    if (wrap === undefined) {
      m.error = NO_DEK_SENTENCE;
      return;
    }
    const finish = await req("POST", passkeyRegisterFinishUrl(), {
      handle: handleOf(start.data),
      credential: serializeCredential(cred),
      wrap,
    });
    if (stale()) {
      return;
    }
    if (isAuthFail(finish.status)) {
      void m.host?.signOut();
      return;
    }
    if (finish.status !== 200) {
      m.error = failSentence(finish.status, finish.data);
      return;
    }
    m.host?.flash(PASSKEY_ADDED_TOAST);
    m.pending = false;
    await refreshFactors(state);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (prf !== undefined) {
      zeroizeBytes(prf);
    }
    if (!stale()) {
      m.pending = false;
      paint(state);
    }
  }
}

async function onSetPassword(state: AppState, password: string): Promise<void> {
  const m = memOf(state);
  const email = state.session.get()?.email ?? "";
  if (m.pending || email === "") {
    return;
  }
  if (!passwordOk(password)) {
    m.error = PASSWORD_LENGTH_SENTENCE;
    paint(state);
    return;
  }
  if (!keyholder.isUnlocked()) {
    m.error = NO_DEK_SENTENCE;
    paint(state);
    return;
  }
  const stale = guard(m);
  m.pending = true;
  m.error = undefined;
  paint(state);
  try {
    const wrap = await keyholder.wrapPassword(password);
    if (wrap === undefined) {
      m.error = NO_DEK_SENTENCE;
      return;
    }
    const res = await req("POST", passwordRegisterUrl(), { email, password, wrap });
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
    m.host?.flash(PASSWORD_SET_TOAST);
    m.pending = false;
    await refreshFactors(state);
  } catch {
    if (!stale()) {
      m.error = FAIL_SENTENCE;
    }
  } finally {
    if (!stale()) {
      m.pending = false;
      paint(state);
    }
  }
}

/* Paint */

function focusKey(root: HTMLElement): { action: string; row: string } | undefined {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement) || !root.contains(active)) {
    return undefined;
  }
  const action = active.getAttribute("data-action");
  if (action === null) {
    return undefined;
  }
  return { action, row: active.closest("[data-row-id]")?.getAttribute("data-row-id") ?? "" };
}

function restoreFocus(root: HTMLElement, key: { action: string; row: string } | undefined): void {
  if (key === undefined) {
    return;
  }
  const scope = key.row === "" ? root : (root.querySelector(`[data-row-id="${key.row}"]`) ?? root);
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
  const session = state.session.get();
  const hasPassword = session?.has_password === true;
  const children: Array<Node | string> = [];
  if (m.error !== undefined) {
    children.push(
      el("div", { class: "alert alert-danger", role: "alert", "data-error": "" }, [m.error]),
    );
  }
  children.push(
    passkeysCard(state, m, hasPassword),
    sessionsCard(state, m),
    passwordCard(state, m, hasPassword),
    keyCard(),
  );
  const page = el("div", { class: "page", "data-width": "820" }, [
    el("div", { class: "stack" }, children),
  ]);
  root.replaceChildren(page);
  restoreFocus(root, key);
}

function passkeysCard(state: AppState, m: Mem, hasPassword: boolean): HTMLElement {
  const add = el(
    "button",
    {
      type: "button",
      class: "btn btn-sm spacer",
      "data-action": "add-passkey",
      disabled: m.pending ? true : undefined,
    },
    ["Add passkey"],
  );
  add.addEventListener("click", () => {
    void onAdd(state);
  });
  const body: Array<Node | string> = [
    el("div", { class: "card-head" }, [el("div", { class: "card-title" }, ["Passkeys"]), add]),
  ];
  const rows = m.passkeys;
  if (rows === undefined) {
    if (m.loadingPasskeys) {
      body.push(el("div", { class: "empty", "data-state": "loading" }, [LOADING_PASSKEYS]));
    }
  } else if (rows.length === 0) {
    body.push(el("div", { class: "empty", "data-state": "empty" }, [EMPTY_PASSKEYS]));
  } else {
    const allowed = removePasskeyEnabled(rows.length, hasPassword);
    for (const row of rows) {
      body.push(passkeyRowEl(state, m, row, allowed));
    }
  }
  if (rows !== undefined) {
    body.push(
      el("div", { class: "card-note", "data-note": "" }, [passkeyNote(rows.length, hasPassword)]),
    );
  }
  return el("section", { class: "card", "data-card": "passkeys", "aria-label": "Passkeys" }, body);
}

function passkeyRowEl(state: AppState, m: Mem, row: PasskeyRow, allowed: boolean): HTMLElement {
  const remove = el(
    "button",
    {
      type: "button",
      class: allowed ? "btn btn-sm btn-danger" : "btn btn-sm",
      "data-action": "remove",
      disabled: allowed && !m.pending ? undefined : true,
    },
    ["Remove"],
  );
  remove.addEventListener("click", () => {
    void onRemove(state, row.id);
  });
  return el("div", { class: "grid cols-passkeys", "data-row-id": row.id, "data-passkey-id": row.id }, [
    el("div", { class: "cell-mono truncate" }, [row.id]),
    el("div", { class: "cell-muted" }, [passkeyAdded(row)]),
    remove,
  ]);
}

function sessionsCard(state: AppState, m: Mem): HTMLElement {
  const body: Array<Node | string> = [
    el("div", { class: "card-head" }, [el("div", { class: "card-title" }, ["Browser sessions"])]),
  ];
  const rows = m.sessions === undefined ? undefined : consoleRows(m.sessions);
  if (rows === undefined) {
    if (m.loadingSessions) {
      body.push(el("div", { class: "empty", "data-state": "loading" }, [LOADING_SESSIONS]));
    }
  } else if (rows.length === 0) {
    body.push(el("div", { class: "empty", "data-state": "empty" }, [EMPTY_SESSIONS]));
  } else {
    const now = Date.now();
    for (const row of rows) {
      body.push(sessionRowEl(state, m, row, now));
    }
  }
  return el(
    "section",
    { class: "card", "data-card": "sessions", "aria-label": "Browser sessions" },
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
      disabled: m.pending ? true : undefined,
    },
    ["Revoke"],
  );
  revoke.addEventListener("click", () => {
    void onRevoke(state, row);
  });
  const labels = sessionLabels(row, nowMs);
  const who: Array<Node | string> = [
    el("span", { class: "truncate" }, [row.label === "" ? row.id : row.label]),
  ];
  if (row.current) {
    who.push(el("span", { class: "badge badge-ok", "data-current": "" }, ["current"]));
  }
  return el(
    "div",
    {
      class: "grid cols-sessions",
      "data-row-id": row.id,
      "data-session-id": row.id,
      "data-current": row.current ? "true" : undefined,
    },
    [
      el("div", { class: "hrow truncate" }, who),
      el("div", { class: "cell-muted" }, [labels.signedIn]),
      el("div", { class: "cell-muted" }, [labels.lastSeen]),
      revoke,
    ],
  );
}

function passwordCard(state: AppState, m: Mem, hasPassword: boolean): HTMLElement {
  const input = el("input", {
    class: "input input-sm",
    type: "password",
    autocomplete: "new-password",
    "aria-label": "Password",
    "data-field": "password",
    disabled: m.pending ? true : undefined,
  });
  const submit = el(
    "button",
    {
      type: "submit",
      class: "btn btn-sm btn-primary",
      "data-action": "set-password",
      disabled: m.pending ? true : undefined,
    },
    [passwordAction(hasPassword)],
  );
  const form = el("form", { class: "access-form", "data-form": "password" }, [input, submit]);
  form.addEventListener("submit", (ev) => {
    ev.preventDefault();
    const field = asInput(form.querySelector('[data-field="password"]'));
    if (field === null) {
      return;
    }
    const password = field.value;
    field.value = "";
    void onSetPassword(state, password);
  });
  return el("section", { class: "card", "data-card": "password", "aria-label": "Password" }, [
    el("div", { class: "card-head" }, [
      el("div", {}, [
        el("div", { class: "card-title" }, ["Password"]),
        el("div", { class: "card-sub", "data-password-sub": "" }, [PASSWORD_SUB]),
      ]),
    ]),
    form,
  ]);
}

function keyCard(): HTMLElement {
  return el("section", { class: "card card-plain", "data-card": "key", "aria-label": KEY_TITLE }, [
    el("div", {}, [
      el("div", { class: "card-title" }, [KEY_TITLE]),
      el("div", { class: "card-text" }, [KEY_BODY]),
    ]),
    el("div", { class: "key-label spacer", "data-key-label": "" }, [keyLabel(keyholder.remainingMs())]),
  ]);
}
