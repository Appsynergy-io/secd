/** CLI approval: seal this tab's DEK to the ephemeral X25519 key in the link. */

import {
  BREAKPOINT_PX,
  FAIL_SENTENCE,
  NO_DEK_SENTENCE,
  NO_EPH_SENTENCE,
  RATE_SENTENCE,
  deviceApproveUrl,
  deviceQuery,
  errorMessage,
  layoutMode,
  req,
} from "../lib/api.ts";
import { copyText } from "../lib/clipboard.ts";
import {
  fromHex,
  getDek,
  sealDekToEph,
  zeroizeBytes,
} from "../lib/crypto.ts";
import { signal, type Signal } from "../lib/signal.ts";

export const NO_SESSION_SENTENCE = "Sign in from this browser first.";
export const CLIP_FAIL_SENTENCE =
  "The browser refused the clipboard. Select the value and copy it.";

export type DeviceSession = {
  email: string;
  session_id: string;
};

export type DeviceState = {
  userCode: Signal<string>;
  eph: Signal<string>;
  session: { get(): DeviceSession | undefined };
  error: Signal<string | undefined>;
  pending: Signal<boolean>;
};

const copiedAt = new WeakMap<object, Signal<boolean>>();
const seeded = new WeakSet<object>();
const focusHints = new WeakMap<object, "copy" | "code">();

