import {
  BREAKPOINT_PX,
  EMAIL_AUTOCOMPLETE,
  FAIL_SENTENCE,
  LAST_FACTOR_SENTENCE,
  LAST_KEY,
  RATE_SENTENCE,
  REMEMBER_DAYS,
  deviceQuery,
  layoutMode,
  logoutUrl,
  passkeyDeletePath,
  passkeysUrl,
  passwordLoginUrl,
  removePasskeyEnabled,
  req,
  sessionUrl,
  startUrl,
  type LayoutMode,
} from "./lib/api.ts";
import { copyText } from "./lib/clipboard.ts";
import { signal } from "./lib/signal.ts";
import {
  coercePublicKey,
  getPasskey,
  serializeCredential,
} from "./lib/webauthn.ts";
import { renderDevice } from "./screens/device.ts";

export type Screen = "gate" | "device" | "register" | "activity" | "account";

export const SCREENS: readonly Screen[] = [
  "gate",
  "device",
  "register",
  "activity",
  "account",
];

export function hrefFor(screen: Screen): string {
  switch (screen) {
    case "device":
      return "/device";
    case "register":
      return "/register";
    case "activity":
      return "/activity";
    case "account":
      return "/account";
    default:
      return "/";
  }
}

export type DekFactor = "passkey" | "password";

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

export function currentLayout(widthPx = globalThis.innerWidth): LayoutMode {
  const w =
    typeof widthPx === "number" && Number.isFinite(widthPx) && widthPx > 0
      ? widthPx
      : BREAKPOINT_PX;
  return layoutMode(w);
}

export { layoutMode, removePasskeyEnabled };

export function screenFromPath(path: string): Screen {
  switch (path) {
    case "/activity":
      return "activity";
    case "/account":
      return "account";
    case "/device":
      return "device";
    case "/register":
      return "register";
    default:
      return "gate";
  }
}

/** A CLI approval link carries a user code; it lands on the device screen. */
export function initialPath(path: string, userCode: string): string {
  return userCode === "" ? path : "/device";
}

type AuthMethod = "register" | "passkey" | "password" | "either";

export type SessionInfo = {
  email: string;
  has_passkey: boolean;
  has_password: boolean;
  session_id: string;
};

export type PasskeyRow = {
  id: string;
  created: string;
};

type Remembered = {
  email: string;
  has_passkey: boolean;
  at: string;
};

export type GateKind =
  | "approve-only"
  | "remembered-passkey"
  | "remembered-password"
  | "cold"
  | "identity";

export type GateView = {
  kind: GateKind;
  showEmail: boolean;
  showPassword: boolean;
  showPasskey: boolean;
  showApprove: boolean;
  emailAutocomplete: string | undefined;
  emailPrefill: string | undefined;
  showUseDifferentAccount: boolean;
  showUsePasswordInstead: boolean;
  userCode: string | undefined;
};

function rememberIsFresh(atIso: string, nowMs: number): boolean {
  const at = Date.parse(atIso);
  if (Number.isNaN(at)) {
    return false;
  }
  return nowMs - at <= REMEMBER_DAYS * 24 * 60 * 60 * 1000;
}

