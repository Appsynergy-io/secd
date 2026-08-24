/** Account: sessions + Revoke, passkeys + Add/Remove, DEK as a factor chain. */

import {
  BREAKPOINT_PX,
  FAIL_SENTENCE,
  LAST_FACTOR_SENTENCE,
  NO_DEK_SENTENCE,
  RATE_SENTENCE,
  errorMessage,
  layoutMode,
  logoutUrl,
  passkeyDeletePath,
  passkeyRegisterFinishUrl,
  passkeyRegisterStartUrl,
  passkeysUrl,
  removePasskeyEnabled,
  req,
  sessionRevokePath,
  sessionsUrl,
  sessionUrl,
  type LayoutMode,
} from "../lib/api.ts";
import { copyText } from "../lib/clipboard.ts";
import { getDek, toHex, wrapPasskey, wrapToJson, zeroizeBytes } from "../lib/crypto.ts";
import type { Signal } from "../lib/signal.ts";
import {
  coercePublicKey,
  createPasskey,
  prfBytes,
  serializeCredential,
} from "../lib/webauthn.ts";

export const CLIP_FAIL_SENTENCE =
  "The browser refused the clipboard. Select the value and copy it.";
export const LOADING_SESSIONS = "Loading sessions.";
export const EMPTY_SESSIONS = "No sessions.";
export const LOADING_PASSKEYS = "Loading passkeys.";
export const EMPTY_PASSKEYS = "No passkeys.";
export const SELECT_TITLE = "Select a session or passkey";
export const SELECT_BODY = "Choose a row from the list.";
export const CHAIN_TITLE = "Vault key";

export type DekFactor = "passkey" | "password";

export type SessionInfo = {
  email: string;
  has_passkey: boolean;
  has_password: boolean;
  session_id: string;
};

export type SessionRow = {
  id: string;
  kind: string;
  label: string;
  created: string;
  last_seen: string;
  current: boolean;
};

export type PasskeyRow = {
  id: string;
  created: string;
};

export type AccountSel = { kind: "session" | "passkey"; id: string };

export type AccountHost = {
  path: Signal<string>;
  error: Signal<string | undefined>;
  pending: Signal<boolean>;
  session: Signal<SessionInfo | undefined>;
  passkeys: Signal<PasskeyRow[] | undefined>;
};

export type AccountActions = {
  onLogout(): void;
  loadSession(): Promise<void>;
  wipeDek(): Promise<void>;
  redraw(): void;
};

type Mem = {
  sessions: SessionRow[] | undefined;
  sessionsError: string | undefined;
  selected: AccountSel | undefined;
  copied: boolean;
  clipFail: boolean;
  nav: HTMLElement | undefined;
  widthPx: number | undefined;
  actions: AccountActions | undefined;
  sessionLoads: boolean;
  passkeyLoads: boolean;
};

const mem = new WeakMap<object, Mem>();
const focusHints = new WeakMap<object, "copy" | "fallback">();
let logoutGen = 0;
let passkeyLoadGen = 0;

function memOf(state: object): Mem {
  let m = mem.get(state);
  if (m === undefined) {
    m = {
      sessions: undefined,
      sessionsError: undefined,
      selected: undefined,
      copied: false,
      clipFail: false,
      nav: undefined,
      widthPx: undefined,
      actions: undefined,
      sessionLoads: false,
      passkeyLoads: false,
    };
    mem.set(state, m);
  }
  return m;
}

export function currentLogoutGen(): number {
  return logoutGen;
}

export function bumpLogoutGen(): void {
  logoutGen += 1;
}

export function leaveAccount(state: object): void {
  const m = memOf(state);
  passkeyLoadGen += 1;
  m.sessions = undefined;
  m.sessionsError = undefined;
  m.selected = undefined;
  m.copied = false;
  m.clipFail = false;
  m.sessionLoads = false;
  m.passkeyLoads = false;
  const host = state as AccountHost;
  if ("passkeys" in host && host.passkeys !== undefined) {
    host.passkeys.set(undefined);
  }
}

export function dekFactors(q: {
  has_passkey: boolean;
  has_password: boolean;
}): DekFactor[] {
  const out: DekFactor[] = [];
  if (q.has_passkey) {
    out.push("passkey");
  }
  if (q.has_password) {
    out.push("password");
  }
  return out;
}