function copiedOf(state: object): Signal<boolean> {
  let s = copiedAt.get(state);
  if (s === undefined) {
    s = signal(false);
    copiedAt.set(state, s);
  }
  return s;
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

function asButton(node: Element | null): HTMLButtonElement | null {
  return node !== null && node.tagName === "BUTTON" ? (node as HTMLButtonElement) : null;
}

function asInput(node: Element | null): HTMLInputElement | null {
  return node !== null && node.tagName === "INPUT" ? (node as HTMLInputElement) : null;
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

function isErrorReason(node: Element): boolean {
  const cls = node.getAttribute("class") ?? "";
  return cls.split(/\s+/).includes("error");
}

function refreshDeviceControls(page: HTMLElement, state: DeviceState): void {
  const code = state.userCode.get();
  const eph = state.eph.get();
  const session = state.session.get();
  const pending = state.pending.get();
  const err = state.error.get();
  const copied = copiedOf(state);
  const reason = deviceDisabledReason({
    userCode: code,
    eph,
    session,
    hasDek: getDek() !== undefined,
  });
  const disabled = pending || reason !== undefined;
  const kind = viewKind({ pending, error: err, userCode: code, eph });
  page.setAttribute("data-state", kind);

  const codeView = page.querySelector("[data-device]");
  if (codeView !== null) {
    codeView.textContent = code === "" ? "No device code." : code;
  }

  const copy = asButton(page.querySelector('[data-action="copy"]'));
  if (copy !== null) {
    copy.textContent = copied.get() && code !== "" ? "Copied" : "Copy";
    disable(copy, code === "" || pending);
  }

  const approve = asButton(page.querySelector('[data-action="approve"]'));
  if (approve !== null) {
    disable(approve, disabled);
  }

  const firstReason = page.querySelector("[data-reason]");
  const reasonOnly =
    firstReason !== null && !isErrorReason(firstReason) ? firstReason : null;
  const showReason = kind !== "error" ? reason : undefined;
  if (showReason !== undefined) {
    if (reasonOnly !== null) {
      reasonOnly.textContent = showReason;
    } else {
      const node = el("p", { "data-reason": "" }, [showReason]);
      if (firstReason !== null) {
        page.insertBefore(node, firstReason);
      } else {
        page.append(node);
      }
    }
  } else if (reasonOnly !== null) {
    reasonOnly.remove();
  }
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

export function seedDeviceQuery(state: DeviceState, search: string): void {
  if (seeded.has(state)) {
    return;
  }
  seeded.add(state);
  const q = deviceQuery(search);
  if (state.userCode.get() === "" && q.code !== "") {
    state.userCode.set(q.code);
  }
  if (state.eph.get() === "" && q.eph !== "") {
    state.eph.set(q.eph);
  }
}

export function deviceDisabledReason(q: {
  userCode: string;
  eph: string;
  session: DeviceSession | undefined;
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

function viewKind(q: {
  pending: boolean;
  error: string | undefined;
  userCode: string;
  eph: string;
}): "loading" | "empty" | "error" | "ready" {
  if (q.pending) {
    return "loading";
  }
  if (q.error !== undefined) {
    return "error";
  }
  if (q.userCode === "" || !ephOk(q.eph)) {
    return "empty";
  }
  return "ready";
}

function failSentence(status: number, data: unknown): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  return errorMessage(data) ?? FAIL_SENTENCE;
}

function stillOnDevice(root: HTMLElement): boolean {
  return root.querySelector('[data-page="device"]') !== null;
}

export function renderDevice(
  state: DeviceState,
  root: HTMLElement,
  onApproved?: () => void,
): void {
  seedDeviceQuery(state, globalThis.location?.search ?? "");
  const prevInput = asInput(root.querySelector("#user_code"));
  const prevApprove = asButton(root.querySelector('[data-action="approve"]'));
  const hadCodeFocus = prevInput !== null && document.activeElement === prevInput;
  const hadApproveFocus =
    prevApprove !== null && document.activeElement === prevApprove;
  const selStart = prevInput !== null ? prevInput.selectionStart : null;
  const selEnd = prevInput !== null ? prevInput.selectionEnd : null;
  const code = state.userCode.get();
  const eph = state.eph.get();
  const session = state.session.get();
  const pending = state.pending.get();
  const err = state.error.get();
  const copied = copiedOf(state);
  const reason = deviceDisabledReason({
    userCode: code,
    eph,
    session,
    hasDek: getDek() !== undefined,
  });
  const disabled = pending || reason !== undefined;
  const kind = viewKind({ pending, error: err, userCode: code, eph });
  const width =
    typeof globalThis.innerWidth === "number" &&
    Number.isFinite(globalThis.innerWidth) &&
    globalThis.innerWidth > 0
      ? globalThis.innerWidth
      : BREAKPOINT_PX;
  const layout = layoutMode(width);

  const codeView = el("p", {
    class: "mono",
    "data-device": "",
    "data-field": "device",
  });
  if (code === "") {
    codeView.textContent = "No device code.";
  } else {
    codeView.textContent = code;
  }

  const copy = el(
    "button",
    {
      type: "button",
      class: "secondary",
      "data-action": "copy",
      disabled: code === "" || pending ? true : undefined,
    },
    [copied.get() && code !== "" ? "Copied" : "Copy"],
  );
  copy.addEventListener("click", () => {
    void onCopy(state, root, onApproved);
  });

  const sessionCard = sessionCardEl(session);
  const form = el("form", { class: "secd-auth-form" });
  form.addEventListener("submit", (ev) => {
    ev.preventDefault();
    void onApprove(state, root, onApproved);
  });
  const input = el("input", {
    id: "user_code",
    class: "mono",
    name: "user_code",
    autocomplete: "off",
    value: code,
  });
  input.addEventListener("input", () => {
    copied.set(false);
    state.userCode.set(input.value);
    const pageEl = input.closest("[data-page]");
    if (pageEl !== null) {
      refreshDeviceControls(pageEl as HTMLElement, state);
    }
  });
  const approve = el(
    "button",
    {
      type: "submit",
      "data-action": "approve",
      disabled: disabled ? true : undefined,
    },
    ["Approve"],
  );
  form.append(el("label", { for: "user_code" }, ["Device code"]), input, approve);

  const page = el(
    "div",
    {
      class: "app",
      "data-page": "device",
      "data-layout": layout,
      "data-state": kind,
    },
    [
      el("h1", {}, ["Approve this machine."]),
      el("div", { class: "workspace" }, [
        el("div", { class: "card" }, [
          el("h2", {}, ["Device"]),
          codeView,
          copy,
        ]),
        sessionCard,
      ]),
      form,
      reason && kind !== "error" ? el("p", { "data-reason": "" }, [reason]) : "",
      err ? el("p", { class: "error", "data-reason": "" }, [err]) : "",
    ],
  );
  root.replaceChildren(page);
  const restored = asInput(page.querySelector("#user_code"));
  if (restored !== null) {
    restored.value = code;
    if (
      typeof selStart === "number" &&
      typeof selEnd === "number" &&
      typeof restored.setSelectionRange === "function"
    ) {
      restored.setSelectionRange(selStart, selEnd);
    }
  }
  const hint = focusHints.get(state);
  focusHints.delete(state);
  const found =
    hint === "code"
      ? page.querySelector("#user_code")
      : hint === "copy"
        ? page.querySelector('[data-action="copy"]')
        : hadApproveFocus
          ? !approve.disabled
            ? approve
            : page.querySelector("#user_code")
          : hadCodeFocus
            ? page.querySelector("#user_code")
            : null;
  if (found !== null && typeof (found as HTMLElement).focus === "function") {
    (found as HTMLElement).focus();
  }
  if (hint === "code") {
    const codeInput = asInput(found) ?? restored;
    if (codeInput !== null) {
      if (typeof codeInput.select === "function") {
        codeInput.select();
      } else if (typeof codeInput.setSelectionRange === "function") {
        codeInput.setSelectionRange(0, codeInput.value.length);
      }
    }
  }
}

function sessionCardEl(session: DeviceSession | undefined): HTMLElement {
  const body: Array<Node | string> = [el("h2", {}, ["Session"])];
  if (session === undefined) {
    body.push(el("p", { "data-session": "empty" }, ["No session."]));
  } else {
    body.push(
      el("p", { class: "mono", "data-session-email": "", "data-field": "session" }, [
        session.email,
      ]),
      el("p", { class: "mono", "data-session-id": "", "data-field": "session" }, [
        session.session_id,
      ]),
    );
  }
  return el("div", { class: "card", "data-session": "" }, body);
}

async function onCopy(
  state: DeviceState,
  root: HTMLElement,
  onApproved?: () => void,
): Promise<void> {
  const code = state.userCode.get();
  if (code === "" || state.pending.get()) {
    return;
  }
  const ok = await copyText(code);
  if (!stillOnDevice(root)) {
    return;
  }
  if (state.pending.get()) {
    copiedOf(state).set(ok);
    const copy = asButton(root.querySelector('[data-action="copy"]'));
    if (copy !== null) {
      copy.textContent = ok && code !== "" ? "Copied" : "Copy";
    }
    return;
  }
  if (ok) {
    copiedOf(state).set(true);
    if (state.error.get() === CLIP_FAIL_SENTENCE) {
      state.error.set(undefined);
    }
    focusHints.set(state, "copy");
  } else {
    copiedOf(state).set(false);
    const err = state.error.get();
    if (err === undefined || err === CLIP_FAIL_SENTENCE) {
      state.error.set(CLIP_FAIL_SENTENCE);
    }
    focusHints.set(state, "code");
  }
  renderDevice(state, root, onApproved);
}

async function onApprove(
  state: DeviceState,
  root: HTMLElement,
  onApproved?: () => void,
): Promise<void> {
  if (state.pending.get()) {
    return;
  }
  const reason = deviceDisabledReason({
    userCode: state.userCode.get(),
    eph: state.eph.get(),
    session: state.session.get(),
    hasDek: getDek() !== undefined,
  });
  if (reason !== undefined) {
    return;
  }
  const dek = getDek();
  if (dek === undefined) {
    state.error.set(NO_DEK_SENTENCE);
    renderDevice(state, root, onApproved);
    return;
  }
  let ephBytes: Uint8Array;
  try {
    ephBytes = fromHex(state.eph.get());
  } catch {
    state.error.set(NO_EPH_SENTENCE);
    renderDevice(state, root, onApproved);
    return;
  }
  if (ephBytes.length !== 32) {
    zeroizeBytes(ephBytes);
    state.error.set(NO_EPH_SENTENCE);
    renderDevice(state, root, onApproved);
    return;
  }
  state.pending.set(true);
  state.error.set(undefined);
  renderDevice(state, root, onApproved);
  try {
    let sealed: { alg: string; eph_pub: string; blob: string };
    try {
      sealed = sealDekToEph(dek, ephBytes);
    } catch {
      state.error.set(FAIL_SENTENCE);
      return;
    }
    const res = await req("POST", deviceApproveUrl(), {
      user_code: state.userCode.get(),
      sealed_dek: sealed,
    });
    if (!stillOnDevice(root)) {
      return;
    }
    if (res.status === 200) {
      state.error.set(undefined);
      if (onApproved !== undefined) {
        onApproved();
        return;
      }
      return;
    }
    state.error.set(failSentence(res.status, res.data));
  } catch {
    if (!stillOnDevice(root)) {
      return;
    }
    state.error.set(FAIL_SENTENCE);
  } finally {
    zeroizeBytes(ephBytes);
    state.pending.set(false);
    if (
      stillOnDevice(root) &&
      (onApproved === undefined || state.error.get() !== undefined)
    ) {
      renderDevice(state, root, onApproved);
    }
  }
}
