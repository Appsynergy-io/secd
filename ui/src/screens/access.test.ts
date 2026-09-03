import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  FAIL_SENTENCE,
  LAST_FACTOR_SENTENCE,
  NO_DEK_SENTENCE,
  RATE_SENTENCE,
  passkeyDeletePath,
  passkeyRegisterFinishUrl,
  passkeyRegisterStartUrl,
  passkeysUrl,
  passwordRegisterUrl,
  sessionRevokePath,
  sessionsUrl,
} from "../lib/api.ts";
import * as keyholder from "../lib/keyholder.ts";
import { clearDek, mintDek, setDek, toHex } from "../lib/crypto.ts";
import type { AppState, AuthMethod, Host, NavCounts, SessionInfo } from "../lib/host.ts";
import { signal } from "../lib/signal.ts";
import {
  EMPTY_PASSKEYS,
  EMPTY_SESSIONS,
  KEY_BODY,
  KEY_TICK_MS,
  KEY_TITLE,
  LOADING_PASSKEYS,
  LOADING_SESSIONS,
  NO_KEY_LABEL,
  PASSKEY_ADDED_TOAST,
  PASSKEY_NOTE_LAST,
  PASSKEY_NOTE_OK,
  PASSWORD_LENGTH_SENTENCE,
  PASSWORD_SET_TOAST,
  PASSWORD_SUB,
  SIGNED_OUT_TOAST,
  consoleRows,
  failSentence,
  keyLabel,
  leaveAccess,
  parsePasskeys,
  parseSessions,
  passkeyAdded,
  passkeyNote,
  passwordAction,
  renderAccess,
  revokedToast,
  sessionLabels,
  type PasskeyRow,
  type SessionRow,
} from "./access.ts";

const origFetch = globalThis.fetch;
const origNav = globalThis.navigator;
const origSetInterval = globalThis.setInterval;
const origClearInterval = globalThis.clearInterval;

const PK: PasskeyRow = { id: "0a0b0c0d", created: "2026-01-01T00:00:00Z" };
const PK2: PasskeyRow = { id: "5e6f7a8b", created: "2026-06-14T14:03:00Z" };

const SESSIONS: SessionRow[] = [
  {
    id: "sess-this",
    kind: "console",
    label: "This browser",
    created: "2026-01-01T00:00:00Z",
    last_seen: "2026-01-02T00:00:00Z",
    current: true,
  },
  {
    id: "sess-ff",
    kind: "console",
    label: "Firefox · nuc-k3s",
    created: "2026-01-01T00:00:00Z",
    last_seen: "2026-01-02T12:00:00Z",
    current: false,
  },
  {
    id: "sess-nuc",
    kind: "device",
    label: "nuc",
    created: "2026-01-01T00:00:00Z",
    last_seen: "2026-01-02T12:00:00Z",
    current: false,
  },
];

type Call = { method: string; url: string; body?: unknown };

type Cap = {
  flash: string[];
  signOut: number;
  loadSession: number;
};

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), { status });
}

function sessionInfo(q: { has_password?: boolean; has_passkey?: boolean } = {}): SessionInfo {
  return {
    email: "a@b.c",
    session_id: "s1",
    has_passkey: q.has_passkey ?? true,
    has_password: q.has_password ?? false,
  };
}

function makeState(session: SessionInfo | undefined = sessionInfo()): AppState {
  return {
    path: signal("/access"),
    email: signal(session?.email ?? ""),
    password: signal(""),
    error: signal<string | undefined>(undefined),
    pending: signal(false),
    session: signal(session),
    method: signal<AuthMethod | undefined>(undefined),
    different: signal(false),
    revealPassword: signal(false),
    userCode: signal(""),
    eph: signal(""),
    counts: signal<NavCounts>({}),
    toast: signal(""),
  };
}

function makeHost(cap: Cap): Host {
  return {
    navigate() {},
    redraw() {},
    flash(message) {
      cap.flash.push(message);
    },
    async signOut() {
      cap.signOut += 1;
    },
    async loadSession() {
      cap.loadSession += 1;
    },
    actions: document.createElement("div"),
  };
}

function cap(): Cap {
  return { flash: [], signOut: 0, loadSession: 0 };
}

function installFetch(impl: (call: Call) => Response | Promise<Response>): Call[] {
  const calls: Call[] = [];
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = reqUrl(input);
    const method = String(init?.method ?? "GET");
    let body: unknown;
    if (typeof init?.body === "string") {
      try {
        body = JSON.parse(init.body) as unknown;
      } catch {
        body = init.body;
      }
    }
    const call: Call = { method, url, body };
    calls.push(call);
    return impl(call);
  }) as unknown as typeof fetch;
  return calls;
}

