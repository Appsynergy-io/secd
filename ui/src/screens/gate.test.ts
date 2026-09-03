import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  EMAIL_AUTOCOMPLETE,
  FAIL_SENTENCE,
  LAST_KEY,
  RATE_SENTENCE,
} from "../lib/api.ts";
import {
  clearDek,
  getDek,
  mintDek,
  setDek,
  toHex,
  wrapPasskey,
  wrapPassword,
  wrapToJson,
  zeroizeBytes,
} from "../lib/crypto.ts";
import * as keyholder from "../lib/keyholder.ts";
import type { AppState, AuthMethod, Host, SessionInfo } from "../lib/host.ts";
import { loadRemember } from "../lib/remember.ts";
import { signal } from "../lib/signal.ts";
import {
  CREATE_LABEL,
  CREATE_PASSKEY_LABEL,
  DIFFERENT_LABEL,
  PASSKEY_LABEL,
  PASSKEY_SUB,
  PASSWORD_LABEL,
  PASSWORD_SUB,
  PENDING_LABEL,
  REGISTER_SUB,
  REGISTER_TITLE,
  SHORT_SENTENCE,
  SIGN_IN_LABEL,
  SUB,
  TITLE,
  WELCOME_TITLE,
  afterLoginPath,
  copyFor,
  leaveGate,
  renderGate,
  resolveGate,
} from "./gate.ts";

const EMAIL = "a@b.c";
const PW = "twelve-chars!";
const FRESH = (): string => new Date().toISOString();

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function pathOf(url: string): string {
  try {
    return new URL(url, "http://localhost").pathname;
  } catch {
    return url;
  }
}

function settled(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

async function untilIdle(state: AppState): Promise<void> {
  for (let i = 0; i < 400; i++) {
    if (!state.pending.get()) {
      await settled();
      return;
    }
    await settled();
  }
  throw new Error("gate stayed pending");
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
    eph?: string;
  } = {},
): AppState {
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
    eph: signal(q.eph ?? ""),
    counts: signal({}),
    toast: signal(""),
  };
}

function liveSession(hasPasskey = true): SessionInfo {
  return { email: EMAIL, has_passkey: hasPasskey, has_password: !hasPasskey, session_id: "s1" };
}

function remember(hasPasskey: boolean, at = FRESH()): void {
  localStorage.setItem(LAST_KEY, JSON.stringify({ email: EMAIL, has_passkey: hasPasskey, at }));
}

function makeHost(
  state: AppState,
  root: HTMLElement,
  q: {
    navs?: string[];
    signOuts?: number[];
    load?: () => Promise<void>;
  } = {},
): Host {
  const navs = q.navs ?? [];
  const signOuts = q.signOuts ?? [];
  const host: Host = {
    navigate(to) {
      navs.push(to);
    },
    redraw() {
      renderGate(state, root, host);
    },
    flash() {},
    async signOut() {
      signOuts.push(1);
    },
    loadSession: q.load ?? (async () => {}),
    actions: document.createElement("div"),
  };
  return host;
}

type Call = { method: string; url: string; body: unknown };

function installFetch(
  handler: (method: string, url: string, body: unknown) => Response | Promise<Response>,
): Call[] {
  const calls: Call[] = [];
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = pathOf(reqUrl(input));
    const method = String(init?.method ?? "GET").toUpperCase();
    const body = init?.body === undefined ? undefined : JSON.parse(String(init.body));
    calls.push({ method, url, body });
    return handler(method, url, body);
  }) as unknown as typeof fetch;
  return calls;
}

function submit(root: HTMLElement): void {
  root.querySelector("form")?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
}

function passkeyCred(prf: Uint8Array, raw = new Uint8Array([10, 11, 12, 13])): PublicKeyCredential {
  return {
    id: "cred",
    type: "public-key",
    rawId: raw.buffer,
    response: {
      clientDataJSON: new Uint8Array([1]).buffer,
      authenticatorData: new Uint8Array([2]).buffer,
      signature: new Uint8Array([3]).buffer,
      attestationObject: new Uint8Array([2]).buffer,
    },
    getClientExtensionResults: () => ({
      prf: { results: { first: prf.buffer } },
    }),
  } as unknown as PublicKeyCredential;
}

