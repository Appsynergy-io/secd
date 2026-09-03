/** Sign-in: email, then passkey PRF or argon2id password. The first sign-in
 *  creates the vault. Paints into the bare shell; the DEK never leaves this tab. */

import {
  EMAIL_AUTOCOMPLETE,
  FAIL_SENTENCE,
  REMEMBER_DAYS,
  logoutUrl,
  passkeyLoginFinishUrl,
  passkeyLoginStartUrl,
  passkeyRegisterFinishUrl,
  passkeyRegisterStartUrl,
  passwordLoginUrl,
  passwordRegisterUrl,
  req,
  startUrl,
} from "../lib/api.ts";
import { emailOk, passwordOk, toHex, zeroizeBytes } from "../lib/crypto.ts";
import * as keyholder from "../lib/keyholder.ts";
import { asInput, el } from "../lib/dom.ts";
import { currentLogoutGen } from "../lib/gen.ts";
import type { AppState, AuthMethod, Host, SessionInfo } from "../lib/host.ts";
import {
  forgetRemember,
  loadRemember,
  saveRemember,
  sentenceFor,
  type Remembered,
} from "../lib/remember.ts";
import type { Signal } from "../lib/signal.ts";
import {
  coercePublicKey,
  createPasskey,
  getPasskey,
  prfBytes,
  serializeCredential,
} from "../lib/webauthn.ts";

export const TITLE = "Sign in";
export const SUB = "Unlocking derives the vault key in this browser. The server never sees it.";
export const WELCOME_TITLE = "Welcome back.";
export const PASSKEY_SUB = "Use your passkey.";
export const PASSWORD_SUB = "Enter your password.";
export const REGISTER_TITLE = "Create your vault";
export const REGISTER_SUB =
  "No vault exists yet. A password or a passkey derives its key in this browser. The server never sees it.";
export const PASSKEY_LABEL = "Continue with passkey";
export const PASSWORD_LABEL = "Use password";
export const SIGN_IN_LABEL = "Sign in";
export const CREATE_LABEL = "Create vault";
export const CREATE_PASSKEY_LABEL = "Create passkey instead";
export const DIFFERENT_LABEL = "Use a different account";
export const PENDING_LABEL = "Signing in…";
export const SHORT_SENTENCE = "Use at least 12 characters.";
export const MISMATCH_SENTENCE = "The passwords do not match.";
export const BRAND = "secd console";

export type GateKind =
  | "approve-only"
  | "remembered-passkey"
  | "remembered-password"
  | "cold"
  | "identity";

/** Which factor the primary button drives. */
export type GateMode = "passkey" | "password" | "register";

export type GateView = {
  kind: GateKind;
  mode: GateMode;
  showEmail: boolean;
  emailPrefill: string | undefined;
  /** The factor the button under "or" switches to; undefined hides it. */
  alternate: "passkey" | "password" | undefined;
  showUseDifferentAccount: boolean;
  userCode: string | undefined;
};

export type GateCopy = {
  title: string;
  sub: string;
  primary: string;
  secondary: string | undefined;
};

type Store = {
  root: HTMLElement;
  host: Host;
  password: Signal<string>;
  confirm: string;
  shown: boolean;
  focus: "email" | "password" | "confirm" | undefined;
};

const stores = new WeakMap<object, Store>();

