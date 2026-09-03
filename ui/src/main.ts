/** The console shell: routing, the sidebar and header, the toast, sign-out.
 *  Screens paint into the content pane through the contract in lib/host.ts. */

import {
  BREAKPOINT_PX,
  FAIL_SENTENCE,
  HINT_PX,
  WIDE_PX,
  auditUrl,
  deviceQuery,
  logoutUrl,
  providersUrl,
  req,
  sessionUrl,
  sessionsUrl,
  vaultUrl,
} from "./lib/api.ts";
import * as keyholder from "./lib/keyholder.ts";
import { el } from "./lib/dom.ts";
import { bumpLogoutGen, currentLogoutGen } from "./lib/gen.ts";
import type { AppState, Host, NavCounts, Screen, SessionInfo } from "./lib/host.ts";
import { forgetRemember, loadRemember, sentenceFor } from "./lib/remember.ts";
import { signal } from "./lib/signal.ts";
import { remainingLabel } from "./lib/time.ts";
import { leaveAccess, renderAccess } from "./screens/access.ts";
import { leaveActivity, renderActivity } from "./screens/activity.ts";
import { leaveApprove, renderApprove } from "./screens/approve.ts";
import { leaveDevices, renderDevices } from "./screens/devices.ts";
import { leaveGate, renderGate } from "./screens/gate.ts";
import { leaveProviders, renderProviders } from "./screens/providers.ts";
import { leaveVault, renderVault } from "./screens/vault.ts";

export type { AppState, Host, NavCounts, Screen, SessionInfo } from "./lib/host.ts";

declare const SECD_VERSION: string | undefined;

/** Bound by scripts/build-ui.sh from Cargo.toml; "dev" under bun test. */
export const VERSION: string = typeof SECD_VERSION === "string" ? SECD_VERSION : "dev";
export const TOAST_MS = 1900;
export const KEY_TICK_MS = 30_000;
export const NO_KEY_LABEL = "no vault key in this tab";

export type ShellScreen = Exclude<Screen, "gate" | "approve">;

export const NAV: ReadonlyArray<[ShellScreen, string]> = [
  ["vault", "Vault"],
  ["providers", "Providers"],
  ["devices", "Devices"],
  ["activity", "Activity"],
  ["access", "Access"],
];

export const HINTS: Readonly<Record<ShellScreen, string>> = {
  vault: "ciphertext at rest · opened in this tab only",
  providers: "field schemas and the env vars the CLI exports",
  devices: "long-lived device sessions · approvals on /device",
  activity: "append-only, hash-chained, value-free",
  access: "factors, sessions and the key held by this tab",
};

export const SCREENS: readonly Screen[] = [
  "gate",
  "approve",
  "vault",
  "providers",
  "devices",
  "activity",
  "access",
];

export function hrefFor(screen: Screen): string {
  switch (screen) {
    case "approve":
      return "/device";
    case "vault":
      return "/vault";
    case "providers":
      return "/providers";
    case "devices":
      return "/devices";
    case "activity":
      return "/activity";
    case "access":
      return "/access";
    default:
      return "/";
  }
}

export function screenFromPath(path: string): Screen {
  switch (path) {
    case "/device":
      return "approve";
    case "/vault":
      return "vault";
    case "/providers":
      return "providers";
    case "/devices":
      return "devices";
    case "/activity":
      return "activity";
    case "/access":
      return "access";
    default:
      return "gate";
  }
}

/** A CLI approval link carries a user code; it lands on the approval page. */
export function initialPath(path: string, userCode: string): string {
  return userCode === "" ? path : "/device";
}

/** Where a fresh unlock goes: the approval it came for, else the vault. */
export function afterLoginPath(userCode: string): string {
  return userCode === "" ? "/vault" : "/device";
}

export function initials(email: string): string {
  return email.slice(0, 2).toUpperCase();
}

export function titleFor(screen: ShellScreen): string {
  return NAV.find(([id]) => id === screen)?.[1] ?? "";
}

