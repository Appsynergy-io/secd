/** Sign-in: email, then passkey PRF or argon2id password. */

import {
  EMAIL_AUTOCOMPLETE,
  FAIL_SENTENCE,
  LAST_KEY,
  RATE_SENTENCE,
  REMEMBER_DAYS,
  passkeyLoginFinishUrl,
  passkeyLoginStartUrl,
  passkeyRegisterFinishUrl,
  passkeyRegisterStartUrl,
  passwordLoginUrl,
  passwordRegisterUrl,
  req,
  startUrl,
} from "../lib/api.ts";
import { copyText } from "../lib/clipboard.ts";
import type { Signal } from "../lib/signal.ts";
import {
  coercePublicKey,
  createPasskey,
  getPasskey,
  prfBytes,
  serializeCredential,
} from "../lib/webauthn.ts";

export type AuthMethod = "register" | "passkey" | "password" | "either";

export type SessionInfo = {
  email: string;
  has_passkey: boolean;
  has_password: boolean;
  session_id: string;
};

export type Remembered = {
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

export type GateState = {
  path: Signal<string>;
  email: Signal<string>;
  password: Signal<string>;
  error: Signal<string | undefined>;
  pending: Signal<boolean>;
  session: Signal<SessionInfo | undefined>;
  method: Signal<AuthMethod | undefined>;
  different: Signal<boolean>;
  revealPassword: Signal<boolean>;
  userCode: Signal<string>;
};

export type GateHost = {
  navigate(to: string): void;
  redraw(): void;
  loadSession(): Promise<void>;
};

export const CLIP_FAIL_SENTENCE =
  "The browser refused the clipboard. Select the value and copy it.";

const clipFails = new WeakMap<object, boolean>();
const focusHints = new WeakMap<object, "email" | "password" | "copy">();

function rememberIsFresh(atIso: string, nowMs: number): boolean {
  const at = Date.parse(atIso);
  if (Number.isNaN(at)) {
    return false;
  }
  return nowMs - at <= REMEMBER_DAYS * 24 * 60 * 60 * 1000;
}

export function loadRemember(): Remembered | undefined {
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

export function saveRemember(email: string, hasPasskey: boolean): void {
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
    /* ignore quota / private mode */
  }
}

export function sentenceFor(status: number): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  return FAIL_SENTENCE;
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
      if (k === "disabled" && "disabled" in node) {
        (node as HTMLButtonElement).disabled = true;
      }
      if (k === "readonly" && "readOnly" in node) {
        (node as HTMLInputElement).readOnly = true;
      }
      continue;
    }
    node.setAttribute(k, v);
    if (k === "value" && (tag === "input" || tag === "textarea")) {
      (node as HTMLInputElement).value = v;
    }
  }
  for (const child of children) {
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

function disable(btn: HTMLButtonElement, on: boolean): void {
  btn.disabled = on;
  if (on) {
    btn.setAttribute("disabled", "");
    return;
  }
  if (typeof btn.removeAttribute === "function") {
    btn.removeAttribute("disabled");
  }
}

function afterLoginPath(userCode: string): string {
  return userCode === "" ? "/register" : "/device";
}

function copyControl(value: string, onFail: () => void): HTMLElement {
  const row = el("div", { class: "row" });
  const field = el("input", {
    class: "mono",
    readonly: true,
    autocomplete: "off",
    "data-copy": "value",
    "aria-label": "Error text",
    value,
  });
  const btn = el(
    "button",
    {
      type: "button",
      class: "secondary",
      "data-action": "copy",
      "aria-label": "Copy error",
    },
    ["Copy"],
  );
  let copying = false;
  btn.addEventListener("click", () => {
    void (async () => {
      if (copying) {
        return;
      }
      copying = true;
      const ok = await copyText(value);
      if (ok) {
        btn.textContent = "Copied";
      } else {
        btn.textContent = "Copy";
        onFail();
      }
      copying = false;
    })();
  });
  row.append(field, btn);
  return row;
}

function viewOf(state: GateState): GateView {
  return resolveGate({
    session: state.session.get(),
    remember: loadRemember(),
    email: state.email.get() || undefined,
    method: state.method.get(),
    useDifferentAccount: state.different.get(),
    revealPassword: state.revealPassword.get(),
    userCode: state.userCode.get() || undefined,
  });
}

function titleFor(kind: GateKind): string {
  if (kind === "remembered-passkey" || kind === "remembered-password") {
    return "Welcome back.";
  }
  if (kind === "identity") {
    return "Continue.";
  }
  return "Sign in.";
}

function subFor(kind: GateKind): string {
  switch (kind) {
    case "cold":
      return "Enter your email to continue.";
    case "remembered-passkey":
      return "Use your passkey.";
    case "remembered-password":
      return "Enter your password.";
    case "identity":
      return "Choose a factor.";
    default:
      return "";
  }
}

function reasonFor(q: {
  pending: boolean;
  showEmail: boolean;
  showPassword: boolean;
  email: string;
  password: string;
}): string | undefined {
  if (q.pending) {
    return "Signing in.";
  }
  if (q.showEmail && q.email === "") {
    return "Enter your email.";
  }
  if (q.showPassword && q.password === "") {
    return "Enter your password.";
  }
  return undefined;
}

export function renderGate(state: GateState, root: HTMLElement, host: GateHost): void {
  if (state.session.get() !== undefined && state.path.get() === "/") {
    host.navigate(afterLoginPath(state.userCode.get()));
    return;
  }

  const prevActive = document.activeElement;
  const prevId =
    prevActive instanceof HTMLElement &&
    prevActive.id !== "" &&
    root.contains(prevActive)
      ? prevActive.id
      : undefined;
  const hadPassword = root.querySelector("#password") !== null;
  const hadEmail = root.querySelector("#email") !== null;

  const remember = loadRemember();
  if (state.email.get() === "" && remember?.email) {
    state.email.set(remember.email);
  }
  const view = resolveGate({
    session: state.session.get(),
    remember,
    email: state.email.get() || undefined,
    method: state.method.get(),
    useDifferentAccount: state.different.get(),
    revealPassword: state.revealPassword.get(),
    userCode: state.userCode.get() || undefined,
  });
  if (view.emailPrefill && state.email.get() === "") {
    state.email.set(view.emailPrefill);
  }

  const pending = state.pending.get();
  const reason = reasonFor({
    pending,
    showEmail: view.showEmail,
    showPassword: view.showPassword,
    email: state.email.get(),
    password: state.password.get(),
  });
  const form = el("form", {
    class: "secd-auth-form",
    "aria-busy": pending ? "true" : undefined,
  });
  form.addEventListener("submit", (ev) => {
    ev.preventDefault();
    if (pending) {
      return;
    }
    if (view.showEmail || view.showPassword) {
      void onContinue(state, host);
      return;
    }
    if (view.showPasskey) {
      void onPasskey(state, host);
    }
  });

  if (view.emailPrefill && !view.showEmail && view.kind !== "approve-only") {
    form.append(
      el("div", { class: "mono", "data-remembered": view.emailPrefill }, [view.emailPrefill]),
    );
  }
  if (view.showEmail) {
    const input = el("input", {
      id: "email",
      type: "email",
      name: "email",
      class: "mono",
      autocomplete: view.emailAutocomplete ?? EMAIL_AUTOCOMPLETE,
      required: true,
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
      required: true,
    });
    input.value = state.password.get();
    input.addEventListener("input", () => {
      state.password.set(input.value);
    });
    form.append(el("label", { for: "password" }, ["Password"]), input);
  }

  if (view.showPasskey) {
    const pk = el(
      "button",
      {
        type: view.showEmail || view.showPassword ? "button" : "submit",
        "data-action": "passkey",
      },
      ["Use a passkey"],
    ) as HTMLButtonElement;
    disable(pk, pending);
    pk.addEventListener("click", (ev) => {
      if (pk.type === "submit") {
        return;
      }
      ev.preventDefault();
      void onPasskey(state, host);
    });
    form.append(pk);
  }

  if (view.showEmail || view.showPassword) {
    const cont = el("button", { type: "submit" }, ["Continue"]) as HTMLButtonElement;
    const blocked =
      pending ||
      (view.showEmail && state.email.get() === "") ||
      (view.showPassword && state.password.get() === "");
    disable(cont, blocked);
    form.append(cont);
  }

  if (view.showUsePasswordInstead) {
    const pw = el(
      "button",
      { type: "button", class: "secondary", "data-action": "password" },
      ["Use a password instead"],
    );
    pw.addEventListener("click", () => {
      state.revealPassword.set(true);
      focusHints.set(state, "password");
      host.redraw();
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
      state.method.set(undefined);
      state.password.set("");
      focusHints.set(state, "email");
      host.redraw();
    });
    form.append(diff);
  }

  const err = state.error.get();
  if (err === undefined) {
    clipFails.delete(state);
  }
  const clipFail = clipFails.get(state) === true;
  const sub = subFor(view.kind);
  const page = el(
    "div",
    {
      class: "app",
      "data-page": "gate",
      "data-kind": view.kind,
      "data-state": pending ? "loading" : err ? "error" : reason ? "empty" : "ready",
    },
    [el("h1", {}, [titleFor(view.kind)]), sub ? el("p", {}, [sub]) : "", form],
  );
  if (reason) {
    page.append(el("p", { "data-reason": "" }, [reason]));
  }
  if (err) {
    page.append(
      el("p", { class: "error", role: "alert" }, [clipFail ? CLIP_FAIL_SENTENCE : err]),
      copyControl(err, () => {
        clipFails.set(state, true);
        focusHints.set(state, "copy");
        host.redraw();
      }),
    );
  }
  root.replaceChildren(page);

  const hint = focusHints.get(state);
  focusHints.delete(state);
  const passwordJustRevealed = view.showPassword && !hadPassword;
  const emailJustRevealed = view.showEmail && !hadEmail;
  let target: HTMLElement | null = null;
  if (passwordJustRevealed || hint === "password") {
    target = page.querySelector("#password");
  } else if (hint === "email" || emailJustRevealed) {
    target = page.querySelector("#email");
  } else if (hint === "copy" || clipFail) {
    target = page.querySelector('[data-copy="value"]');
  } else if (prevId !== undefined) {
    target = page.querySelector(`#${prevId}`);
  }
  if (target !== null && typeof target.focus === "function") {
    target.focus();
  }
}

export async function onContinue(state: GateState, host: GateHost): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  state.pending.set(true);
  state.error.set(undefined);
  try {
    const crypto = await import("../lib/crypto.ts");
    crypto.clearDek();
    const view = viewOf(state);
    const email = state.email.get();
    const password = state.password.get();
    if (view.showPassword && password !== "") {
      const isRegister = state.method.get() === "register";
      if (isRegister) {
        const fresh = crypto.mintDek();
        try {
          let body: Record<string, unknown>;
          try {
            const wrap = crypto.wrapPassword(fresh, new TextEncoder().encode(password));
            body = { email, password, wrap: crypto.wrapToJson(wrap) };
          } catch {
            state.password.set("");
            state.error.set(FAIL_SENTENCE);
            return;
          }
          const res = await req("POST", passwordRegisterUrl(), body);
          state.password.set("");
          if (res.status !== 200) {
            state.error.set(sentenceFor(res.status));
            return;
          }
          crypto.setDek(fresh);
        } finally {
          crypto.zeroizeBytes(fresh);
        }
      } else {
        const res = await req("POST", passwordLoginUrl(), { email, password });
        state.password.set("");
        if (res.status !== 200) {
          state.error.set(sentenceFor(res.status));
          return;
        }
        const opened = crypto.unwrapAny(
          crypto.wrapsFromJson(res.data),
          new TextEncoder().encode(password),
          undefined,
        );
        if (opened !== undefined) {
          crypto.setDek(opened);
        }
      }
      await host.loadSession();
      if (state.session.get() === undefined) {
        crypto.clearDek();
        if (state.error.get() === undefined) {
          state.error.set(FAIL_SENTENCE);
        }
        return;
      }
      saveRemember(email, false);
      host.navigate(afterLoginPath(state.userCode.get()));
      return;
    }
    const res = await req("POST", startUrl(), { email });
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
    host.redraw();
  }
}

