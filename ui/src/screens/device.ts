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

export function renderDevice(
  state: DeviceState,
  root: HTMLElement,
  onApproved?: () => void,
): void {
  seedDeviceQuery(state, globalThis.location?.search ?? "");
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
    renderDevice(state, root, onApproved);
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
  if (ok) {
    copiedOf(state).set(true);
    state.error.set(undefined);
  } else {
    copiedOf(state).set(false);
    state.error.set(CLIP_FAIL_SENTENCE);
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
    state.error.set(FAIL_SENTENCE);
  } finally {
    zeroizeBytes(ephBytes);
    state.pending.set(false);
    if (onApproved === undefined || state.error.get() !== undefined) {
      renderDevice(state, root, onApproved);
    }
  }
}