export function keyLabel(remainingMs: number): string {
  return remainingMs > 0 ? `vault key · ${remainingLabel(remainingMs)}` : NO_KEY_LABEL;
}

export function layoutFlags(widthPx: number): { split: boolean; wide: boolean; hint: boolean } {
  return { split: widthPx >= BREAKPOINT_PX, wide: widthPx >= WIDE_PX, hint: widthPx >= HINT_PX };
}

let mounted: HTMLElement | undefined;
let unsubCounts: (() => void) | undefined;
let unsubToast: (() => void) | undefined;
let keyTimer: ReturnType<typeof setInterval> | undefined;
let toastTimer: ReturnType<typeof setTimeout> | undefined;

function widthNow(): number {
  const w = globalThis.innerWidth;
  return typeof w === "number" && Number.isFinite(w) && w > 0 ? w : WIDE_PX;
}

function applyLayout(shell: HTMLElement): void {
  const f = layoutFlags(widthNow());
  shell.setAttribute("data-split", String(f.split));
  shell.setAttribute("data-wide", String(f.wide));
  shell.setAttribute("data-hint", String(f.hint));
}

function hostFor(state: AppState, actions: HTMLElement): Host {
  return {
    navigate(to) {
      navigate(state, to);
    },
    redraw() {
      render(state);
    },
    flash(message) {
      flash(state, message);
    },
    signOut() {
      return signOut(state);
    },
    loadSession() {
      return loadSession(state);
    },
    actions,
  };
}

export function flash(state: AppState, message: string): void {
  state.toast.set(message);
  if (toastTimer !== undefined) {
    clearTimeout(toastTimer);
  }
  toastTimer = setTimeout(() => {
    toastTimer = undefined;
    state.toast.set("");
  }, TOAST_MS);
}

function toastEl(state: AppState): HTMLElement {
  const node = el("div", { class: "toast", role: "status", "aria-live": "polite", hidden: true });
  const paint = (msg: string): void => {
    node.textContent = msg;
    node.hidden = msg === "";
  };
  paint(state.toast.get());
  unsubToast?.();
  unsubToast = state.toast.subscribe(paint);
  return node;
}

function sideEl(state: AppState, screen: ShellScreen, host: Host): HTMLElement {
  const session = state.session.get();
  const email = session?.email ?? "";
  const nav = el("nav", { class: "side-nav", "aria-label": "Console" });
  const countNodes = new Map<ShellScreen, HTMLElement>();
  for (const [id, label] of NAV) {
    const count = el("span", { class: "nav-count", "data-count": id });
    countNodes.set(id, count);
    const btn = el(
      "a",
      {
        class: "nav-item",
        href: hrefFor(id),
        "aria-current": screen === id ? "page" : undefined,
      },
      [el("span", { class: "nav-dot" }), el("span", {}, [label]), count],
    );
    nav.append(btn);
  }
  const paintCounts = (c: NavCounts): void => {
    for (const [id, node] of countNodes) {
      const v = id === "access" ? undefined : c[id];
      node.textContent = v === undefined ? "" : String(v);
    }
  };
  paintCounts(state.counts.get());
  unsubCounts?.();
  unsubCounts = state.counts.subscribe(paintCounts);

  const key = el("div", { class: "who-key", "data-key": "" }, [keyLabel(keyholder.remainingMs())]);
  if (keyTimer !== undefined) {
    clearInterval(keyTimer);
  }
  keyTimer = setInterval(() => {
    key.textContent = keyLabel(keyholder.remainingMs());
  }, KEY_TICK_MS);

  const out = el("button", { type: "button", class: "btn btn-block", "data-action": "logout" }, [
    "Sign out",
  ]);
  out.addEventListener("click", () => {
    void host.signOut();
  });

  return el("aside", { class: "side" }, [
    el("div", { class: "side-brand" }, [
      el("div", { class: "brand-mark", "aria-hidden": "true" }, ["s"]),
      el("div", { class: "brand-name" }, ["secd"]),
      el("div", { class: "chip-version", "data-version": "" }, [VERSION]),
    ]),
    nav,
    el("div", { class: "side-foot" }, [
      el("div", { class: "who" }, [
        el("div", { class: "avatar", "aria-hidden": "true" }, [initials(email)]),
        el("div", { class: "truncate" }, [
          el("div", { class: "who-email truncate", "data-email": "" }, [email]),
          key,
        ]),
      ]),
      out,
    ]),
  ]);
}