export async function onPasskey(state: GateState, host: GateHost): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  state.pending.set(true);
  state.error.set(undefined);
  try {
    const crypto = await import("../lib/crypto.ts");
    crypto.clearDek();
    const email = state.email.get();
    if (state.method.get() === "register") {
      const start = await req("POST", passkeyRegisterStartUrl(), { email });
      if (start.status !== 200) {
        state.error.set(sentenceFor(start.status));
        return;
      }
      const pk = coercePublicKey(start.data) as unknown as PublicKeyCredentialCreationOptions;
      const cred = await createPasskey(pk);
      const prf = prfBytes(cred);
      if (prf === undefined) {
        state.error.set(FAIL_SENTENCE);
        return;
      }
      const handle =
        typeof (start.data as { handle?: unknown }).handle === "string"
          ? (start.data as { handle: string }).handle
          : "";
      const fresh = crypto.mintDek();
      try {
        const wrap = crypto.wrapPasskey(fresh, prf, crypto.toHex(new Uint8Array(cred.rawId)));
        const finish = await req("POST", passkeyRegisterFinishUrl(), {
          handle,
          credential: serializeCredential(cred),
          wrap: crypto.wrapToJson(wrap),
          email,
        });
        if (finish.status !== 200) {
          state.error.set(sentenceFor(finish.status));
          return;
        }
        crypto.setDek(fresh);
      } finally {
        crypto.zeroizeBytes(fresh);
      }
    } else {
      const start = await req("POST", passkeyLoginStartUrl(), {
        email: email || undefined,
      });
      if (start.status !== 200) {
        state.error.set(sentenceFor(start.status));
        return;
      }
      const pk = coercePublicKey(start.data) as unknown as PublicKeyCredentialRequestOptions;
      const cred = await getPasskey(pk, false);
      const prf = prfBytes(cred);
      if (prf === undefined) {
        state.error.set(FAIL_SENTENCE);
        return;
      }
      const handle =
        typeof (start.data as { handle?: unknown }).handle === "string"
          ? (start.data as { handle: string }).handle
          : "";
      const finish = await req("POST", passkeyLoginFinishUrl(), {
        handle,
        credential: serializeCredential(cred),
      });
      if (finish.status !== 200) {
        state.error.set(sentenceFor(finish.status));
        return;
      }
      const opened = crypto.unwrapAny(crypto.wrapsFromJson(finish.data), undefined, prf);
      if (opened !== undefined) {
        crypto.setDek(opened);
      }
    }
    await host.loadSession();
    if (state.session.get() === undefined) {
      crypto.clearDek();
      if (state.error.get() === undefined) {
        state.error.set(FAIL_SENTENCE);
      }
      return;
    }
    saveRemember(email, true);
    host.navigate(afterLoginPath(state.userCode.get()));
  } catch {
    state.error.set(FAIL_SENTENCE);
  } finally {
    state.pending.set(false);
    host.redraw();
  }
}
