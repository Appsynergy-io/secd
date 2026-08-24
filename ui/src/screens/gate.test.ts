import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  EMAIL_AUTOCOMPLETE,
  FAIL_SENTENCE,
  LAST_KEY,
  RATE_SENTENCE,
} from "../lib/api.ts";
import { signal } from "../lib/signal.ts";
import {
  CLIP_FAIL_SENTENCE,
  loadRemember,
  renderGate,
  resolveGate,
  type AuthMethod,
  type GateHost,
  type GateState,
  type SessionInfo,
} from "./gate.ts";

if (typeof document === "undefined") {
  GlobalRegistrator.register({ url: "http://localhost/" });
}

const EMAIL = "a@b.c";
const FRESH = (): string => new Date().toISOString();

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function makeState(
  q: {
    path?: string;
    email?: string;
    password?: string;
    error?: string | undefined;
    pending?: boolean;
    session?: SessionInfo | undefined;
    method?: AuthMethod | undefined;
    different?: boolean;
    revealPassword?: boolean;
    userCode?: string;
  } = {},
): GateState {
  return {
    path: signal(q.path ?? "/"),
    email: signal(q.email ?? ""),
    password: signal(q.password ?? ""),
    error: signal(q.error),
    pending: signal(q.pending === true),
    session: signal(q.session),
    method: signal(q.method),
    different: signal(q.different === true),
    revealPassword: signal(q.revealPassword === true),
    userCode: signal(q.userCode ?? ""),
  };
}

function liveSession(): SessionInfo {
  return { email: EMAIL, has_passkey: true, has_password: false, session_id: "s1" };
}

function remember(hasPasskey: boolean, at = FRESH()): void {
  localStorage.setItem(LAST_KEY, JSON.stringify({ email: EMAIL, has_passkey: hasPasskey, at }));
}

function hostFor(
  state: GateState,
  root: HTMLElement,
  navs: string[],
  load: () => Promise<void> = async () => {},
): GateHost {
  const host: GateHost = {
    navigate(to) {
      navs.push(to);
      state.path.set(to);
    },
    redraw() {
      renderGate(state, root, host);
    },
    loadSession: load,
  };
  return host;
}

function submit(root: HTMLElement): void {
  root.querySelector("form")?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
}

describe("resolveGate", () => {
  test("five kinds from session, remember, method, and cold start", () => {
    expect(resolveGate({ session: liveSession() }).kind).toBe("approve-only");
    expect(resolveGate({ remember: { email: EMAIL, has_passkey: true, at: FRESH() } }).kind).toBe(
      "remembered-passkey",
    );
    expect(resolveGate({ remember: { email: EMAIL, has_passkey: false, at: FRESH() } }).kind).toBe(
      "remembered-password",
    );
    expect(resolveGate({ method: "password" }).kind).toBe("identity");
    const cold = resolveGate({});
    expect(cold.kind).toBe("cold");
    expect(cold.showEmail).toBe(true);
    expect(cold.emailAutocomplete).toBe(EMAIL_AUTOCOMPLETE);
  });

  test("stale secd.last falls through to email", () => {
    const v = resolveGate({
      remember: { email: EMAIL, has_passkey: true, at: "1999-01-01T00:00:00.000Z" },
    });
    expect(v.kind).toBe("cold");
    expect(v.showEmail).toBe(true);
  });

  test("remembered passkey omits email and password", () => {
    const v = resolveGate({ remember: { email: EMAIL, has_passkey: true, at: FRESH() } });
    expect(v.showEmail).toBe(false);
    expect(v.showPassword).toBe(false);
    expect(v.showPasskey).toBe(true);
    expect(v.emailPrefill).toBe(EMAIL);
  });
});