export function lastFactor(factors: readonly DekFactor[]): boolean {
  return factors.length === 1;
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

function failSentence(status: number, data?: unknown): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  if (errorMessage(data) === "last factor") {
    return LAST_FACTOR_SENTENCE;
  }
  return FAIL_SENTENCE;
}

export function parseSessions(v: unknown): SessionRow[] {
  const rec = typeof v === "object" && v !== null ? (v as Record<string, unknown>) : undefined;
  const rows = rec?.["sessions"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: SessionRow[] = [];
  for (const row of rows) {
    if (typeof row !== "object" || row === null) {
      continue;
    }
    const r = row as Record<string, unknown>;
    const id = r["id"];
    if (typeof id !== "string" || id === "") {
      continue;
    }
    out.push({
      id,
      kind: typeof r["kind"] === "string" ? r["kind"] : "",
      label: typeof r["label"] === "string" ? r["label"] : "",
      created: typeof r["created"] === "string" ? r["created"] : "",
      last_seen: typeof r["last_seen"] === "string" ? r["last_seen"] : "",
      current: r["current"] === true,
    });
  }
  return out;
}

export function parsePasskeys(v: unknown): PasskeyRow[] {
  const rec = typeof v === "object" && v !== null ? (v as Record<string, unknown>) : undefined;
  const rows = rec?.["passkeys"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: PasskeyRow[] = [];
  for (const row of rows) {
    if (typeof row !== "object" || row === null) {
      continue;
    }
    const r = row as Record<string, unknown>;
    const id = r["id"];
    if (typeof id !== "string" || id === "") {
      continue;
    }
    const created = r["created"];
    out.push({
      id,
      created: typeof created === "string" ? created : "",
    });
  }
  return out;
}

export function shortId(id: string): string {
  const chars = [...id];
  if (chars.length > 12) {
    return `${chars.slice(0, 12).join("")}\u2026`;
  }
  return id;
}

export function createdDay(created: string): string {
  const i = created.indexOf("T");
  return i >= 0 ? created.slice(0, i) : created;
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

function defaultNav(): HTMLElement {
  const items: Array<[string, string]> = [
    ["/register", "Register"],
    ["/activity", "Activity"],
    ["/account", "Account"],
  ];
  return el(
    "nav",
    { class: "nav", "aria-label": "Console" },
    items.map(([href, label]) =>
      el(
        "a",
        {
          href,
          "aria-current": href === "/account" ? "page" : undefined,
        },
        [label],
      ),
    ),
  );
}

function redraw(state: AccountHost, root: HTMLElement): void {
  const m = memOf(state);
  if (m.actions !== undefined) {
    m.actions.redraw();
    return;
  }
  if (state.path.get() !== "/account") {
    return;
  }
  paint(state, root);
}

export function renderAccount(
  state: AccountHost,
  root: HTMLElement,
  nav?: HTMLElement,
  actions?: AccountActions,
  widthPx?: number,
): void {
  const m = memOf(state);
  if (nav !== undefined) {
    m.nav = nav;
  }
  if (actions !== undefined) {
    m.actions = actions;
  }
  if (widthPx !== undefined) {
    m.widthPx = widthPx;
  }
  paint(state, root);
  if (state.passkeys.get() === undefined) {
    void loadPasskeys(state, root);
  }
  if (m.sessions === undefined) {
    void loadSessions(state, root);
  }
}

async function loadPasskeys(state: AccountHost, root: HTMLElement): Promise<void> {
  const m = memOf(state);
  if (
    state.passkeys.get() !== undefined ||
    m.passkeyLoads ||
    state.session.get() === undefined
  ) {
    return;
  }
  m.passkeyLoads = true;
  const gen = logoutGen;
  const loadGen = passkeyLoadGen;
  state.error.set(undefined);
  try {
    const res = await req("GET", passkeysUrl());
    if (
      gen !== logoutGen ||
      loadGen !== passkeyLoadGen ||
      state.session.get() === undefined ||
      state.path.get() !== "/account"
    ) {
      return;
    }
    if (res.status !== 200) {
      state.error.set(failSentence(res.status, res.data));
      return;
    }
    state.passkeys.set(parsePasskeys(res.data));
    state.error.set(undefined);
  } catch {
    if (
      gen !== logoutGen ||
      loadGen !== passkeyLoadGen ||
      state.session.get() === undefined ||
      state.path.get() !== "/account"
    ) {
      return;
    }
    state.error.set(FAIL_SENTENCE);
  } finally {
    const stale = gen !== logoutGen || loadGen !== passkeyLoadGen;
    const left = state.session.get() === undefined || state.path.get() !== "/account";
    if (!stale && (left || state.passkeys.get() !== undefined)) {
      m.passkeyLoads = false;
    }
    if (!stale) {
      redraw(state, root);
    }
  }
}

async function loadSessions(state: AccountHost, root: HTMLElement): Promise<void> {
  const m = memOf(state);
  if (m.sessions !== undefined || m.sessionLoads || state.session.get() === undefined) {
    return;
  }
  m.sessionLoads = true;
  const gen = logoutGen;
  const loadGen = passkeyLoadGen;
  m.sessionsError = undefined;
  try {
    const res = await req("GET", sessionsUrl());
    if (
      gen !== logoutGen ||
      loadGen !== passkeyLoadGen ||
      state.session.get() === undefined ||
      state.path.get() !== "/account"
    ) {
      return;
    }
    if (res.status !== 200) {
      m.sessionsError = failSentence(res.status, res.data);
      return;
    }
    m.sessions = parseSessions(res.data);
    m.sessionsError = undefined;
  } catch {
    if (
      gen !== logoutGen ||
      loadGen !== passkeyLoadGen ||
      state.session.get() === undefined ||
      state.path.get() !== "/account"
    ) {
      return;
    }
    m.sessionsError = FAIL_SENTENCE;
  } finally {
    const stale = gen !== logoutGen || loadGen !== passkeyLoadGen;
    if (!stale) {
      m.sessionLoads = false;
      redraw(state, root);
    }
  }
}

async function loadSessionFallback(state: AccountHost): Promise<void> {
  const gen = logoutGen;
  const res = await req("GET", sessionUrl());
  if (gen !== logoutGen) {
    return;
  }
  if (res.status === 401 || res.status === 403) {
    await signOut(state);
    return;
  }
  if (res.status !== 200) {
    state.error.set(failSentence(res.status, res.data));
    return;
  }
  const data = asSession(res.data);
  if (data === undefined) {
    state.error.set(FAIL_SENTENCE);
    return;
  }
  state.session.set(data);
}

function asSession(v: unknown): SessionInfo | undefined {
  const rec = typeof v === "object" && v !== null ? (v as Record<string, unknown>) : undefined;
  if (!rec) {
    return undefined;
  }
  const email = rec["email"];
  const sessionId = rec["session_id"];
  if (typeof email !== "string" || typeof sessionId !== "string") {
    return undefined;
  }
  return {
    email,
    session_id: sessionId,
    has_passkey: rec["has_passkey"] === true,
    has_password: rec["has_password"] === true,
  };
}

async function wipeDekFallback(): Promise<void> {
  try {
    const crypto = await import("../lib/crypto.ts");
    crypto.clearDek();
  } catch {
    /* session is already cleared */
  }
}

async function signOut(state: AccountHost): Promise<void> {
  const m = memOf(state);
  if (m.actions !== undefined) {
    await m.actions.wipeDek();
    m.actions.onLogout();
    return;
  }
  leaveAccount(state);
  state.session.set(undefined);
  state.pending.set(false);
  state.error.set(undefined);
  if (globalThis.location && globalThis.location.pathname !== "/") {
    globalThis.history?.pushState(null, "", "/");
  }
  state.path.set("/");
  await wipeDekFallback();
}

async function onLogoutClick(state: AccountHost, _root: HTMLElement): Promise<void> {
  const m = memOf(state);
  if (m.actions !== undefined) {
    m.actions.onLogout();
    return;
  }
  state.pending.set(true);
  state.error.set(undefined);
  bumpLogoutGen();
  try {
    await req("POST", logoutUrl());
  } catch {
    /* POST /logout still signs out */
  } finally {
    await signOut(state);
  }
}

async function onRemove(
  state: AccountHost,
  root: HTMLElement,
  id: string,
): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  const session = state.session.get();
  const loaded = state.passkeys.get();
  if (
    loaded === undefined ||
    !removePasskeyEnabled(loaded.length, session?.has_password === true)
  ) {
    return;
  }
  if (id === "") {
    return;
  }
  const gen = logoutGen;
  state.pending.set(true);
  state.error.set(undefined);
  const m = memOf(state);
  try {
    const res = await req("DELETE", passkeyDeletePath(id));
    if (gen !== logoutGen) {
      return;
    }
    if (res.status !== 200) {
      state.error.set(failSentence(res.status, res.data));
      return;
    }
    state.passkeys.set(undefined);
    if (m.actions !== undefined) {
      await m.actions.loadSession();
    } else {
      await loadSessionFallback(state);
    }
    if (state.session.get() === undefined || state.error.get() !== undefined) {
      return;
    }
  } catch {
    if (gen !== logoutGen) {
      return;
    }
    state.error.set(FAIL_SENTENCE);
  } finally {
    if (gen !== logoutGen) {
      return;
    }
    state.pending.set(false);
    redraw(state, root);
  }
}

async function onRevoke(
  state: AccountHost,
  root: HTMLElement,
  id: string,
): Promise<void> {
  if (state.pending.get() || id === "") {
    return;
  }
  const m = memOf(state);
  const row = m.sessions?.find((s) => s.id === id);
  const gen = logoutGen;
  state.pending.set(true);
  state.error.set(undefined);
  try {
    const res = await req("DELETE", sessionRevokePath(id));
    if (gen !== logoutGen) {
      return;
    }
    if (res.status !== 200) {
      state.error.set(failSentence(res.status, res.data));
      return;
    }
    if (row?.current === true) {
      await signOut(state);
      return;
    }
    m.sessions = undefined;
  } catch {
    if (gen !== logoutGen) {
      return;
    }
    state.error.set(FAIL_SENTENCE);
  } finally {
    if (gen !== logoutGen) {
      return;
    }
    state.pending.set(false);
    redraw(state, root);
  }
}

async function onAdd(state: AccountHost, root: HTMLElement): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  const email = state.session.get()?.email;
  if (email === undefined || email === "") {
    return;
  }
  const dek = getDek();
  if (dek === undefined) {
    state.error.set(NO_DEK_SENTENCE);
    paint(state, root);
    return;
  }
  const gen = logoutGen;
  state.pending.set(true);
  state.error.set(undefined);
  paint(state, root);
  try {
    const start = await req("POST", passkeyRegisterStartUrl(), { email });
    if (gen !== logoutGen) {
      return;
    }
    if (start.status !== 200) {
      state.error.set(failSentence(start.status, start.data));
      return;
    }
    const pk = coercePublicKey(start.data) as unknown as PublicKeyCredentialCreationOptions;
    const cred = await createPasskey(pk);
    const prf = prfBytes(cred);
    try {
      if (prf === undefined) {
        state.error.set(FAIL_SENTENCE);
        return;
      }
      const wrap = wrapPasskey(dek, prf, toHex(new Uint8Array(cred.rawId)));
      const handle =
        typeof (start.data as { handle?: unknown }).handle === "string"
          ? (start.data as { handle: string }).handle
          : "";
      const finish = await req("POST", passkeyRegisterFinishUrl(), {
        handle,
        credential: serializeCredential(cred),
        wrap: wrapToJson(wrap),
        email,
      });
      if (gen !== logoutGen) {
        return;
      }
      if (finish.status !== 200) {
        state.error.set(failSentence(finish.status, finish.data));
        return;
      }
      state.passkeys.set(undefined);
      const m = memOf(state);
      if (m.actions !== undefined) {
        await m.actions.loadSession();
      } else {
        await loadSessionFallback(state);
      }
    } finally {
      if (prf !== undefined) {
        zeroizeBytes(prf);
      }
    }
  } catch {
    if (gen !== logoutGen) {
      return;
    }
    state.error.set(FAIL_SENTENCE);
  } finally {
    if (gen !== logoutGen) {
      return;
    }
    state.pending.set(false);
    redraw(state, root);
  }
}

async function onCopy(state: AccountHost, root: HTMLElement, text: string): Promise<void> {
  const m = memOf(state);
  if (state.pending.get() || text === "") {
    return;
  }
  const ok = await copyText(text);
  m.copied = ok;
  m.clipFail = !ok;
  focusHints.set(state, ok ? "copy" : "fallback");
  paint(state, root);
}

function select(state: AccountHost, root: HTMLElement, sel: AccountSel): void {
  const m = memOf(state);
  m.selected = sel;
  m.copied = false;
  m.clipFail = false;
  paint(state, root);
}

function paint(state: AccountHost, root: HTMLElement): void {
  const m = memOf(state);
  const prev = document.activeElement;
  const prevEl = prev instanceof HTMLElement && root.contains(prev) ? prev : undefined;
  const prevSessionId = prevEl?.closest("[data-session-id]")?.getAttribute("data-session-id");
  const prevPasskeyId = prevEl?.closest("[data-passkey-id]")?.getAttribute("data-passkey-id");
  const prevIdentity =
    prevEl instanceof HTMLButtonElement && !prevEl.hasAttribute("data-action");
  const hint = focusHints.get(state);
  focusHints.delete(state);
  const session = state.session.get();
  const factors = dekFactors({
    has_passkey: session?.has_passkey === true,
    has_password: session?.has_password === true,
  });
  const loaded = state.passkeys.get();
  const hasPassword = session?.has_password === true;
  const last =
    loaded !== undefined && !removePasskeyEnabled(loaded.length, hasPassword);
  const removeId = loaded?.[0]?.id;
  const removeOk =
    loaded !== undefined &&
    removeId !== undefined &&
    removeId !== "" &&
    removePasskeyEnabled(loaded.length, hasPassword);
  const pending = state.pending.get();
  const err = state.error.get();
  const layout = layoutFor(m);
  const dekHeld = getDek() !== undefined;
  const selected = selectedRow(m, loaded);

  const links = factors.map((factor) => {
    const label = factor === "passkey" ? "Passkey" : "Password";
    const children: Array<Node | string> = [el("span", { class: "mono" }, [label])];
    if (factor === "passkey") {
      const remove = el(
        "button",
        {
          type: "button",
          class: "danger",
          "data-action": "remove",
          disabled: removeOk && !pending ? undefined : true,
        },
        ["Remove"],
      );
      remove.addEventListener("click", () => {
        const id = state.passkeys.get()?.[0]?.id;
        if (id !== undefined) {
          void onRemove(state, root, id);
        }
      });
      children.push(remove);
    }
    return el("li", { class: "chain-link", "data-factor": factor }, children);
  });
  const chain = el(
    "div",
    {
      class: "chain",
      "data-chain": "dek",
      "data-last": last ? "1" : "0",
    },
    [
      el("h2", {}, [CHAIN_TITLE]),
      el("ul", { class: "chain-links" }, links),
      last ? el("p", { class: "chain-reason" }, [LAST_FACTOR_SENTENCE]) : "",
    ],
  );

  const add = el(
    "button",
    {
      type: "button",
      "data-action": "add-passkey",
      disabled: pending || !dekHeld ? true : undefined,
    },
    ["Add passkey"],
  );
  add.addEventListener("click", () => {
    void onAdd(state, root);
  });

  const listPane = el("div", { class: "card", "data-pane": "list" }, [
    el("h2", {}, ["Sessions"]),
    sessionsListEl(state, root, pending),
    el("h2", {}, ["Passkeys"]),
    passkeysListEl(state, root, pending, removeOk),
    add,
    !dekHeld ? el("p", { "data-reason": "" }, [NO_DEK_SENTENCE]) : "",
  ]);
  const inspector = detailsEl(state, root, selected, false, pending);
  const workspace = el("div", { class: "workspace" }, [listPane, inspector]);

  const out = el("button", { type: "button", class: "secondary", "data-action": "logout" }, [
    "Sign out",
  ]);
  out.addEventListener("click", () => {
    void onLogoutClick(state, root);
  });

  const children: Array<Node | string> = [
    m.nav ?? defaultNav(),
    el("h1", {}, ["Account"]),
    el("p", { class: "mono", "data-field": "email" }, [session?.email ?? ""]),
    chain,
    workspace,
  ];
  if (selected !== undefined) {
    children.push(detailsEl(state, root, selected, true, pending));
  }
  children.push(out);
  if (m.clipFail) {
    children.push(el("p", { class: "error", "data-reason": "" }, [CLIP_FAIL_SENTENCE]));
    const copyVal = copyValue(selected);
    if (copyVal !== "") {
      children.push(
        el("input", {
          class: "mono",
          readonly: true,
          autocomplete: "off",
          "data-copy-fallback": "",
          "aria-label": "Identifier",
          value: copyVal,
        }),
      );
    }
  }
  if (err) {
    children.push(el("p", { class: "error" }, [err]));
  }

  const page = el(
    "div",
    {
      class: "app",
      "data-page": "account",
      "data-layout": layout,
      "data-last": last ? "1" : "0",
    },
    children,
  );
  root.replaceChildren(page);
  let target: HTMLElement | null = null;
  if (hint === "fallback" || m.clipFail) {
    const found = page.querySelector("[data-copy-fallback]");
    target = found instanceof HTMLElement ? found : null;
  } else if (hint === "copy") {
    const sel =
      layout === "list-only"
        ? '[data-pane="sheet"] [data-action="copy"]'
        : '[data-pane="inspector"] [data-action="copy"]';
    const found = page.querySelector(sel) ?? page.querySelector('[data-action="copy"]');
    target = found instanceof HTMLElement ? found : null;
  } else if (prevIdentity && prevSessionId !== null && prevSessionId !== undefined) {
    const found = page.querySelector(
      `[data-session-id="${prevSessionId}"] button:not([data-action])`,
    );
    target = found instanceof HTMLElement ? found : null;
  } else if (prevIdentity && prevPasskeyId !== null && prevPasskeyId !== undefined) {
    const found = page.querySelector(
      `[data-passkey-id="${prevPasskeyId}"] button:not([data-action])`,
    );
    target = found instanceof HTMLElement ? found : null;
  }
  if (target !== null && typeof target.focus === "function") {
    target.focus();
  }
}

type Selected =
  | { kind: "session"; row: SessionRow }
  | { kind: "passkey"; row: PasskeyRow }
  | undefined;

function selectedRow(m: Mem, passkeys: PasskeyRow[] | undefined): Selected {
  const sel = m.selected;
  if (sel === undefined) {
    return undefined;
  }
  if (sel.kind === "session") {
    const row = m.sessions?.find((s) => s.id === sel.id);
    return row === undefined ? undefined : { kind: "session", row };
  }
  const row = passkeys?.find((p) => p.id === sel.id);
  return row === undefined ? undefined : { kind: "passkey", row };
}

function copyValue(sel: Selected): string {
  if (sel === undefined) {
    return "";
  }
  return sel.row.id;
}

function sessionsListEl(
  state: AccountHost,
  root: HTMLElement,
  pending: boolean,
): HTMLElement {
  const m = memOf(state);
  const body: Array<Node | string> = [];
  if (m.sessions === undefined) {
    if (m.sessionsError !== undefined) {
      body.push(el("p", { class: "error", "data-reason": "" }, [m.sessionsError]));
    } else {
      body.push(el("p", { "data-state": "loading" }, [LOADING_SESSIONS]));
    }
  } else if (m.sessions.length === 0) {
    body.push(el("p", { "data-state": "empty" }, [EMPTY_SESSIONS]));
  } else {
    for (const row of m.sessions) {
      body.push(sessionRowEl(state, root, row, pending));
    }
  }
  return el("div", { class: "list", "data-list": "sessions" }, body);
}

function sessionRowEl(
  state: AccountHost,
  root: HTMLElement,
  row: SessionRow,
  pending: boolean,
): HTMLElement {
  const m = memOf(state);
  const current = m.selected?.kind === "session" && m.selected.id === row.id;
  const wrap = el("div", {
    class: "row",
    "data-session-id": row.id,
    "data-current": row.current ? "true" : undefined,
  });
  const identity = el(
    "button",
    {
      type: "button",
      class: "secondary",
      "aria-current": current ? "true" : undefined,
    },
    [
      el("span", { class: "mono", "data-name": "" }, [row.label === "" ? row.id : row.label]),
      el("span", {}, [`${row.kind} · ${row.last_seen}`]),
    ],
  );
  identity.addEventListener("click", () => {
    select(state, root, { kind: "session", id: row.id });
  });
  const revoke = el(
    "button",
    {
      type: "button",
      class: "danger",
      "data-action": "revoke",
      disabled: pending ? true : undefined,
    },
    ["Revoke"],
  );
  revoke.addEventListener("click", () => {
    void onRevoke(state, root, row.id);
  });
  wrap.append(identity, revoke);
  return wrap;
}

function passkeysListEl(
  state: AccountHost,
  root: HTMLElement,
  pending: boolean,
  removeOk: boolean,
): HTMLElement {
  const loaded = state.passkeys.get();
  const err = state.error.get();
  const body: Array<Node | string> = [];
  if (loaded === undefined) {
    if (err !== undefined) {
      body.push(el("p", { class: "error", "data-reason": "" }, [err]));
    } else {
      body.push(el("p", { "data-state": "loading" }, [LOADING_PASSKEYS]));
    }
  } else if (loaded.length === 0) {
    body.push(el("p", { "data-state": "empty" }, [EMPTY_PASSKEYS]));
  } else {
    for (const row of loaded) {
      body.push(passkeyRowEl(state, root, row, pending, removeOk));
    }
  }
  return el("div", { class: "list", "data-list": "passkeys" }, body);
}

function passkeyRowEl(
  state: AccountHost,
  root: HTMLElement,
  row: PasskeyRow,
  pending: boolean,
  removeOk: boolean,
): HTMLElement {
  const m = memOf(state);
  const current = m.selected?.kind === "passkey" && m.selected.id === row.id;
  const wrap = el("div", {
    class: "row",
    "data-passkey-id": row.id,
  });
  const identity = el(
    "button",
    {
      type: "button",
      class: "secondary",
      "aria-current": current ? "true" : undefined,
    },
    [
      el("span", { class: "mono", "data-name": "" }, [shortId(row.id)]),
      el("span", {}, [createdDay(row.created)]),
    ],
  );
  identity.addEventListener("click", () => {
    select(state, root, { kind: "passkey", id: row.id });
  });
  const remove = el(
    "button",
    {
      type: "button",
      class: "danger",
      "data-action": "remove",
      disabled: removeOk && !pending ? undefined : true,
    },
    ["Remove"],
  );
  remove.addEventListener("click", () => {
    void onRemove(state, root, row.id);
  });
  wrap.append(identity, remove);
  return wrap;
}

function detailsEl(
  state: AccountHost,
  root: HTMLElement,
  selected: Selected,
  sheet: boolean,
  pending: boolean,
): HTMLElement {
  const m = memOf(state);
  const value = copyValue(selected);
  const copyDisabled = pending || value === "";
  const copy = el(
    "button",
    {
      type: "button",
      "data-action": "copy",
      disabled: copyDisabled ? true : undefined,
    },
    [m.copied && value !== "" ? "Copied" : "Copy"],
  );
  copy.addEventListener("click", () => {
    void onCopy(state, root, value);
  });
  const body: Array<Node | string> = [];
  if (selected === undefined) {
    body.push(el("h2", {}, [SELECT_TITLE]));
    body.push(el("p", {}, [SELECT_BODY]));
  } else if (selected.kind === "session") {
    const row = selected.row;
    body.push(el("h2", { class: "mono", "data-field": "session" }, [row.label || row.id]));
    body.push(el("p", { class: "mono", "data-field": "" }, [row.id]));
    body.push(el("p", {}, [`${row.kind} · ${row.last_seen}`]));
    if (row.current) {
      body.push(el("p", {}, ["This browser"]));
    }
    const revoke = el(
      "button",
      {
        type: "button",
        class: "danger",
        "data-action": "revoke",
        disabled: pending ? true : undefined,
      },
      ["Revoke"],
    );
    revoke.addEventListener("click", () => {
      void onRevoke(state, root, row.id);
    });
    body.push(revoke);
  } else {
    const row = selected.row;
    const session = state.session.get();
    const loaded = state.passkeys.get();
    const removeOk =
      loaded !== undefined &&
      removePasskeyEnabled(loaded.length, session?.has_password === true);
    body.push(el("h2", { class: "mono", "data-field": "passkey" }, [shortId(row.id)]));
    body.push(el("p", { class: "hex", "data-field": "" }, [row.id]));
    body.push(el("p", {}, [createdDay(row.created)]));
    const remove = el(
      "button",
      {
        type: "button",
        class: "danger",
        "data-action": "remove",
        disabled: removeOk && !pending ? undefined : true,
      },
      ["Remove"],
    );
    remove.addEventListener("click", () => {
      void onRemove(state, root, row.id);
    });
    body.push(remove);
    if (!removeOk && loaded !== undefined) {
      body.push(el("p", { class: "chain-reason" }, [LAST_FACTOR_SENTENCE]));
    }
  }
  body.push(copy);
  if (copyDisabled) {
    body.push(el("p", { "data-reason": "" }, [SELECT_BODY]));
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
    return el(
      "div",
      { class: "secd-overlay", "data-pane": "sheet", "data-sheet": "open" },
      [el("div", { class: "secd-modal" }, [el("div", { class: "card" }, body)])],
    );
  }
  return el("div", { class: "card", "data-pane": "inspector" }, body);
}