function topEl(screen: ShellScreen, actions: HTMLElement): HTMLElement {
  const right = el("div", { class: "top-actions" }, [
    actions,
    el("div", { class: "chip chip-live", "data-source": "live" }, ["live"]),
    el("div", { class: "chip", "data-host": "" }, [
      el("span", { class: "dot", "aria-hidden": "true" }),
      globalThis.location?.host ?? "",
    ]),
  ]);
  return el("header", { class: "top" }, [
    el("div", { class: "top-title" }, [titleFor(screen)]),
    el("div", { class: "top-hint" }, [HINTS[screen]]),
    right,
  ]);
}

function paintShell(state: AppState, root: HTMLElement, screen: ShellScreen): void {
  const actions = el("div", { class: "hrow", "data-actions": "" });
  const host = hostFor(state, actions);
  const content = el("div", {
    class: "content",
    "data-screen": screen,
    "data-scroll": screen === "vault" ? "hidden" : "auto",
  });
  const shell = el("div", { class: "shell" }, [
    sideEl(state, screen, host),
    el("main", { class: "main" }, [topEl(screen, actions), content]),
  ]);
  applyLayout(shell);
  root.replaceChildren(shell, toastEl(state));
  switch (screen) {
    case "vault":
      renderVault(state, content, host);
      break;
    case "providers":
      renderProviders(state, content, host);
      break;
    case "devices":
      renderDevices(state, content, host);
      break;
    case "activity":
      renderActivity(state, content, host);
      break;
    default:
      renderAccess(state, content, host);
  }
}

function paintBare(state: AppState, root: HTMLElement, screen: "gate" | "approve"): void {
  const actions = el("div", { class: "hrow", "data-actions": "" });
  const host = hostFor(state, actions);
  const shell = el("div", { class: "shell", "data-screen": screen });
  applyLayout(shell);
  root.replaceChildren(shell, toastEl(state));
  if (screen === "approve") {
    renderApprove(state, shell, host);
  } else {
    renderGate(state, shell, host);
  }
}

export function render(state: AppState): void {
  const root = mounted?.isConnected ? mounted : document.getElementById("app");
  if (!root) {
    return;
  }
  mounted = root;
  const screen = screenFromPath(state.path.get());
  if (screen !== "gate") {
    leaveGate(state);
  }
  if (screen !== "approve") {
    leaveApprove(state);
  }
  if (screen !== "vault") {
    leaveVault(state);
  }
  if (screen !== "providers") {
    leaveProviders(state);
  }
  if (screen !== "devices") {
    leaveDevices(state);
  }
  if (screen !== "activity") {
    leaveActivity(state);
  }
  if (screen !== "access") {
    leaveAccess(state);
  }
  const session = state.session.get();
  const unlocked = session !== undefined && keyholder.isUnlocked();
  switch (screen) {
    case "gate":
      if (unlocked) {
        navigate(state, afterLoginPath(state.userCode.get()));
        return;
      }
      paintBare(state, root, "gate");
      break;
    case "approve":
    case "vault":
      if (!unlocked) {
        paintBare(state, root, "gate");
      } else if (screen === "approve") {
        paintBare(state, root, "approve");
      } else {
        paintShell(state, root, "vault");
      }
      break;
    default:
      if (session === undefined) {
        paintBare(state, root, "gate");
      } else {
        paintShell(state, root, screen);
      }
  }
}

export function navigate(state: AppState, to: string): void {
  if (globalThis.location.pathname !== to) {
    globalThis.history.pushState(null, "", to);
  }
  state.path.set(to);
}