describe("gate screen", () => {
  const origFetch = globalThis.fetch;
  const origNav = globalThis.navigator;

  beforeEach(() => {
    document.body.replaceChildren();
    localStorage.clear();
  });

  afterEach(() => {
    globalThis.fetch = origFetch;
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: origNav });
    localStorage.clear();
  });

  test("loadRemember reads secd.last", () => {
    remember(false);
    expect(loadRemember()?.email).toBe(EMAIL);
    expect(loadRemember()?.has_passkey).toBe(false);
  });

  test("remembered-password Continue posts the stored email", async () => {
    remember(false);
    const posts: Array<{ url: string; body: unknown }> = [];
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      posts.push({
        url: reqUrl(input),
        body: init?.body === undefined ? undefined : JSON.parse(String(init.body)),
      });
      return new Response("{}", { status: 401 });
    }) as unknown as typeof fetch;
    const root = document.createElement("div");
    const state = makeState({ password: "twelve-chars!" });
    const navs: string[] = [];
    renderGate(state, root, hostFor(state, root, navs));
    expect(root.querySelector("#email")).toBeNull();
    expect(root.querySelector("button[type='submit']")?.textContent).toBe("Continue");
    submit(root);
    await Bun.sleep(1);
    expect(state.email.get()).toBe(EMAIL);
    expect(posts[0]).toEqual({
      url: "/api/auth/password/login",
      body: { email: EMAIL, password: "twelve-chars!" },
    });
    expect(navs).toEqual([]);
    expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
  });

  test("remembered-passkey has no Continue; Passkey is the submit", () => {
    remember(true);
    const root = document.createElement("div");
    const state = makeState();
    renderGate(state, root, hostFor(state, root, []));
    expect(root.querySelector("[data-kind='remembered-passkey']")).not.toBeNull();
    const buttons = [...root.querySelectorAll("button")].map((b) => b.textContent);
    expect(buttons).toContain("Use a passkey");
    expect(buttons).not.toContain("Continue");
    expect(root.querySelector('[data-action="passkey"]')?.getAttribute("type")).toBe("submit");
  });

  test("live session on / with empty user_code navigates to /register", () => {
    const root = document.createElement("div");
    const posts: string[] = [];
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      posts.push(reqUrl(input));
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
    const state = makeState({ session: liveSession(), path: "/", userCode: "" });
    const navs: string[] = [];
    renderGate(state, root, hostFor(state, root, navs));
    expect(navs).toEqual(["/register"]);
    expect(root.querySelector('[data-action="approve"]')).toBeNull();
    expect(root.querySelector("form")).toBeNull();
    expect(posts).toEqual([]);
  });

  test("429 uses the rate sentence; other failures use the fail sentence", async () => {
    const root = document.createElement("div");
    globalThis.fetch = (async () => new Response("{}", { status: 429 })) as unknown as typeof fetch;
    const state = makeState({ email: EMAIL });
    renderGate(state, root, hostFor(state, root, []));
    submit(root);
    await Bun.sleep(1);
    expect(root.querySelector(".error")?.textContent).toBe(RATE_SENTENCE);
    globalThis.fetch = (async () => new Response("{}", { status: 401 })) as unknown as typeof fetch;
    state.error.set(undefined);
    state.pending.set(false);
    renderGate(state, root, hostFor(state, root, []));
    submit(root);
    await Bun.sleep(1);
    expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
  });

  test("Copy writes the failure sentence and becomes Copied only after writeText resolves", async () => {
    const root = document.createElement("div");
    let finish: ((value: void) => void) | undefined;
    let wrote: string | undefined;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {
        clipboard: {
          writeText: (text: string) =>
            new Promise<void>((resolve) => {
              wrote = text;
              finish = resolve;
            }),
        },
      },
    });
    const state = makeState({ email: EMAIL, error: FAIL_SENTENCE });
    renderGate(state, root, hostFor(state, root, []));
    const btn = root.querySelector('[data-action="copy"]') as HTMLButtonElement | null;
    expect(btn?.textContent).toBe("Copy");
    expect((root.querySelector('[data-copy="value"]') as HTMLInputElement | null)?.value).toBe(
      FAIL_SENTENCE,
    );
    btn?.click();
    await Bun.sleep(1);
    expect(btn?.textContent).toBe("Copy");
    expect(wrote).toBe(FAIL_SENTENCE);
    finish?.();
    await Bun.sleep(1);
    expect(root.querySelector('[data-action="copy"]')?.textContent).toBe("Copied");
    expect(wrote).toBe(FAIL_SENTENCE);
  });

  test("clipboard refusal keeps Copy and offers select-to-copy", async () => {
    const root = document.createElement("div");
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: {} });
    const state = makeState({ email: EMAIL, error: FAIL_SENTENCE });
    renderGate(state, root, hostFor(state, root, []));
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    await Bun.sleep(1);
    expect(root.querySelector('[data-action="copy"]')?.textContent).toBe("Copy");
    expect(root.querySelector(".error")?.textContent).toBe(CLIP_FAIL_SENTENCE);
    expect((root.querySelector('[data-copy="value"]') as HTMLInputElement | null)?.value).toBe(
      CLIP_FAIL_SENTENCE,
    );
  });

  test("empty email disables Continue with a visible reason", () => {
    const root = document.createElement("div");
    const state = makeState();
    renderGate(state, root, hostFor(state, root, []));
    expect(root.querySelector("[data-state='empty']")).not.toBeNull();
    expect(root.querySelector("button[type='submit']")?.hasAttribute("disabled")).toBe(true);
    expect(root.querySelector("[data-reason]")?.textContent).toBe("Enter your email.");
  });

  test("pending disables submit and shows Signing in", () => {
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, pending: true });
    renderGate(state, root, hostFor(state, root, []));
    expect(root.querySelector("[data-state='loading']")).not.toBeNull();
    expect(root.querySelector("form")?.getAttribute("aria-busy")).toBe("true");
    expect(root.querySelector("button[type='submit']")?.hasAttribute("disabled")).toBe(true);
    expect(root.querySelector("[data-reason]")?.textContent).toBe("Signing in.");
  });

  test("cold Continue POSTs /api/auth/start with the typed email", async () => {
    const root = document.createElement("div");
    const posts: Array<{ url: string; body: unknown }> = [];
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      posts.push({
        url: reqUrl(input),
        body: init?.body === undefined ? undefined : JSON.parse(String(init.body)),
      });
      return new Response(JSON.stringify({ method: "password" }), { status: 200 });
    }) as unknown as typeof fetch;
    const state = makeState({ email: EMAIL });
    renderGate(state, root, hostFor(state, root, []));
    submit(root);
    await Bun.sleep(1);
    expect(posts).toEqual([{ url: "/api/auth/start", body: { email: EMAIL } }]);
    expect(state.method.get()).toBe("password");
    expect(root.querySelector("#password")).not.toBeNull();
  });

  test("POST login 200 then GET /session 401 stays on / with .error", async () => {
    const root = document.createElement("div");
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "POST" && url === "/api/auth/password/login") {
        return new Response("{}", { status: 200 });
      }
      if (method === "GET" && url === "/api/session") {
        return new Response("{}", { status: 401 });
      }
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
    const state = makeState({ email: EMAIL, password: "twelve-chars!", method: "password" });
    const navs: string[] = [];
    const host = hostFor(state, root, navs, async () => {
      const res = await fetch("/api/session");
      if (res.status !== 200) {
        state.session.set(undefined);
      }
    });
    renderGate(state, root, host);
    submit(root);
    await Bun.sleep(1);
    expect(state.path.get()).toBe("/");
    expect(navs).toEqual([]);
    expect(state.session.get()).toBeUndefined();
    expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
  });
});