function rememberIsFresh(atIso: string, nowMs: number): boolean {
  const at = Date.parse(atIso);
  return !Number.isNaN(at) && nowMs - at <= REMEMBER_DAYS * 24 * 60 * 60 * 1000;
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
  unlocked?: boolean | undefined;
}): GateView {
  if (q.session && q.unlocked) {
    return {
      kind: "approve-only",
      mode: "passkey",
      showEmail: false,
      emailPrefill: undefined,
      alternate: undefined,
      showUseDifferentAccount: false,
      userCode: q.userCode,
    };
  }
  const now = q.nowMs ?? Date.now();
  const fromSession =
    q.session && !q.useDifferentAccount
      ? {
          email: q.session.email,
          has_passkey: q.session.has_passkey,
          at: new Date(now).toISOString(),
        }
      : undefined;
  const remembered =
    fromSession ??
    (q.remember && !q.useDifferentAccount && rememberIsFresh(q.remember.at, now)
      ? q.remember
      : undefined);
  if (remembered) {
    if (remembered.has_passkey) {
      const password = Boolean(q.revealPassword);
      return {
        kind: "remembered-passkey",
        mode: password ? "password" : "passkey",
        showEmail: false,
        emailPrefill: remembered.email,
        alternate: password ? "passkey" : "password",
        showUseDifferentAccount: true,
        userCode: q.userCode,
      };
    }
    return {
      kind: "remembered-password",
      mode: "password",
      showEmail: false,
      emailPrefill: remembered.email,
      alternate: undefined,
      showUseDifferentAccount: true,
      userCode: q.userCode,
    };
  }
  const prefill = q.email ?? q.remember?.email;
  if (q.method) {
    let mode: GateMode;
    let alternate: GateView["alternate"];
    switch (q.method) {
      case "register":
        mode = "register";
        alternate = "passkey";
        break;
      case "password":
        mode = "password";
        alternate = undefined;
        break;
      case "passkey":
        mode = "passkey";
        alternate = undefined;
        break;
      default:
        mode = q.revealPassword ? "password" : "passkey";
        alternate = q.revealPassword ? "passkey" : "password";
    }
    return {
      kind: "identity",
      mode,
      showEmail: true,
      emailPrefill: prefill,
      alternate,
      showUseDifferentAccount: Boolean(q.remember),
      userCode: q.userCode,
    };
  }
  return {
    kind: "cold",
    mode: q.revealPassword ? "password" : "passkey",
    showEmail: true,
    emailPrefill: prefill,
    alternate: q.revealPassword ? "passkey" : "password",
    showUseDifferentAccount: false,
    userCode: q.userCode,
  };
}

/** Titles and button labels for a view. */
export function copyFor(view: GateView, pending: boolean): GateCopy {
  const remembered = view.kind === "remembered-passkey" || view.kind === "remembered-password";
  let title = TITLE;
  let sub = SUB;
  if (view.mode === "register") {
    title = REGISTER_TITLE;
    sub = REGISTER_SUB;
  } else if (remembered) {
    title = WELCOME_TITLE;
    sub = view.mode === "password" ? PASSWORD_SUB : PASSKEY_SUB;
  }
  let primary: string;
  if (pending) {
    primary = PENDING_LABEL;
  } else if (view.mode === "register") {
    primary = CREATE_LABEL;
  } else if (view.mode === "password") {
    primary = SIGN_IN_LABEL;
  } else {
    primary = PASSKEY_LABEL;
  }
  let secondary: string | undefined;
  if (view.alternate === "passkey") {
    secondary = view.mode === "register" ? CREATE_PASSKEY_LABEL : PASSKEY_LABEL;
  } else if (view.alternate === "password") {
    secondary = PASSWORD_LABEL;
  }
  return { title, sub, primary, secondary };
}

export function afterLoginPath(userCode: string): string {
  return userCode === "" ? "/vault" : "/device";
}

function viewOf(state: AppState): GateView {
  return resolveGate({
    session: state.session.get(),
    remember: loadRemember(),
    email: state.email.get() || undefined,
    method: state.method.get(),
    useDifferentAccount: state.different.get(),
    revealPassword: state.revealPassword.get(),
    userCode: state.userCode.get() || undefined,
    unlocked: keyholder.isUnlocked(),
  });
}

export function leaveGate(state: object): void {
  const store = stores.get(state);
  if (store) {
    store.password.set("");
    store.confirm = "";
    store.shown = false;
    stores.delete(state);
  }
  const pw = (state as { password?: Signal<string> }).password;
  if (pw !== undefined) {
    pw.set("");
  }
}