export function resolveGate(q: {
  session?: SessionInfo | undefined;
  remember?: Remembered | undefined;
  email?: string | undefined;
  nowMs?: number | undefined;
  method?: AuthMethod | undefined;
  useDifferentAccount?: boolean | undefined;
  revealPassword?: boolean | undefined;
  userCode?: string | undefined;
}): GateView {
  if (q.session) {
    return {
      kind: "approve-only",
      showEmail: false,
      showPassword: false,
      showPasskey: false,
      showApprove: true,
      emailAutocomplete: undefined,
      emailPrefill: undefined,
      showUseDifferentAccount: false,
      showUsePasswordInstead: false,
      userCode: q.userCode,
    };
  }
  const now = q.nowMs ?? Date.now();
  const remembered =
    q.remember && !q.useDifferentAccount && rememberIsFresh(q.remember.at, now)
      ? q.remember
      : undefined;
  if (remembered) {
    if (remembered.has_passkey) {
      return {
        kind: "remembered-passkey",
        showEmail: false,
        showPassword: false,
        showPasskey: true,
        showApprove: false,
        emailAutocomplete: undefined,
        emailPrefill: remembered.email,
        showUseDifferentAccount: true,
        showUsePasswordInstead: false,
        userCode: q.userCode,
      };
    }
    return {
      kind: "remembered-password",
      showEmail: false,
      showPassword: true,
      showPasskey: false,
      showApprove: false,
      emailAutocomplete: undefined,
      emailPrefill: remembered.email,
      showUseDifferentAccount: true,
      showUsePasswordInstead: false,
      userCode: q.userCode,
    };
  }
  if (q.method) {
    const showPassword =
      q.method === "password" ||
      q.method === "register" ||
      (q.method === "either" && Boolean(q.revealPassword));
    const showPasskey =
      q.method === "passkey" || q.method === "register" || q.method === "either";
    const showUsePassword = q.method === "either" && !q.revealPassword;
    return {
      kind: "identity",
      showEmail: true,
      showPassword,
      showPasskey,
      showApprove: false,
      emailAutocomplete: EMAIL_AUTOCOMPLETE,
      emailPrefill: q.email ?? q.remember?.email,
      showUseDifferentAccount: Boolean(q.remember),
      showUsePasswordInstead: showUsePassword,
      userCode: q.userCode,
    };
  }
  return {
    kind: "cold",
    showEmail: true,
    showPassword: false,
    showPasskey: false,
    showApprove: false,
    emailAutocomplete: EMAIL_AUTOCOMPLETE,
    emailPrefill: q.email ?? q.remember?.email,
    showUseDifferentAccount: false,
    showUsePasswordInstead: false,
    userCode: q.userCode,
  };
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
    if (v === true) {
      node.setAttribute(k, "");
    } else {
      node.setAttribute(k, v);
    }
  }
  for (const child of children) {
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

export type AppState = {
  path: ReturnType<typeof signal<string>>;
  email: ReturnType<typeof signal<string>>;
  password: ReturnType<typeof signal<string>>;
  error: ReturnType<typeof signal<string | undefined>>;
  pending: ReturnType<typeof signal<boolean>>;
  session: ReturnType<typeof signal<SessionInfo | undefined>>;
  method: ReturnType<typeof signal<AuthMethod | undefined>>;
  different: ReturnType<typeof signal<boolean>>;
  revealPassword: ReturnType<typeof signal<boolean>>;
  userCode: ReturnType<typeof signal<string>>;
  eph: ReturnType<typeof signal<string>>;
  passkeys: ReturnType<typeof signal<PasskeyRow[] | undefined>>;
};

let mounted: HTMLElement | undefined;
const passkeyLoads = new WeakSet<object>();
let logoutGen = 0;
let passkeyLoadGen = 0;

function loadRemember(): Remembered | undefined {
  try {
    const raw = localStorage.getItem(LAST_KEY);
    if (!raw) {
      return undefined;
    }
    const v = JSON.parse(raw) as unknown;
    if (typeof v !== "object" || v === null) {
      return undefined;
    }
    const rec = v as Record<string, unknown>;
    if (typeof rec["email"] !== "string" || typeof rec["has_passkey"] !== "boolean") {
      return undefined;
    }
    if (typeof rec["at"] !== "string") {
      return undefined;
    }
    return {
      email: rec["email"],
      has_passkey: rec["has_passkey"],
      at: rec["at"],
    };
  } catch {
    return undefined;
  }
}

function saveRemember(email: string, hasPasskey: boolean): void {
  try {
    localStorage.setItem(
      LAST_KEY,
      JSON.stringify({
        email,
        has_passkey: hasPasskey,
        at: new Date().toISOString(),
      }),
    );
  } catch {
    /* ignore */
  }
}

function sentenceFor(status: number): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  return FAIL_SENTENCE;
}

function nav(state: AppState, screen: Screen): HTMLElement {
  const items: Array<[Screen, string]> = [
    ["register", "Register"],
    ["activity", "Activity"],
    ["account", "Account"],
  ];
  return el(
    "nav",
    { class: "nav", "aria-label": "Console" },
    items.map(([id, label]) =>
      el(
        "a",
        {
          href: hrefFor(id),
          "aria-current": screen === id ? "page" : undefined,
        },
        [label],
      ),
    ),
  );
}