function installPasskey(kind: "get" | "create", prf: Uint8Array): void {
  const cred = passkeyCred(prf);
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: {
      credentials: {
        get: async () => {
          if (kind !== "get") {
            throw new Error("login get must not run");
          }
          return cred;
        },
        create: async () => {
          if (kind !== "create") {
            throw new Error("register create must not run");
          }
          return cred;
        },
      },
    },
  });
}

describe("resolveGate", () => {
  test("five kinds from session, remember, method, and cold start", () => {
    expect(resolveGate({ session: liveSession(), unlocked: true }).kind).toBe("approve-only");
    expect(resolveGate({ session: liveSession() }).kind).toBe("remembered-passkey");
    expect(resolveGate({ remember: { email: EMAIL, has_passkey: true, at: FRESH() } }).kind).toBe(
      "remembered-passkey",
    );
    expect(resolveGate({ remember: { email: EMAIL, has_passkey: false, at: FRESH() } }).kind).toBe(
      "remembered-password",
    );
    expect(resolveGate({ method: "password" }).kind).toBe("identity");
    const cold = resolveGate({});
    expect(cold.kind).toBe("cold");
    expect(cold.mode).toBe("passkey");
    expect(cold.showEmail).toBe(true);
    expect(cold.alternate).toBe("password");
  });

  test("stale secd.last falls through to email", () => {
    const v = resolveGate({
      remember: { email: EMAIL, has_passkey: true, at: "1999-01-01T00:00:00.000Z" },
    });
    expect(v.kind).toBe("cold");
    expect(v.showEmail).toBe(true);
  });

  test("remembered passkey omits email and offers password as the alternate", () => {
    const v = resolveGate({ remember: { email: EMAIL, has_passkey: true, at: FRESH() } });
    expect(v.showEmail).toBe(false);
    expect(v.mode).toBe("passkey");
    expect(v.alternate).toBe("password");
    expect(v.emailPrefill).toBe(EMAIL);
    expect(v.showUseDifferentAccount).toBe(true);
  });

  test("Use password on a remembered passkey switches the primary factor", () => {
    const v = resolveGate({
      remember: { email: EMAIL, has_passkey: true, at: FRESH() },
      revealPassword: true,
    });
    expect(v.kind).toBe("remembered-passkey");
    expect(v.mode).toBe("password");
    expect(v.alternate).toBe("passkey");
  });

  test("register and either map onto the identity card", () => {
    const reg = resolveGate({ method: "register", email: EMAIL });
    expect(reg.kind).toBe("identity");
    expect(reg.mode).toBe("register");
    expect(reg.alternate).toBe("passkey");
    const either = resolveGate({ method: "either" });
    expect(either.mode).toBe("passkey");
    expect(either.alternate).toBe("password");
  });
});

describe("copyFor and afterLoginPath", () => {
  test("cold card matches the design; pending replaces the primary label", () => {
    const view = resolveGate({});
    expect(copyFor(view, false)).toEqual({
      title: TITLE,
      sub: SUB,
      primary: PASSKEY_LABEL,
      secondary: PASSWORD_LABEL,
    });
    expect(copyFor(view, true).primary).toBe(PENDING_LABEL);
  });

  test("remembered and register cards keep the secondary-state sentences", () => {
    expect(
      copyFor(resolveGate({ remember: { email: EMAIL, has_passkey: true, at: FRESH() } }), false),
    ).toEqual({
      title: WELCOME_TITLE,
      sub: PASSKEY_SUB,
      primary: PASSKEY_LABEL,
      secondary: PASSWORD_LABEL,
    });
    expect(
      copyFor(resolveGate({ remember: { email: EMAIL, has_passkey: false, at: FRESH() } }), false),
    ).toEqual({
      title: WELCOME_TITLE,
      sub: PASSWORD_SUB,
      primary: SIGN_IN_LABEL,
      secondary: undefined,
    });
    expect(copyFor(resolveGate({ method: "register" }), false)).toEqual({
      title: REGISTER_TITLE,
      sub: REGISTER_SUB,
      primary: CREATE_LABEL,
      secondary: CREATE_PASSKEY_LABEL,
    });
  });

  test("a user code sends unlock to /device, else /vault", () => {
    expect(afterLoginPath("")).toBe("/vault");
    expect(afterLoginPath("ABCD-EFGH")).toBe("/device");
  });
});