export function renderGate(state: AppState, root: HTMLElement, host: Host): void {
  if (state.session.get() !== undefined && keyholder.isUnlocked()) {
    host.navigate(afterLoginPath(state.userCode.get()));
    return;
  }
  let store = stores.get(state);
  if (!store) {
    store = {
      root,
      host,
      password: state.password,
      confirm: "",
      shown: false,
      focus: undefined,
    };
    stores.set(state, store);
  } else {
    store.root = root;
    store.host = host;
  }
  paint(state, store);
}

function passwordField(
  q: {
    id: "password" | "confirm";
    label: string;
    autocomplete: string;
    value: string;
    shown: boolean;
  },
  onInput: (value: string) => void,
  onToggle: () => void,
): HTMLElement {
  const input = el("input", {
    id: q.id,
    name: q.id,
    class: "input input-lg",
    type: q.shown ? "text" : "password",
    autocomplete: q.autocomplete,
    spellcheck: "false",
  });
  input.value = q.value;
  input.addEventListener("input", () => {
    onInput(input.value);
  });
  const toggle = el(
    "button",
    {
      type: "button",
      class: "btn btn-xs gate-pw-toggle",
      "data-action": `${q.id}-toggle`,
      "aria-label": q.shown ? `Hide ${q.label.toLowerCase()}` : `Show ${q.label.toLowerCase()}`,
      "aria-pressed": q.shown ? "true" : "false",
    },
    [q.shown ? "Hide" : "Show"],
  );
  toggle.addEventListener("click", onToggle);
  return el("div", { class: "gate-field" }, [
    el("label", { class: "label label-lg", for: q.id }, [q.label]),
    el("div", { class: "gate-pw" }, [input, toggle]),
  ]);
}