export async function signOut(state: AppState): Promise<void> {
  state.pending.set(true);
  state.error.set(undefined);
  bumpLogoutGen();
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
  forgetRemember();
  state.session.set(undefined);
  state.pending.set(false);
  state.error.set(undefined);
  state.method.set(undefined);
  state.different.set(false);
  state.password.set("");
  state.counts.set({});
  navigate(state, "/");
  render(state);
}

async function wipeDek(): Promise<void> {
  try {
    await keyholder.lock();
  } catch {
    /* session is already cleared */
  }
}

export async function loadSession(state: AppState): Promise<void> {
  const gen = currentLogoutGen();
  const res = await req("GET", sessionUrl());
  if (gen !== currentLogoutGen()) {
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
  const fresh = state.session.get()?.session_id !== data.session_id;
  state.session.set(data);
  if (fresh) {
    void seedCounts(state);
  }
}

export function asSession(v: unknown): SessionInfo | undefined {
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

function lengthOf(data: unknown, key: string): number | undefined {
  const rec = typeof data === "object" && data !== null ? (data as Record<string, unknown>) : undefined;
  const rows = rec?.[key];
  return Array.isArray(rows) ? rows.length : undefined;
}

function deviceCount(data: unknown): number | undefined {
  const rec = typeof data === "object" && data !== null ? (data as Record<string, unknown>) : undefined;
  const rows = rec?.["sessions"];
  if (!Array.isArray(rows)) {
    return undefined;
  }
  return rows.filter(
    (r) => typeof r === "object" && r !== null && (r as Record<string, unknown>)["kind"] === "device",
  ).length;
}

/** One pass over the four lists the sidebar counts. Failures leave a count blank. */
export async function seedCounts(state: AppState): Promise<void> {
  const gen = currentLogoutGen();
  const settle = async (url: string): Promise<unknown> => {
    try {
      const res = await req("GET", url);
      return res.status === 200 ? res.data : undefined;
    } catch {
      return undefined;
    }
  };
  const [vault, providers, sessions, audit] = await Promise.all([
    settle(vaultUrl()),
    settle(providersUrl()),
    settle(sessionsUrl()),
    settle(auditUrl()),
  ]);
  if (gen !== currentLogoutGen() || state.session.get() === undefined) {
    return;
  }
  const next: NavCounts = { ...state.counts.get() };
  const v = lengthOf(vault, "entries");
  const p = lengthOf(providers, "providers");
  const d = deviceCount(sessions);
  const a = lengthOf(audit, "events");
  if (v !== undefined) {
    next.vault = v;
  }
  if (p !== undefined) {
    next.providers = p;
  }
  if (d !== undefined) {
    next.devices = d;
  }
  if (a !== undefined) {
    next.activity = a;
  }
  state.counts.set(next);
}

export function freshState(bootPath = "/", code = "", eph = ""): AppState {
  return {
    path: signal(bootPath),
    email: signal(loadRemember()?.email ?? ""),
    password: signal(""),
    error: signal(undefined),
    pending: signal(false),
    session: signal(undefined),
    method: signal(undefined),
    different: signal(false),
    revealPassword: signal(false),
    userCode: signal(code),
    eph: signal(eph),
    counts: signal({}),
    toast: signal(""),
  };
}

function boot(root: HTMLElement): void {
  const { code, eph } = deviceQuery(globalThis.location.search);
  const bootPath = initialPath(globalThis.location.pathname, code);
  if (bootPath !== globalThis.location.pathname) {
    globalThis.history.replaceState(null, "", bootPath);
  }
  const state = freshState(bootPath, code, eph);
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
  globalThis.addEventListener("resize", () => {
    const shell = mounted?.querySelector(".shell");
    if (shell instanceof HTMLElement) {
      applyLayout(shell);
    }
  });
  keyholder.subscribe(() => {
    render(state);
  });
  void (async () => {
    try {
      await keyholder.start();
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
