import {
  BREAKPOINT_PX,
  FAIL_SENTENCE,
  deviceQuery,
  layoutMode,
  logoutUrl,
  removePasskeyEnabled,
  req,
  sessionUrl,
  type LayoutMode,
} from "./lib/api.ts";
import { signal } from "./lib/signal.ts";
import {
  bumpLogoutGen,
  currentLogoutGen,
  leaveAccount,
  renderAccount as renderAccountScreen,
} from "./screens/account.ts";
import { leaveActivity, renderActivity as renderActivityScreen } from "./screens/activity.ts";
import { renderDevice } from "./screens/device.ts";
import { abandonRegister, renderRegister as renderRegisterScreen } from "./screens/register.ts";
import { getDek } from "./lib/crypto.ts";
import {
  forgetRemember,
  leaveGate,
  loadRemember,
  renderGate,
  resolveGate,
  sentenceFor,
  type AuthMethod,
  type GateHost,
  type GateKind,
  type GateView,
  type SessionInfo,
} from "./screens/gate.ts";

export { resolveGate, type AuthMethod, type GateKind, type GateView, type SessionInfo };

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


export type PasskeyRow = {
  id: string;
  created: string;
};

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


function gateHost(state: AppState): GateHost {
  return {
    navigate(to) {
      navigate(state, to);
    },
    redraw() {
      render(state);
    },
    loadSession() {
      return loadSession(state);
    },
  };
}

function nav(state: AppState, screen: Screen): HTMLElement {
  const items: Array<[Screen, string]> = [
    ["register", "Register"],
    ["activity", "Activity"],
    ["account", "Account"],
  ];
  const bar = el(
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
  if (state.session.get() !== undefined) {
    const out = el(
      "button",
      { type: "button", class: "secondary", "data-action": "logout" },
      ["Sign out"],
    );
    out.addEventListener("click", () => {
      void onLogout(state);
    });
    bar.append(out);
  }
  return bar;
}

export function renderRegister(state: AppState, root: HTMLElement): void {
  const host = state as AppState & { onLogout?: () => void };
  host.onLogout = () => {
    void onLogout(state);
  };
  renderRegisterScreen(host, root);
}

function renderActivity(state: AppState, root: HTMLElement): void {
  renderActivityScreen(state, root, nav(state, "activity"));
}

export function renderAccount(state: AppState, root: HTMLElement): void {
  mounted = root;
  renderAccountScreen(state, root, nav(state, "account"), {
    onLogout: () => {
      void onLogout(state);
    },
    loadSession: () => loadSession(state),
    wipeDek,
    redraw: () => {
      render(state);
    },
  });
}

export function render(state: AppState): void {
  const root = mounted ?? document.getElementById("app");
  if (!root) {
    return;
  }
  mounted = root;
  const screen = screenFromPath(state.path.get());
  if (screen !== "gate") {
    leaveGate(state);
  }
  if (screen !== "register") {
    abandonRegister(state);
  }
  if (screen !== "account") {
    leaveAccount(state);
  }
  if (screen !== "activity") {
    leaveActivity(state);
  }
  switch (screen) {
    case "device":
      if (state.session.get() === undefined || getDek() === undefined) {
        renderGate(state, root, gateHost(state));
      } else {
        renderDevice(state, root, () => {
          navigate(state, "/register");
        });
      }
      break;
    case "register":
      if (getDek() === undefined) {
        renderGate(state, root, gateHost(state));
      } else {
        renderRegister(state, root);
      }
      break;
    case "activity":
      if (state.session.get() === undefined) {
        renderGate(state, root, gateHost(state));
      } else {
        renderActivity(state, root);
      }
      break;
    case "account":
      if (state.session.get() === undefined) {
        renderGate(state, root, gateHost(state));
      } else {
        renderAccount(state, root);
      }
      break;
    default:
      renderGate(state, root, gateHost(state));
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
  leaveAccount(state);
  leaveGate(state);
  forgetRemember();
  state.session.set(undefined);
  state.pending.set(false);
  state.error.set(undefined);
  state.method.set(undefined);
  state.different.set(false);
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

async function loadSession(state: AppState): Promise<void> {
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