function paint(state: AppState, store: Store): void {
  const { root, host } = store;
  const prev = document.activeElement;
  const prevId =
    prev instanceof HTMLElement && prev.id !== "" && root.contains(prev) ? prev.id : undefined;
  const hadPassword = root.querySelector("#password") !== null;

  const view = viewOf(state);
  if (view.emailPrefill !== undefined && state.email.get() === "") {
    state.email.set(view.emailPrefill);
  }
  const pending = state.pending.get();
  const copy = copyFor(view, pending);
  const err = state.error.get();

  const form = el("form", {
    class: "gate-card",
    "data-kind": view.kind,
    "data-mode": view.mode,
    "aria-busy": pending ? "true" : undefined,
  });
  form.addEventListener("submit", (ev) => {
    ev.preventDefault();
    if (state.pending.get()) {
      return;
    }
    if (view.mode === "passkey") {
      void onPasskey(state, host);
    } else {
      void onContinue(state, host);
    }
  });
  form.append(
    el("div", { class: "gate-title" }, [copy.title]),
    el("div", { class: "gate-sub" }, [copy.sub]),
  );

  if (view.showEmail) {
    const email = el("input", {
      id: "email",
      name: "email",
      class: "input input-lg",
      type: "email",
      autocomplete: EMAIL_AUTOCOMPLETE,
      placeholder: "you@company.com",
      spellcheck: "false",
    });
    email.value = state.email.get();
    email.addEventListener("input", () => {
      state.email.set(email.value);
    });
    form.append(
      el("div", { class: "gate-field" }, [
        el("label", { class: "label label-lg", for: "email" }, ["Email"]),
        email,
      ]),
    );
  } else if (view.emailPrefill !== undefined) {
    form.append(
      el("div", { class: "gate-field" }, [
        el("div", { class: "label label-lg" }, ["Email"]),
        el("div", { class: "gate-account mono", "data-remembered": "" }, [view.emailPrefill]),
      ]),
    );
  }

  if (view.mode === "password" || view.mode === "register") {
    form.append(
      passwordField(
        {
          id: "password",
          label: "Password",
          autocomplete: view.mode === "register" ? "new-password" : "current-password",
          value: state.password.get(),
          shown: store.shown,
        },
        (v) => {
          state.password.set(v);
        },
        () => {
          store.shown = !store.shown;
          store.focus = "password";
          paint(state, store);
        },
      ),
    );
  }
  if (view.mode === "register") {
    form.append(
      passwordField(
        {
          id: "confirm",
          label: "Confirm password",
          autocomplete: "new-password",
          value: store.confirm,
          shown: store.shown,
        },
        (v) => {
          store.confirm = v;
        },
        () => {
          store.shown = !store.shown;
          store.focus = "confirm";
          paint(state, store);
        },
      ),
    );
  }

  if (err !== undefined) {
    form.append(el("div", { class: "alert alert-danger gate-alert", role: "alert" }, [err]));
  }

  const primary = el(
    "button",
    {
      type: "submit",
      class: "btn btn-primary btn-lg btn-block gate-primary",
      "data-action": view.mode === "passkey" ? "passkey" : "continue",
      disabled: pending,
    },
    [copy.primary],
  );
  primary.disabled = pending;
  form.append(primary);

  if (view.alternate !== undefined && copy.secondary !== undefined) {
    const alternate = view.alternate;
    const secondary = el(
      "button",
      {
        type: "button",
        class: "btn btn-lg btn-block",
        "data-action": alternate === "passkey" ? "passkey" : "password",
        disabled: pending,
      },
      [copy.secondary],
    );
    secondary.disabled = pending;
    secondary.addEventListener("click", () => {
      if (state.pending.get()) {
        return;
      }
      if (alternate === "passkey") {
        void onPasskey(state, host);
        return;
      }
      state.revealPassword.set(true);
      store.focus = "password";
      paint(state, store);
    });
    form.append(el("div", { class: "divider" }, ["or"]), secondary);
  }

  if (view.showUseDifferentAccount) {
    const different = el(
      "button",
      { type: "button", class: "gate-link", "data-action": "different" },
      [DIFFERENT_LABEL],
    );
    different.addEventListener("click", () => {
      forgetRemember();
      state.different.set(true);
      state.method.set(undefined);
      state.revealPassword.set(false);
      state.password.set("");
      state.email.set("");
      state.error.set(undefined);
      store.confirm = "";
      store.focus = "email";
      paint(state, store);
    });
    form.append(el("div", { class: "gate-links" }, [different]));
  }

  const page = el("div", { class: "gate", "data-page": "gate" }, [
    el("div", { class: "gate-wrap" }, [
      el("div", { class: "gate-brand" }, [
        el("div", { class: "brand-mark brand-mark-lg", "aria-hidden": "true" }, ["s"]),
        el("div", {}, [BRAND]),
      ]),
      form,
      el("div", { class: "gate-foot" }, [
        `LAN only · ${globalThis.location?.host ?? ""} · TLS 1.3`,
      ]),
    ]),
  ]);
  root.replaceChildren(page);

  const hint = store.focus;
  store.focus = undefined;
  let targetId: string | undefined;
  if (hint !== undefined) {
    targetId = hint;
  } else if ((view.mode === "password" || view.mode === "register") && !hadPassword) {
    targetId = "password";
  } else if (prevId !== undefined) {
    targetId = prevId;
  } else if (view.showEmail && state.email.get() === "") {
    targetId = "email";
  }
  const target = targetId === undefined ? null : asInput(page.querySelector(`#${targetId}`));
  if (target !== null && typeof target.focus === "function") {
    target.focus();
  }
}

function repaint(state: AppState): void {
  const store = stores.get(state);
  if (store) {
    paint(state, store);
  }
}

async function rejectUnlockedSession(state: AppState): Promise<void> {
  try {
    await req("POST", logoutUrl());
  } catch {
    /* cookie drop is best-effort; the tab must not stay unlocked */
  }
  void keyholder.lock();
  state.session.set(undefined);
  state.error.set(FAIL_SENTENCE);
}