function lists(passkeys: PasskeyRow[], sessions: SessionRow[] = SESSIONS): Call[] {
  return installFetch((c) => {
    if (c.method === "GET" && c.url === passkeysUrl()) {
      return json({ passkeys });
    }
    if (c.method === "GET" && c.url === sessionsUrl()) {
      return json({ sessions });
    }
    return json({ ok: true });
  });
}

async function settled(): Promise<void> {
  for (let i = 0; i < 20; i++) {
    await Promise.resolve();
  }
  await new Promise<void>((r) => setTimeout(r, 0));
}

async function waitFor(pred: () => boolean, ms = 4000): Promise<void> {
  const t0 = Date.now();
  while (!pred()) {
    if (Date.now() - t0 > ms) {
      throw new Error("waitFor timeout");
    }
    await Bun.sleep(15);
  }
}

function mockCreate(prf: Uint8Array, raw: Uint8Array): void {
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    value: {
      credentials: {
        create: async () => ({
          id: "cred",
          type: "public-key",
          rawId: raw.buffer,
          response: {
            clientDataJSON: new Uint8Array([1]).buffer,
            attestationObject: new Uint8Array([2]).buffer,
          },
          getClientExtensionResults: () => ({
            prf: { results: { first: prf.buffer } },
          }),
        }),
      },
    },
  });
}

let state: AppState | undefined;
const ticks: unknown[] = [];
const cleared: unknown[] = [];

beforeEach(async () => {

  await keyholder.start();
  ticks.length = 0;
  cleared.length = 0;
  globalThis.setInterval = ((handler: TimerHandler, timeout?: number) => {
    const id = origSetInterval(handler, timeout);
    if (timeout === KEY_TICK_MS) {
      ticks.push(id);
    }
    return id;
  }) as typeof setInterval;
  globalThis.clearInterval = ((id: string | number | ReturnType<typeof setInterval>) => {
    if (ticks.includes(id)) {
      cleared.push(id);
    }
    origClearInterval(id);
  }) as typeof clearInterval;
});

afterEach(() => {
  if (state !== undefined) {
    leaveAccess(state);
    state = undefined;
  }
  globalThis.fetch = origFetch;
  Object.defineProperty(globalThis, "navigator", {
    configurable: true,
    writable: true,
    value: origNav,
  });
  globalThis.setInterval = origSetInterval;
  globalThis.clearInterval = origClearInterval;
  clearDek();
});

describe("access helpers", () => {
  test("parsePasskeys and parseSessions skip empty ids", () => {
    expect(
      parseSessions({
        sessions: [
          { id: "sess-this", kind: "console", label: "here", current: true },
          { id: "" },
          { kind: "device" },
        ],
      }),
    ).toEqual([
      {
        id: "sess-this",
        kind: "console",
        label: "here",
        created: "",
        last_seen: "",
        current: true,
      },
    ]);
    expect(parsePasskeys({ passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }, {}] })).toEqual(
      [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    );
    expect(parsePasskeys({ passkeys: "nope" })).toEqual([]);
    expect(parseSessions({})).toEqual([]);
  });

  test("consoleRows keeps browser sessions; note and password action follow the factor count", () => {
    expect(consoleRows(SESSIONS).map((s) => s.id)).toEqual(["sess-this", "sess-ff"]);
    expect(passkeyNote(1, false)).toBe(PASSKEY_NOTE_LAST);
    expect(passkeyNote(2, false)).toBe(PASSKEY_NOTE_OK);
    expect(passkeyNote(1, true)).toBe(PASSKEY_NOTE_OK);
    expect(passwordAction(false)).toBe("Set password");
    expect(passwordAction(true)).toBe("Change password");
  });

  test("row labels, key remaining, revoke toasts, and fail sentences", () => {
    expect(passkeyAdded(PK).startsWith("added ")).toBe(true);
    const now = Date.parse("2026-01-02T00:00:10Z");
    const labels = sessionLabels(SESSIONS[0]!, now);
    expect(labels.signedIn.startsWith("signed in ")).toBe(true);
    expect(labels.lastSeen.startsWith("last seen ")).toBe(true);
    expect(keyLabel(0)).toBe(NO_KEY_LABEL);
    expect(keyLabel(11 * 3_600_000 + 42 * 60_000)).toBe("vault key · 11h 42m left");
    expect(revokedToast(SESSIONS[0]!)).toBe(SIGNED_OUT_TOAST);
    expect(revokedToast(SESSIONS[1]!)).toBe("Revoked Firefox · nuc-k3s");
    expect(revokedToast({ ...SESSIONS[1]!, label: "" })).toBe("Revoked sess-ff");
    expect(failSentence(429)).toBe(RATE_SENTENCE);
    expect(failSentence(400, { error: "last factor" })).toBe(LAST_FACTOR_SENTENCE);
    expect(failSentence(500)).toBe(FAIL_SENTENCE);
  });
});