function renderGate(state: AppState, root: HTMLElement): void {
  const view = resolveGate({
    session: state.session.get(),
    remember: loadRemember(),
    email: state.email.get() || undefined,
    method: state.method.get(),
    useDifferentAccount: state.different.get(),
    revealPassword: state.revealPassword.get(),
    userCode: state.userCode.get() || undefined,
  });
  const form = el("form", { class: "secd-auth-form" });
  form.addEventListener("submit", (ev) => {
    ev.preventDefault();
    void onContinue(state);
  });
  if (view.showEmail) {
    const input = el("input", {
      id: "email",
      type: "email",
      name: "email",
      autocomplete: view.emailAutocomplete ?? EMAIL_AUTOCOMPLETE,
      value: view.emailPrefill ?? state.email.get(),
    });
    input.addEventListener("input", () => {
      state.email.set(input.value);
    });
    form.append(el("label", { for: "email" }, ["Email"]), input);
  }
  if (view.showPassword) {
    const input = el("input", {
      id: "password",
      type: "password",
      name: "password",
      autocomplete: "current-password",
    });
    input.value = state.password.get();
    input.addEventListener("input", () => {
      state.password.set(input.value);
    });
    form.append(el("label", { for: "password" }, ["Password"]), input);
  }
  if (view.showApprove) {
    form.append(
      el("button", { type: "submit", "data-action": "approve" }, ["Approve"]),
    );
  } else {
    form.append(el("button", { type: "submit" }, ["Continue"]));
  }
  if (view.showPasskey) {
    const pk = el("button", { type: "button", class: "secondary", "data-action": "passkey" }, [
      "Passkey",
    ]);
    pk.addEventListener("click", () => {
      void onPasskey(state);
    });
    form.append(pk);
  }
  if (view.showUsePasswordInstead) {
    const pw = el(
      "button",
      { type: "button", class: "secondary", "data-action": "password" },
      ["Use password instead"],
    );
    pw.addEventListener("click", () => {
      state.revealPassword.set(true);
      render(state);
    });
    form.append(pw);
  }
  if (view.showUseDifferentAccount) {
    const diff = el(
      "button",
      { type: "button", class: "secondary", "data-action": "different" },
      ["Use a different account"],
    );
    diff.addEventListener("click", () => {
      state.different.set(true);
      render(state);
    });
    form.append(diff);
  }
  const err = state.error.get();
  const page = el("div", { class: "app", "data-page": "gate" }, [
    el("h1", {}, ["secd"]),
    form,
    err ? el("p", { class: "error" }, [err]) : "",
  ]);
  root.replaceChildren(page);
}

function inspectorCard(): HTMLElement {
  return el("div", { class: "card", "data-pane": "inspector" }, [
    el("h2", { class: "mono" }, ["Secret"]),
    el("p", {}, ["Select a secret"]),
  ]);
}

export function renderRegister(state: AppState, root: HTMLElement): void {
  mounted = root;
  const copy = el("button", { type: "button", "data-action": "copy" }, ["Copy"]);
  copy.addEventListener("click", () => {
    void copyText("••••••••");
  });
  const layout = currentLayout();
  const workspace = el("div", { class: "workspace" }, [
    el("div", { class: "card", "data-pane": "list" }, [
      el("div", { class: "list", "data-list": "secrets" }, [
        el("p", {}, ["No secrets yet."]),
      ]),
    ]),
    inspectorCard(),
  ]);
  root.replaceChildren(
    el("div", { class: "app", "data-page": "register", "data-layout": layout }, [
      nav(state, "register"),
      el("h1", {}, ["Register"]),
      el("p", {}, ["Copy is the default action."]),
      workspace,
      copy,
    ]),
  );
}

function renderActivity(state: AppState, root: HTMLElement): void {
  root.replaceChildren(
    el("div", { class: "app", "data-page": "activity" }, [
      nav(state, "activity"),
      el("h1", {}, ["Activity"]),
      el("p", {}, ["Audit metadata. Values are never listed."]),
      el("div", { class: "list", "data-list": "audit" }, [
        el("p", {}, ["No events"]),
      ]),
    ]),
  );
}

export function renderAccount(state: AppState, root: HTMLElement): void {
  mounted = root;
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
          disabled: removeOk ? undefined : true,
        },
        ["Remove"],
      );
      remove.addEventListener("click", () => {
        void onRemovePasskey(state);
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
      el("h2", {}, ["Vault key"]),
      el("ul", { class: "chain-links" }, links),
      last ? el("p", { class: "chain-reason" }, [LAST_FACTOR_SENTENCE]) : "",
    ],
  );
  const out = el("button", { type: "button", class: "secondary", "data-action": "logout" }, [
    "Sign out",
  ]);
  out.addEventListener("click", () => {
    void onLogout(state);
  });
  const err = state.error.get();
  root.replaceChildren(
    el("div", { class: "app", "data-page": "account" }, [
      nav(state, "account"),
      el("h1", {}, ["Account"]),
      el("p", {}, [session?.email ?? ""]),
      chain,
      el("h2", {}, ["Sessions"]),
      el("div", { class: "list", "data-list": "sessions" }),
      el("h2", {}, ["Passkeys"]),
      el("div", { class: "list", "data-list": "passkeys" }),
      out,
      err ? el("p", { class: "error" }, [err]) : "",
    ]),
  );
  if (loaded === undefined) {
    void loadAccountPasskeys(state);
  }
}