describe("gate screen", () => {
  const origFetch = globalThis.fetch;
  const origNav = globalThis.navigator;

  beforeEach(async () => {

    await keyholder.start();
    document.body.replaceChildren();
    localStorage.clear();
    clearDek();
  });

  afterEach(() => {
    globalThis.fetch = origFetch;
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: origNav });
    localStorage.clear();
    clearDek();
    document.body.replaceChildren();
  });

  test("cold start paints the design card into the bare shell", () => {
    const root = document.createElement("div");
    const state = makeState();
    renderGate(state, root, makeHost(state, root));
    expect(root.querySelector(".gate > .gate-wrap")).not.toBeNull();
    expect(root.querySelector(".gate-brand .brand-mark")?.textContent).toBe("s");
    expect(root.querySelector(".gate-brand")?.textContent).toContain("secd console");
    const card = root.querySelector(".gate-card");
    expect(card).not.toBeNull();
    expect(card?.getAttribute("data-kind")).toBe("cold");
    expect(root.querySelector(".gate-title")?.textContent).toBe(TITLE);
    expect(root.querySelector(".gate-sub")?.textContent).toBe(SUB);
    const email = root.querySelector("#email") as HTMLInputElement | null;
    expect(email?.className).toBe("input input-lg");
    expect(email?.type).toBe("email");
    expect(email?.autocomplete).toBe(EMAIL_AUTOCOMPLETE);
    expect(email?.placeholder).toBe("you@company.com");
    expect(root.querySelector("label[for='email']")?.textContent).toBe("Email");
    expect(root.querySelector(".alert")).toBeNull();
    const primary = root.querySelector("button[type='submit']");
    expect(primary?.className).toContain("btn-primary");
    expect(primary?.className).toContain("btn-lg");
    expect(primary?.className).toContain("btn-block");
    expect(primary?.textContent).toBe(PASSKEY_LABEL);
    expect(root.querySelector(".divider")?.textContent).toBe("or");
    const secondary = root.querySelector('[data-action="password"]');
    expect(secondary?.textContent).toBe(PASSWORD_LABEL);
    expect(secondary?.className).toBe("btn btn-lg btn-block");
    expect(root.querySelector(".gate-foot")?.textContent).toBe(
      `LAN only · ${globalThis.location.host} · TLS 1.3`,
    );
    leaveGate(state);
  });

  test("Use password reveals the password field and Sign in", () => {
    const root = document.createElement("div");
    document.body.append(root);
    const state = makeState({ email: EMAIL });
    renderGate(state, root, makeHost(state, root));
    expect(root.querySelector("#password")).toBeNull();
    (root.querySelector('[data-action="password"]') as HTMLButtonElement | null)?.click();
    const pw = root.querySelector("#password") as HTMLInputElement | null;
    expect(pw).not.toBeNull();
    expect(pw?.className).toBe("input input-lg");
    expect(pw?.type).toBe("password");
    expect(pw?.autocomplete).toBe("current-password");
    expect(root.querySelector("button[type='submit']")?.textContent).toBe(SIGN_IN_LABEL);
    expect(root.querySelector('[data-action="passkey"]')?.textContent).toBe(PASSKEY_LABEL);
    const toggle = root.querySelector('[data-action="password-toggle"]') as HTMLButtonElement | null;
    expect(toggle?.textContent).toBe("Show");
    toggle?.click();
    expect((root.querySelector("#password") as HTMLInputElement | null)?.type).toBe("text");
    expect(root.querySelector('[data-action="password-toggle"]')?.textContent).toBe("Hide");
    leaveGate(state);
    root.remove();
  });

  test("remembered passkey prefills the account and submits the passkey", () => {
    remember(true);
    const root = document.createElement("div");
    const state = makeState();
    renderGate(state, root, makeHost(state, root));
    expect(root.querySelector("[data-kind='remembered-passkey']")).not.toBeNull();
    expect(root.querySelector("#email")).toBeNull();
    expect(root.querySelector("[data-remembered]")?.textContent).toBe(EMAIL);
    expect(root.querySelector("button[type='submit']")?.textContent).toBe(PASSKEY_LABEL);
    expect(root.querySelector('[data-action="passkey"]')?.getAttribute("type")).toBe("submit");
    expect(root.querySelector('[data-action="different"]')?.textContent).toBe(DIFFERENT_LABEL);
    expect(state.email.get()).toBe(EMAIL);
    leaveGate(state);
  });

  test("live cookie session with no tab DEK prefills; Use a different account forgets it", () => {
    const root = document.createElement("div");
    document.body.append(root);
    remember(true);
    const state = makeState({ session: liveSession() });
    renderGate(state, root, makeHost(state, root));
    expect(root.querySelector("[data-remembered]")?.textContent).toBe(EMAIL);
    (root.querySelector('[data-action="different"]') as HTMLButtonElement | null)?.click();
    expect(loadRemember()).toBeUndefined();
    expect(localStorage.getItem(LAST_KEY)).toBeNull();
    expect(root.querySelector("#email")).not.toBeNull();
    expect(state.email.get()).toBe("");
    expect(document.activeElement).toBe(root.querySelector("#email"));
    leaveGate(state);
    root.remove();
  });

  test("remembered-password Sign in posts the stored email, not /start", async () => {
    remember(false);
    const calls = installFetch(() => new Response("{}", { status: 401 }));
    const root = document.createElement("div");
    const state = makeState({ password: PW });
    const signOuts: number[] = [];
    renderGate(state, root, makeHost(state, root, { signOuts }));
    expect(root.querySelector("#email")).toBeNull();
    expect(root.querySelector("button[type='submit']")?.textContent).toBe(SIGN_IN_LABEL);
    submit(root);
    await untilIdle(state);
    expect(calls).toEqual([{ method: "POST", url: "/api/auth/password/login", body: { email: EMAIL, password: PW } }]);
    expect(root.querySelector(".alert.alert-danger[role='alert']")?.textContent).toBe(FAIL_SENTENCE);
    expect(signOuts).toEqual([]);
    expect(state.password.get()).toBe("");
    leaveGate(state);
  });

  test("cold Continue with passkey POSTs /api/auth/start; 401 is a failed login, not signOut", async () => {
    const calls = installFetch(() => new Response("{}", { status: 401 }));
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL });
    const signOuts: number[] = [];
    renderGate(state, root, makeHost(state, root, { signOuts }));
    submit(root);
    await untilIdle(state);
    expect(calls).toEqual([{ method: "POST", url: "/api/auth/start", body: { email: EMAIL } }]);
    expect(root.querySelector("[role='alert']")?.textContent).toBe(FAIL_SENTENCE);
    expect(signOuts).toEqual([]);
    leaveGate(state);
  });

  test("429 uses the rate sentence", async () => {
    installFetch(() => new Response("{}", { status: 429 }));
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL });
    const signOuts: number[] = [];
    renderGate(state, root, makeHost(state, root, { signOuts }));
    submit(root);
    await untilIdle(state);
    expect(root.querySelector("[role='alert']")?.textContent).toBe(RATE_SENTENCE);
    expect(signOuts).toEqual([]);
    leaveGate(state);
  });

  test("pending disables both buttons and paints Signing in… before the response", async () => {
    let finish: ((value: Response) => void) | undefined;
    installFetch(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve;
        }),
    );
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL });
    renderGate(state, root, makeHost(state, root));
    submit(root);
    await settled();
    expect(root.querySelector("form")?.getAttribute("aria-busy")).toBe("true");
    expect(root.querySelector("button[type='submit']")?.textContent).toBe(PENDING_LABEL);
    expect((root.querySelector("button[type='submit']") as HTMLButtonElement | null)?.disabled).toBe(
      true,
    );
    expect((root.querySelector('[data-action="password"]') as HTMLButtonElement | null)?.disabled).toBe(
      true,
    );
    finish?.(new Response("{}", { status: 401 }));
    await untilIdle(state);
    leaveGate(state);
  });

  test("Use password then Sign in posts /start then password login", async () => {
    const calls = installFetch((method, url) => {
      if (method === "POST" && url === "/api/auth/start") {
        return new Response(JSON.stringify({ method: "password" }), { status: 200 });
      }
      return new Response("{}", { status: 401 });
    });
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL });
    renderGate(state, root, makeHost(state, root));
    (root.querySelector('[data-action="password"]') as HTMLButtonElement | null)?.click();
    const pw = root.querySelector("#password") as HTMLInputElement;
    pw.value = PW;
    pw.dispatchEvent(new Event("input", { bubbles: true }));
    submit(root);
    await untilIdle(state);
    expect(calls[0]).toEqual({ method: "POST", url: "/api/auth/start", body: { email: EMAIL } });
    expect(calls[1]).toEqual({
      method: "POST",
      url: "/api/auth/password/login",
      body: { email: EMAIL, password: PW },
    });
    leaveGate(state);
  });

  test("password login 200 with wraps navigates to /vault", async () => {
    const dek = mintDek();
    const pwBytes = new TextEncoder().encode(PW);
    const wraps = { wraps: [wrapToJson(wrapPassword(dek, pwBytes))] };
    zeroizeBytes(pwBytes);
    zeroizeBytes(dek);
    const calls = installFetch((method, url) => {
      if (method === "POST" && url === "/api/auth/password/login") {
        return new Response(JSON.stringify(wraps), { status: 200 });
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, password: PW, method: "password" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, {
      navs,
      load: async () => {
        state.session.set(liveSession(false));
      },
    }));
    submit(root);
    await untilIdle(state);
    expect(calls.some((c) => c.url === "/api/auth/password/login")).toBe(true);
    expect(getDek()).toBeDefined();
    expect(navs).toEqual(["/vault"]);
    expect(loadRemember()?.email).toBe(EMAIL);
    expect(loadRemember()?.has_passkey).toBe(false);
    leaveGate(state);
  }, { timeout: 30_000 });

  test("a user code after unlock navigates to /device", async () => {
    const dek = mintDek();
    const pwBytes = new TextEncoder().encode(PW);
    const wraps = { wraps: [wrapToJson(wrapPassword(dek, pwBytes))] };
    zeroizeBytes(pwBytes);
    zeroizeBytes(dek);
    installFetch((method, url) => {
      if (method === "POST" && url === "/api/auth/password/login") {
        return new Response(JSON.stringify(wraps), { status: 200 });
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({
      email: EMAIL,
      password: PW,
      method: "password",
      userCode: "ABCD-EFGH",
    });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, {
      navs,
      load: async () => {
        state.session.set(liveSession(false));
      },
    }));
    submit(root);
    await untilIdle(state);
    expect(navs).toEqual(["/device"]);
    leaveGate(state);
  }, { timeout: 30_000 });

  test("unwrap miss after login stays on the gate with FAIL_SENTENCE", async () => {
    setDek(mintDek());
    installFetch((method, url) => {
      if (method === "POST" && url === "/api/auth/password/login") {
        return new Response(JSON.stringify({ wraps: [] }), { status: 200 });
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, password: PW, method: "password" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, {
      navs,
      load: async () => {
        state.session.set(liveSession(false));
      },
    }));
    submit(root);
    await untilIdle(state);
    expect(getDek()).toBeUndefined();
    expect(state.session.get()).toBeUndefined();
    expect(navs).toEqual([]);
    expect(root.querySelector("[role='alert']")?.textContent).toBe(FAIL_SENTENCE);
    leaveGate(state);
  });

  test("register card posts a password wrap; Create passkey instead never hits login", async () => {
    const prf = new Uint8Array(32).fill(9);
    installPasskey("create", prf);
    const calls = installFetch((method, url) => {
      if (url === "/api/auth/passkey/register/start") {
        return new Response(
          JSON.stringify({ handle: "h1", publicKey: { challenge: "AQIDBA" } }),
          { status: 200 },
        );
      }
      if (url === "/api/auth/passkey/register/finish") {
        return new Response("{}", { status: 200 });
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, method: "register" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, {
      navs,
      load: async () => {
        state.session.set(liveSession());
      },
    }));
    expect(root.querySelector(".gate-title")?.textContent).toBe(REGISTER_TITLE);
    expect(root.querySelector("#password")).not.toBeNull();
    expect(root.querySelector("#confirm")).not.toBeNull();
    expect(root.querySelector('[data-action="passkey"]')?.textContent).toBe(CREATE_PASSKEY_LABEL);
    (root.querySelector('[data-action="passkey"]') as HTMLButtonElement | null)?.click();
    await untilIdle(state);
    expect(calls.some((c) => c.url.includes("/api/auth/passkey/login"))).toBe(false);
    expect(calls.some((c) => c.url === "/api/auth/passkey/register/start")).toBe(true);
    const finish = calls.find((c) => c.url === "/api/auth/passkey/register/finish");
    const body = finish?.body as { handle?: string; email?: string; wrap?: { factor?: string } };
    expect(body.handle).toBe("h1");
    expect(body.email).toBe(EMAIL);
    expect(body.wrap?.factor).toBe("passkey");
    expect(navs).toEqual(["/vault"]);
    leaveGate(state);
  });

  test("register with matching passwords posts wrapPassword", async () => {
    const calls = installFetch((method, url) => {
      if (url === "/api/auth/password/register") {
        return new Response("{}", { status: 200 });
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, password: PW, method: "register" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, {
      navs,
      load: async () => {
        state.session.set(liveSession(false));
      },
    }));
    const confirm = root.querySelector("#confirm") as HTMLInputElement;
    confirm.value = PW;
    confirm.dispatchEvent(new Event("input", { bubbles: true }));
    submit(root);
    await untilIdle(state);
    const posted = calls.find((c) => c.url === "/api/auth/password/register");
    const body = posted?.body as {
      email?: string;
      password?: string;
      wrap?: { factor?: string; blob?: string; salt?: string };
    };
    expect(body.email).toBe(EMAIL);
    expect(body.password).toBe(PW);
    expect(body.wrap?.factor).toBe("password");
    expect(typeof body.wrap?.blob).toBe("string");
    expect(typeof body.wrap?.salt).toBe("string");
    expect(navs).toEqual(["/vault"]);
    leaveGate(state);
  }, { timeout: 30_000 });

  test("short register password paints SHORT_SENTENCE", async () => {
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, password: "tooshort", method: "register" });
    renderGate(state, root, makeHost(state, root));
    const confirm = root.querySelector("#confirm") as HTMLInputElement;
    confirm.value = "tooshort";
    confirm.dispatchEvent(new Event("input", { bubbles: true }));
    submit(root);
    await untilIdle(state);
    expect(root.querySelector("[role='alert']")?.textContent).toBe(SHORT_SENTENCE);
    leaveGate(state);
  });

  test("passkey login posts start then finish with email and navigates", async () => {
    const dek = mintDek();
    const prf = new Uint8Array(32).fill(7);
    const wrapPrf = prf.slice();
    const wraps = {
      wraps: [wrapToJson(wrapPasskey(dek, wrapPrf, toHex(new Uint8Array([10, 11, 12, 13]))))],
    };
    zeroizeBytes(dek);
    zeroizeBytes(wrapPrf);
    installPasskey("get", prf);
    const calls = installFetch((method, url) => {
      if (url === "/api/auth/passkey/login/start") {
        return new Response(
          JSON.stringify({ handle: "h1", publicKey: { challenge: "AQIDBA" } }),
          { status: 200 },
        );
      }
      if (url === "/api/auth/passkey/login/finish") {
        return new Response(JSON.stringify(wraps), { status: 200 });
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, method: "passkey" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, {
      navs,
      load: async () => {
        state.session.set(liveSession());
      },
    }));
    submit(root);
    await untilIdle(state);
    expect(calls[0]).toEqual({
      method: "POST",
      url: "/api/auth/passkey/login/start",
      body: { email: EMAIL },
    });
    const finish = calls.find((c) => c.url === "/api/auth/passkey/login/finish");
    const body = finish?.body as { handle?: string; email?: string; credential?: unknown };
    expect(body.handle).toBe("h1");
    expect(body.email).toBe(EMAIL);
    expect(body.credential).toBeDefined();
    expect(navs).toEqual(["/vault"]);
    expect(getDek()).toBeDefined();
    leaveGate(state);
  });

  test("getPasskey without PRF performs no finish request", async () => {
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {
        credentials: {
          get: async () => ({
            id: "cred",
            type: "public-key",
            rawId: new Uint8Array([1, 2, 3, 4]).buffer,
            response: {
              clientDataJSON: new Uint8Array([1]).buffer,
              authenticatorData: new Uint8Array([2]).buffer,
              signature: new Uint8Array([3]).buffer,
            },
          }),
        },
      },
    });
    const calls = installFetch((method, url) => {
      if (url === "/api/auth/passkey/login/start") {
        return new Response(
          JSON.stringify({ handle: "h1", publicKey: { challenge: "AQIDBA" } }),
          { status: 200 },
        );
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, method: "passkey" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, { navs }));
    submit(root);
    await untilIdle(state);
    expect(calls.some((c) => c.url.includes("/login/finish"))).toBe(false);
    expect(navs).toEqual([]);
    expect(root.querySelector("[role='alert']")?.textContent).toBe(FAIL_SENTENCE);
    leaveGate(state);
  });

  test("passkey path clears the password before WebAuthn", async () => {
    let started = false;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {
        credentials: {
          get: () =>
            new Promise(() => {
              started = true;
            }),
        },
      },
    });
    installFetch((method, url) => {
      if (url === "/api/auth/passkey/login/start") {
        return new Response(
          JSON.stringify({ handle: "h1", publicKey: { challenge: "AQIDBA" } }),
          { status: 200 },
        );
      }
      return new Response("{}", { status: 200 });
    });
    const root = document.createElement("div");
    const state = makeState({
      email: EMAIL,
      password: PW,
      method: "either",
      revealPassword: true,
    });
    renderGate(state, root, makeHost(state, root));
    expect((root.querySelector("#password") as HTMLInputElement | null)?.value).toBe(PW);
    (root.querySelector('[data-action="passkey"]') as HTMLButtonElement | null)?.click();
    await settled();
    expect(state.password.get()).toBe("");
    expect(started).toBe(true);
    leaveGate(state);
  });

  test("approve-only (session + DEK) navigates off the gate", () => {
    setDek(mintDek());
    const root = document.createElement("div");
    const state = makeState({ session: liveSession(), userCode: "" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, { navs }));
    expect(navs).toEqual(["/vault"]);
    expect(root.querySelector("form")).toBeNull();
    const coded = makeState({ session: liveSession(), userCode: "ABCD-EFGH" });
    const navs2: string[] = [];
    renderGate(coded, root, makeHost(coded, root, { navs: navs2 }));
    expect(navs2).toEqual(["/device"]);
  });

  test("live session on / without DEK stays on the gate", () => {
    const calls = installFetch(() => new Response("{}", { status: 200 }));
    const root = document.createElement("div");
    const state = makeState({ session: liveSession(), userCode: "" });
    const navs: string[] = [];
    renderGate(state, root, makeHost(state, root, { navs }));
    expect(navs).toEqual([]);
    expect(root.querySelector("form")).not.toBeNull();
    expect(calls).toEqual([]);
    leaveGate(state);
  });

  test("empty email Continue paints FAIL_SENTENCE and does not fetch", async () => {
    const calls = installFetch(() => new Response("{}", { status: 200 }));
    const root = document.createElement("div");
    const state = makeState();
    renderGate(state, root, makeHost(state, root));
    submit(root);
    await settled();
    expect(calls).toEqual([]);
    expect(root.querySelector("[role='alert']")?.textContent).toBe(FAIL_SENTENCE);
    leaveGate(state);
  });

  test("leaveGate drops the password", () => {
    const root = document.createElement("div");
    const state = makeState({ email: EMAIL, password: PW, revealPassword: true });
    renderGate(state, root, makeHost(state, root));
    expect(state.password.get()).toBe(PW);
    leaveGate(state);
    expect(state.password.get()).toBe("");
  });
});