describe("access screen", () => {
  test("passkeys and browser sessions paint from fetched data", async () => {
    lists([PK, PK2], SESSIONS);
    state = makeState(sessionInfo({ has_password: true }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    expect(root.querySelector('.page[data-width="820"] .stack')).not.toBeNull();
    const passkeys = root.querySelector('[data-card="passkeys"]');
    expect(passkeys?.querySelector(".card-title")?.textContent).toBe("Passkeys");
    expect(passkeys?.querySelector('[data-action="add-passkey"]')?.className).toContain("btn-sm");
    expect(passkeys?.querySelector(`[data-passkey-id="${PK.id}"]`)?.className).toContain("cols-passkeys");
    expect(passkeys?.querySelector(`[data-passkey-id="${PK.id}"]`)?.textContent).toContain(PK.id);
    expect(passkeys?.querySelector(`[data-passkey-id="${PK.id}"]`)?.textContent).toContain(
      passkeyAdded(PK),
    );
    expect(passkeys?.querySelector("[data-note]")?.textContent).toBe(PASSKEY_NOTE_OK);
    const sessions = root.querySelector('[data-card="sessions"]');
    expect(sessions?.querySelector(".card-title")?.textContent).toBe("Browser sessions");
    const current = sessions?.querySelector('[data-session-id="sess-this"]');
    expect(current?.className).toContain("cols-sessions");
    expect(current?.textContent).toContain("This browser");
    expect(current?.querySelector(".badge-ok")?.textContent).toBe("current");
    expect(current?.textContent).toContain("signed in ");
    expect(current?.textContent).toContain("last seen ");
    expect(sessions?.querySelector('[data-session-id="sess-ff"]')?.textContent).toContain(
      "Firefox · nuc-k3s",
    );
    expect(sessions?.querySelector('[data-session-id="sess-nuc"]')).toBeNull();
    expect(root.querySelector('[data-card="password"] .card-title')?.textContent).toBe("Password");
    expect(root.querySelector("[data-password-sub]")?.textContent).toBe(PASSWORD_SUB);
    expect(root.querySelector('[data-action="set-password"]')?.textContent).toBe("Change password");
    const key = root.querySelector('[data-card="key"]');
    expect(key?.classList.contains("card-plain")).toBe(true);
    expect(key?.querySelector(".card-title")?.textContent).toBe(KEY_TITLE);
    expect(key?.querySelector(".card-text")?.textContent).toBe(KEY_BODY);
    expect(key?.querySelector("[data-key-label]")?.textContent).toBe(NO_KEY_LABEL);
  });

  test("loading copy while the lists are in flight", async () => {
    installFetch(() => new Promise<Response>(() => {}));
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    expect(root.textContent).toContain(LOADING_PASSKEYS);
    expect(root.textContent).toContain(LOADING_SESSIONS);
  });

  test("empty copy when both lists are empty", async () => {
    lists([], []);
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    expect(root.textContent).toContain(EMPTY_PASSKEYS);
    expect(root.textContent).toContain(EMPTY_SESSIONS);
  });

  test("Remove is disabled and sunken on the last factor", async () => {
    state = makeState(sessionInfo({ has_password: false }));
    const root = document.createElement("div");
    const calls = lists([PK], SESSIONS);
    renderAccess(state, root, makeHost(cap()));
    await settled();
    const remove = root.querySelector('[data-action="remove"]') as HTMLButtonElement;
    expect(remove.hasAttribute("disabled")).toBe(true);
    expect(remove.className.split(" ")).toContain("btn-sm");
    expect(remove.className.split(" ")).not.toContain("btn-danger");
    expect(root.querySelector("[data-note]")?.textContent).toBe(PASSKEY_NOTE_LAST);
    remove.click();
    await settled();
    expect(calls.some((c) => c.method === "DELETE")).toBe(false);
  });

  test("Remove is enabled when a password remains", async () => {
    lists([PK], SESSIONS);
    state = makeState(sessionInfo({ has_password: true }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    const remove = root.querySelector('[data-action="remove"]') as HTMLButtonElement;
    expect(remove.hasAttribute("disabled")).toBe(false);
    expect(remove.className).toContain("btn-danger");
    expect(root.querySelector("[data-note]")?.textContent).toBe(PASSKEY_NOTE_OK);
  });

  test("Add passkey posts handle, credential, wrap and never a DEK", async () => {
    const dek = mintDek();
    const dekHex = toHex(dek);
    setDek(dek);
    const prf = new Uint8Array(32).fill(9);
    const raw = new Uint8Array([10, 11, 12, 13]);
    mockCreate(prf, raw);
    const bodies: unknown[] = [];
    const hostCap = cap();
    installFetch((c) => {
      if (c.method === "POST" && c.url === passkeyRegisterStartUrl()) {
        return json({ handle: "h1", publicKey: { challenge: "AQIDBA" } });
      }
      if (c.method === "POST" && c.url === passkeyRegisterFinishUrl()) {
        bodies.push(c.body);
        return json({ ok: true });
      }
      if (c.method === "GET" && c.url === passkeysUrl()) {
        return json({ passkeys: [PK] });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [] });
      }
      return json({ ok: true });
    });
    state = makeState(sessionInfo({ has_password: true }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(hostCap));
    await settled();
    (root.querySelector('[data-action="add-passkey"]') as HTMLButtonElement).click();
    await waitFor(() => hostCap.flash.includes(PASSKEY_ADDED_TOAST));
    const body = bodies[0] as Record<string, unknown>;
    expect(Object.keys(body).sort()).toEqual(["credential", "handle", "wrap"]);
    expect(body["handle"]).toBe("h1");
    expect(body["credential"]).toBeDefined();
    const wrap = body["wrap"] as Record<string, unknown>;
    expect(wrap["factor"]).toBe("passkey");
    expect(wrap["cred_id"]).toBe("0a0b0c0d");
    expect(typeof wrap["blob"]).toBe("string");
    expect(wrap["dek"]).toBeUndefined();
    expect(JSON.stringify(body)).not.toContain(dekHex);
    expect(hostCap.flash).toContain(PASSKEY_ADDED_TOAST);
    expect(hostCap.loadSession).toBeGreaterThanOrEqual(1);
  });

  test("Add passkey without a DEK paints NO_DEK_SENTENCE", async () => {
    lists([PK], []);
    state = makeState(sessionInfo({ has_password: true }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    (root.querySelector('[data-action="add-passkey"]') as HTMLButtonElement).click();
    await settled();
    expect(root.querySelector("[data-error]")?.textContent).toBe(NO_DEK_SENTENCE);
  });

  test("password register body is email, password, wrap", async () => {
    const dek = mintDek();
    const dekHex = toHex(dek);
    setDek(dek);
    const bodies: unknown[] = [];
    const hostCap = cap();
    installFetch((c) => {
      if (c.method === "POST" && c.url === passwordRegisterUrl()) {
        bodies.push(c.body);
        return json({ ok: true });
      }
      if (c.method === "GET" && c.url === passkeysUrl()) {
        return json({ passkeys: [PK] });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [] });
      }
      return json({ ok: true });
    });
    state = makeState(sessionInfo({ has_password: false }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(hostCap));
    await settled();
    expect(root.querySelector('[data-action="set-password"]')?.textContent).toBe("Set password");
    const form = root.querySelector('[data-form="password"]') as HTMLFormElement;
    const input = form.querySelector('[data-field="password"]') as HTMLInputElement;
    input.value = "password1234";
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await waitFor(() => hostCap.flash.includes(PASSWORD_SET_TOAST));
    const body = bodies[0] as Record<string, unknown>;
    expect(body["email"]).toBe("a@b.c");
    expect(body["password"]).toBe("password1234");
    const wrap = body["wrap"] as Record<string, unknown>;
    expect(wrap["factor"]).toBe("password");
    expect(typeof wrap["salt"]).toBe("string");
    expect(typeof wrap["blob"]).toBe("string");
    expect(wrap["dek"]).toBeUndefined();
    expect(JSON.stringify(body)).not.toContain(dekHex);
    expect(input.value).toBe("");
    expect(hostCap.loadSession).toBeGreaterThanOrEqual(1);
  });

  test("a short password paints the length sentence and does not POST", async () => {
    const calls = lists([PK], []);
    setDek(mintDek());
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    const form = root.querySelector('[data-form="password"]') as HTMLFormElement;
    (form.querySelector('[data-field="password"]') as HTMLInputElement).value = "short";
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await settled();
    expect(root.querySelector("[data-error]")?.textContent).toBe(PASSWORD_LENGTH_SENTENCE);
    expect(calls.some((c) => c.url === passwordRegisterUrl())).toBe(false);
  });

  test("revoking the current session toasts and signs out", async () => {
    const hostCap = cap();
    const calls = lists([PK], SESSIONS);
    state = makeState(sessionInfo({ has_password: true }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(hostCap));
    await settled();
    (root.querySelector('[data-session-id="sess-this"] [data-action="revoke"]') as HTMLButtonElement).click();
    await waitFor(() => hostCap.signOut > 0);
    expect(calls).toContainEqual({
      method: "DELETE",
      url: sessionRevokePath("sess-this"),
      body: undefined,
    });
    expect(hostCap.flash).toContain(SIGNED_OUT_TOAST);
    expect(hostCap.signOut).toBe(1);
  });

  test("revoking another session toasts the label and refreshes", async () => {
    const hostCap = cap();
    const calls = lists([PK], SESSIONS);
    state = makeState(sessionInfo({ has_password: true }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(hostCap));
    await settled();
    const gets = calls.filter((c) => c.method === "GET" && c.url === sessionsUrl()).length;
    (root.querySelector('[data-session-id="sess-ff"] [data-action="revoke"]') as HTMLButtonElement).click();
    await waitFor(() => hostCap.flash.includes("Revoked Firefox · nuc-k3s"));
    expect(calls).toContainEqual({
      method: "DELETE",
      url: sessionRevokePath("sess-ff"),
      body: undefined,
    });
    expect(hostCap.signOut).toBe(0);
    expect(calls.filter((c) => c.method === "GET" && c.url === sessionsUrl()).length).toBeGreaterThan(
      gets,
    );
  });

  test("401 on passkeys signs the tab out", async () => {
    const hostCap = cap();
    installFetch((c) => {
      if (c.method === "GET" && c.url === passkeysUrl()) {
        return json({}, 401);
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [] });
      }
      return json({});
    });
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(hostCap));
    await waitFor(() => hostCap.signOut > 0);
    expect(hostCap.signOut).toBe(1);
  });

  test("403 on sessions signs the tab out", async () => {
    const hostCap = cap();
    installFetch((c) => {
      if (c.method === "GET" && c.url === passkeysUrl()) {
        return json({ passkeys: [PK] });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({}, 403);
      }
      return json({});
    });
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(hostCap));
    await waitFor(() => hostCap.signOut > 0);
    expect(hostCap.signOut).toBe(1);
  });

  test("a last-factor DELETE paints LAST_FACTOR_SENTENCE", async () => {
    installFetch((c) => {
      if (c.method === "GET" && c.url === passkeysUrl()) {
        return json({ passkeys: [PK, PK2] });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [] });
      }
      if (c.method === "DELETE" && c.url === passkeyDeletePath(PK.id)) {
        return json({ error: "last factor" }, 400);
      }
      return json({});
    });
    state = makeState(sessionInfo({ has_password: false }));
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    (root.querySelector(`[data-passkey-id="${PK.id}"] [data-action="remove"]`) as HTMLButtonElement).click();
    await waitFor(() => root.querySelector("[data-error]") !== null);
    expect(root.querySelector("[data-error]")?.textContent).toBe(LAST_FACTOR_SENTENCE);
  });

  test("a stale passkeys response after leave does not paint", async () => {
    let release: ((r: Response) => void) | undefined;
    installFetch((c) => {
      if (c.method === "GET" && c.url === passkeysUrl()) {
        return new Promise<Response>((resolve) => {
          release = resolve;
        });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [] });
      }
      return json({});
    });
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    leaveAccess(state);
    release?.(json({ passkeys: [{ id: "late-id", created: "2026-01-01T00:00:00Z" }] }));
    await settled();
    expect(root.textContent).not.toContain("late-id");
    state = undefined;
  });

  test("leaveAccess clears the vault-key timer", async () => {
    lists([], []);
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await settled();
    expect(ticks.length).toBe(1);
    leaveAccess(state);
    expect(cleared).toEqual(ticks);
    state = undefined;
  });

  test("a failed sessions GET shows FAIL_SENTENCE and not the empty copy", async () => {
    installFetch((c) => {
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({}, 500);
      }
      if (c.method === "GET" && c.url === passkeysUrl()) {
        return json({ passkeys: [PK] });
      }
      return json({});
    });
    state = makeState();
    const root = document.createElement("div");
    renderAccess(state, root, makeHost(cap()));
    await waitFor(() => root.querySelector("[data-error]") !== null);
    expect(root.querySelector("[data-error]")?.textContent).toBe(FAIL_SENTENCE);
    expect(root.textContent).not.toContain(EMPTY_SESSIONS);
  });
});