export function render(state: AppState): void {
  const root = mounted ?? document.getElementById("app");
  if (!root) {
    return;
  }
  mounted = root;
  const screen = screenFromPath(state.path.get());
  if (screen !== "account") {
    passkeyLoadGen += 1;
    state.passkeys.set(undefined);
    passkeyLoads.delete(state);
  }
  switch (screen) {
    case "device":
      if (state.session.get() === undefined) {
        renderGate(state, root);
      } else {
        renderDevice(state, root, () => {
          navigate(state, "/register");
        });
      }
      break;
    case "register":
      renderRegister(state, root);
      break;
    case "activity":
      renderActivity(state, root);
      break;
    case "account":
      renderAccount(state, root);
      break;
    default:
      renderGate(state, root);
  }
}

export function navigate(state: AppState, to: string): void {
  if (globalThis.location.pathname !== to) {
    globalThis.history.pushState(null, "", to);
  }
  state.path.set(to);
}

async function onLogout(state: AppState): Promise<void> {
  state.pending.set(true);
  state.error.set(undefined);
  logoutGen += 1;
  try {
    await req("POST", logoutUrl());
  } catch {
    /* POST /logout still signs out; do not paint FAIL_SENTENCE on the gate. */
  } finally {
    signOutLocal(state);
    await wipeDek();
  }
}

function signOutLocal(state: AppState): void {
  state.passkeys.set(undefined);
  passkeyLoads.delete(state);
  state.session.set(undefined);
  state.pending.set(false);
  state.error.set(undefined);
  navigate(state, "/");
  render(state);
}

async function wipeDek(): Promise<void> {
  try {
    const crypto = await import("./lib/crypto.ts");
    crypto.clearDek();
  } catch {
    /* session is already cleared */
  }
}

async function onContinue(state: AppState): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  state.pending.set(true);
  state.error.set(undefined);
  try {
    if (state.password.get() !== "") {
      const email = state.email.get();
      const res = await req("POST", passwordLoginUrl(), {
        email,
        password: state.password.get(),
      });
      state.password.set("");
      if (res.status !== 200) {
        state.error.set(sentenceFor(res.status));
        return;
      }
      await loadSession(state);
      if (state.session.get() === undefined) {
        if (state.error.get() === undefined) {
          state.error.set(FAIL_SENTENCE);
        }
        return;
      }
      saveRemember(email, false);
      navigate(state, state.userCode.get() === "" ? "/register" : "/device");
      return;
    }
    const res = await req("POST", startUrl(), { email: state.email.get() });
    if (res.status !== 200) {
      state.error.set(sentenceFor(res.status));
      return;
    }
    const data = res.data as { method?: string };
    const method = data.method;
    if (method === "passkey" || method === "password" || method === "either" || method === "register") {
      state.method.set(method);
    }
  } catch {
    state.error.set(FAIL_SENTENCE);
  } finally {
    state.pending.set(false);
    render(state);
  }
}

async function onPasskey(state: AppState): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  state.pending.set(true);
  state.error.set(undefined);
  try {
    const start = await req("POST", "/api/auth/passkey/login/start", {
      email: state.email.get() || undefined,
    });
    if (start.status !== 200) {
      state.error.set(sentenceFor(start.status));
      return;
    }
    const pk = coercePublicKey(start.data) as unknown as PublicKeyCredentialRequestOptions;
    const cred = await getPasskey(pk, false);
    const handle =
      typeof (start.data as { handle?: unknown }).handle === "string"
        ? (start.data as { handle: string }).handle
        : "";
    const finish = await req("POST", "/api/auth/passkey/login/finish", {
      handle,
      credential: serializeCredential(cred),
    });
    if (finish.status !== 200) {
      state.error.set(sentenceFor(finish.status));
      return;
    }
    await loadSession(state);
    if (state.session.get() === undefined) {
      if (state.error.get() === undefined) {
        state.error.set(FAIL_SENTENCE);
      }
      return;
    }
    saveRemember(state.email.get(), true);
    navigate(state, state.userCode.get() === "" ? "/register" : "/device");
  } catch {
    state.error.set(FAIL_SENTENCE);
  } finally {
    state.pending.set(false);
    render(state);
  }
}