async function resolveMethod(state: AppState, email: string): Promise<AuthMethod | undefined> {
  const known = state.method.get();
  if (known !== undefined) {
    return known;
  }
  const res = await req("POST", startUrl(), { email });
  if (res.status !== 200) {
    state.error.set(sentenceFor(res.status));
    return undefined;
  }
  const method = (res.data as { method?: unknown }).method;
  if (method === "passkey" || method === "password" || method === "either" || method === "register") {
    state.method.set(method);
    return method;
  }
  state.error.set(FAIL_SENTENCE);
  return undefined;
}

/** After the DEK is set: confirm the cookie session, remember the account, leave the gate. */
async function finishUnlock(state: AppState, host: Host, email: string, viaPasskey: boolean): Promise<boolean> {
  await host.loadSession();
  if (state.session.get() === undefined || !keyholder.isUnlocked()) {
    await rejectUnlockedSession(state);
    return false;
  }
  saveRemember(email, viaPasskey);
  host.navigate(afterLoginPath(state.userCode.get()));
  return true;
}

async function passwordRegister(state: AppState, email: string, password: string): Promise<boolean> {
  if (!(await keyholder.create())) {
    state.error.set(FAIL_SENTENCE);
    return false;
  }
  const wrap = await keyholder.wrapPassword(password);
  if (wrap === undefined) {
    await keyholder.lock();
    state.error.set(FAIL_SENTENCE);
    return false;
  }
  const res = await req("POST", passwordRegisterUrl(), { email, password, wrap });
  if (res.status !== 200) {
    await keyholder.lock();
    state.error.set(sentenceFor(res.status));
    return false;
  }
  return true;
}

async function passwordLogin(state: AppState, email: string, password: string): Promise<boolean> {
  const res = await req("POST", passwordLoginUrl(), { email, password });
  if (res.status !== 200) {
    state.error.set(sentenceFor(res.status));
    return false;
  }
  if (!(await keyholder.unlock(res.data, { password }))) {
    await rejectUnlockedSession(state);
    return false;
  }
  return true;
}

/** The primary button in password or register mode; resolves the method first when unknown. */
export async function onContinue(state: AppState, host: Host): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  const email = emailOk(state.email.get());
  if (email === undefined) {
    state.error.set(FAIL_SENTENCE);
    repaint(state);
    return;
  }
  const view = viewOf(state);
  const gen = currentLogoutGen();
  state.pending.set(true);
  state.error.set(undefined);
  repaint(state);
  const store = stores.get(state);
  try {
    void keyholder.lock();
    const knownPassword =
      view.kind === "remembered-password" ||
      (view.kind === "remembered-passkey" && view.mode === "password");
    const method = knownPassword ? "password" : await resolveMethod(state, email);
    if (gen !== currentLogoutGen()) {
      return;
    }
    if (method === undefined) {
      return;
    }
    const password = state.password.get();
    if (method === "register") {
      if (view.mode !== "register") {
        return;
      }
      if (!passwordOk(password)) {
        state.error.set(password === "" ? FAIL_SENTENCE : SHORT_SENTENCE);
        return;
      }
      if (store === undefined || store.confirm !== password) {
        state.error.set(MISMATCH_SENTENCE);
        return;
      }
      const ok = await passwordRegister(state, email, password);
      state.password.set("");
      store.confirm = "";
      if (!ok || gen !== currentLogoutGen()) {
        return;
      }
      await finishUnlock(state, host, email, false);
      return;
    }
    if (method === "passkey") {
      state.password.set("");
      state.revealPassword.set(false);
      return;
    }
    if (password === "") {
      state.error.set(FAIL_SENTENCE);
      return;
    }
    const ok = await passwordLogin(state, email, password);
    state.password.set("");
    if (!ok || gen !== currentLogoutGen()) {
      return;
    }
    await finishUnlock(state, host, email, false);
  } catch {
    state.password.set("");
    state.error.set(FAIL_SENTENCE);
  } finally {
    if (gen === currentLogoutGen()) {
      state.pending.set(false);
      repaint(state);
    }
  }
}

async function passkeyRegister(state: AppState, email: string): Promise<boolean> {
  const start = await req("POST", passkeyRegisterStartUrl(), { email });
  if (start.status !== 200) {
    state.error.set(sentenceFor(start.status));
    return false;
  }
  const pk = coercePublicKey(start.data) as unknown as PublicKeyCredentialCreationOptions;
  const cred = await createPasskey(pk);
  const prf = prfBytes(cred);
  if (prf === undefined) {
    state.error.set(FAIL_SENTENCE);
    return false;
  }
  const handle =
    typeof (start.data as { handle?: unknown }).handle === "string"
      ? (start.data as { handle: string }).handle
      : "";
  try {
    if (!(await keyholder.create())) {
      state.error.set(FAIL_SENTENCE);
      return false;
    }
    const wrap = await keyholder.wrapPasskey(toHex(prf), toHex(new Uint8Array(cred.rawId)));
    if (wrap === undefined) {
      await keyholder.lock();
      state.error.set(FAIL_SENTENCE);
      return false;
    }
    const finish = await req("POST", passkeyRegisterFinishUrl(), {
      handle,
      credential: serializeCredential(cred),
      wrap,
      email,
    });
    if (finish.status !== 200) {
      await keyholder.lock();
      state.error.set(sentenceFor(finish.status));
      return false;
    }
    return true;
  } finally {
    zeroizeBytes(prf);
  }
}

async function passkeyLogin(state: AppState, email: string): Promise<boolean> {
  const start = await req("POST", passkeyLoginStartUrl(), { email });
  if (start.status !== 200) {
    state.error.set(sentenceFor(start.status));
    return false;
  }
  const pk = coercePublicKey(start.data) as unknown as PublicKeyCredentialRequestOptions;
  const cred = await getPasskey(pk, false);
  const prf = prfBytes(cred);
  if (prf === undefined) {
    state.error.set(FAIL_SENTENCE);
    return false;
  }
  try {
    const handle =
      typeof (start.data as { handle?: unknown }).handle === "string"
        ? (start.data as { handle: string }).handle
        : "";
    const finish = await req("POST", passkeyLoginFinishUrl(), {
      handle,
      credential: serializeCredential(cred),
      email,
    });
    if (finish.status !== 200) {
      state.error.set(sentenceFor(finish.status));
      return false;
    }
    if (!(await keyholder.unlock(finish.data, { prf: toHex(prf) }))) {
      await rejectUnlockedSession(state);
      return false;
    }
    return true;
  } finally {
    zeroizeBytes(prf);
  }
}

/** The passkey button: registers the first passkey when the vault is new, else signs in. */
export async function onPasskey(state: AppState, host: Host): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  const email = emailOk(state.email.get());
  if (email === undefined) {
    state.error.set(FAIL_SENTENCE);
    repaint(state);
    return;
  }
  const gen = currentLogoutGen();
  state.password.set("");
  const store = stores.get(state);
  if (store) {
    store.confirm = "";
  }
  state.pending.set(true);
  state.error.set(undefined);
  repaint(state);
  try {
    void keyholder.lock();
    const remembered = viewOf(state).kind === "remembered-passkey";
    const method = remembered ? "passkey" : await resolveMethod(state, email);
    if (gen !== currentLogoutGen() || method === undefined) {
      return;
    }
    if (method === "password") {
      state.revealPassword.set(true);
      if (store) {
        store.focus = "password";
      }
      return;
    }
    const ok =
      method === "register" ? await passkeyRegister(state, email) : await passkeyLogin(state, email);
    if (!ok || gen !== currentLogoutGen()) {
      return;
    }
    await finishUnlock(state, host, email, true);
  } catch {
    state.error.set(FAIL_SENTENCE);
  } finally {
    if (gen === currentLogoutGen()) {
      state.pending.set(false);
      repaint(state);
    }
  }
}