async function onRemovePasskey(state: AppState): Promise<void> {
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
  const id = loaded[0]?.id;
  if (id === undefined || id === "") {
    return;
  }
  const gen = logoutGen;
  state.pending.set(true);
  state.error.set(undefined);
  try {
    const res = await req("DELETE", passkeyDeletePath(id));
    if (gen !== logoutGen) {
      return;
    }
    if (res.status !== 200) {
      state.error.set(sentenceFor(res.status));
      return;
    }
    state.passkeys.set(undefined);
    await loadSession(state);
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
    render(state);
  }
}

async function loadAccountPasskeys(state: AppState): Promise<void> {
  if (
    state.passkeys.get() !== undefined ||
    passkeyLoads.has(state) ||
    state.session.get() === undefined
  ) {
    return;
  }
  passkeyLoads.add(state);
  const gen = logoutGen;
  const loadGen = passkeyLoadGen;
  state.error.set(undefined);
  try {
    const res = await req("GET", passkeysUrl());
    if (
      gen !== logoutGen ||
      loadGen !== passkeyLoadGen ||
      state.session.get() === undefined ||
      screenFromPath(state.path.get()) !== "account"
    ) {
      return;
    }
    if (res.status !== 200) {
      state.error.set(sentenceFor(res.status));
      return;
    }
    state.passkeys.set(parsePasskeys(res.data));
    state.error.set(undefined);
  } catch {
    if (
      gen !== logoutGen ||
      loadGen !== passkeyLoadGen ||
      state.session.get() === undefined ||
      screenFromPath(state.path.get()) !== "account"
    ) {
      return;
    }
    state.error.set(FAIL_SENTENCE);
  } finally {
    const stale = gen !== logoutGen || loadGen !== passkeyLoadGen;
    const left =
      state.session.get() === undefined || screenFromPath(state.path.get()) !== "account";
    if (!stale && (left || state.passkeys.get() !== undefined)) {
      passkeyLoads.delete(state);
    }
    if (!stale) {
      render(state);
    }
  }
}

function parsePasskeys(v: unknown): PasskeyRow[] {
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

async function loadSession(state: AppState): Promise<void> {
  const gen = logoutGen;
  const res = await req("GET", sessionUrl());
  if (gen !== logoutGen) {
    return;
  }
  if (res.status === 401 || res.status === 403) {
    if (state.session.get() !== undefined) {
      signOutLocal(state);
      await wipeDek();
    } else {
      state.session.set(undefined);
    }
    return;
  }
  if (res.status !== 200) {
    state.error.set(sentenceFor(res.status));
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

function boot(root: HTMLElement): void {
  const { code, eph } = deviceQuery(globalThis.location.search);
  const bootPath = initialPath(globalThis.location.pathname, code);
  if (bootPath !== globalThis.location.pathname) {
    globalThis.history.replaceState(null, "", bootPath);
  }
  const state: AppState = {
    path: signal(bootPath),
    email: signal(""),
    password: signal(""),
    error: signal(undefined),
    pending: signal(false),
    session: signal(undefined),
    method: signal(undefined),
    different: signal(false),
    revealPassword: signal(false),
    userCode: signal(code),
    eph: signal(eph),
    passkeys: signal(undefined),
  };
  mounted = root;
  state.path.subscribe(() => {
    render(state);
  });
  document.addEventListener("click", (ev) => {
    if (
      ev.defaultPrevented ||
      ev.button !== 0 ||
      ev.ctrlKey ||
      ev.metaKey ||
      ev.shiftKey ||
      ev.altKey
    ) {
      return;
    }
    const t = ev.target;
    if (!(t instanceof Element)) {
      return;
    }
    const a = t.closest("a[href]");
    if (!a) {
      return;
    }
    const href = a.getAttribute("href");
    if (!href || href.startsWith("http") || href.startsWith("mailto:") || href.startsWith("#")) {
      return;
    }
    ev.preventDefault();
    navigate(state, href);
  });
  globalThis.addEventListener("popstate", () => {
    state.path.set(globalThis.location.pathname);
  });
  const mq = globalThis.matchMedia?.(`(min-width: ${BREAKPOINT_PX}px)`);
  mq?.addEventListener("change", (ev) => {
    const node = mounted?.querySelector("[data-layout]");
    if (node) {
      node.setAttribute("data-layout", ev.matches ? "list-inspector" : "list-only");
    }
  });
  void (async () => {
    try {
      await loadSession(state);
    } catch {
      state.error.set(FAIL_SENTENCE);
    } finally {
      render(state);
    }
  })();
}

const app = globalThis.document?.getElementById("app");
if (app) {
  boot(app);
}
