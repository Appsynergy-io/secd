import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  FAIL_SENTENCE,
  RATE_SENTENCE,
  devicePendingUrl,
  sessionRevokePath,
  sessionsUrl,
} from "../lib/api.ts";
import { asButton } from "../lib/dom.ts";
import { bumpLogoutGen } from "../lib/gen.ts";
import type { AppState, AuthMethod, Host, NavCounts, SessionInfo } from "../lib/host.ts";
import { signal } from "../lib/signal.ts";
import { ago, countdown, dayLabel } from "../lib/time.ts";
import {
  DEVICES_HINT,
  DEVICES_TITLE,
  EMPTY_SENTENCE,
  LOADING_SENTENCE,
  OPEN_LABEL,
  PENDING_LABEL,
  POLL_MS,
  deviceSessions,
  failSentence,
  leaveDevices,
  parsePending,
  parseSessions,
  pendingMeta,
  pendingSentence,
  renderDevices,
  revokedToast,
  sessionLabel,
  type PendingRequest,
  type SessionRow,
} from "./devices.ts";

const origFetch = globalThis.fetch;
const origSetInterval = globalThis.setInterval;
const origClearInterval = globalThis.clearInterval;

const CODE = "K4T7-QM92";
const EPH = "ab".repeat(32);
const CREATED = "2026-08-28T09:12:00Z";
const HOSTNAME = "thinkpad-x1";

const PENDING: PendingRequest = {
  user_code: CODE,
  hostname: HOSTNAME,
  eph_pub: EPH,
  created: CREATED,
  expires_in: 260,
};

const DEVICE_A: SessionRow = {
  id: "dev-a",
  kind: "device",
  label: "nuc-k3s",
  created: "2026-07-28T00:00:00Z",
  last_seen: "2026-08-28T09:00:00Z",
  current: false,
};

const DEVICE_B: SessionRow = {
  id: "dev-b",
  kind: "device",
  label: "thinkpad-x1",
  created: "2026-08-14T00:00:00Z",
  last_seen: "2026-08-28T09:10:00Z",
  current: false,
};

const CONSOLE: SessionRow = {
  id: "sess-this",
  kind: "console",
  label: "This browser",
  created: "2026-01-01T00:00:00Z",
  last_seen: "2026-01-02T00:00:00Z",
  current: true,
};

type Call = { method: string; url: string; body?: unknown };

type Cap = { flash: string[]; signOut: number; navs: string[] };

type FakeTimer = { id: number; timeout: number; handler: () => void };

const fakes = new Map<number, FakeTimer>();
let nextFake = 1_000_000;

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), { status });
}

function sessionInfo(): SessionInfo {
  return {
    email: "ops@imabee.com",
    session_id: "s1",
    has_passkey: true,
    has_password: false,
  };
}

function makeState(): AppState {
  return {
    path: signal("/devices"),
    email: signal("ops@imabee.com"),
    password: signal(""),
    error: signal<string | undefined>(undefined),
    pending: signal(false),
    session: signal(sessionInfo()),
    method: signal<AuthMethod | undefined>(undefined),
    different: signal(false),
    revealPassword: signal(false),
    userCode: signal(""),
    eph: signal(""),
    counts: signal<NavCounts>({}),
    toast: signal(""),
  };
}

function makeHost(c: Cap): Host {
  return {
    navigate(to) {
      c.navs.push(to);
    },
    redraw() {},
    flash(message) {
      c.flash.push(message);
    },
    async signOut() {
      c.signOut += 1;
    },
    async loadSession() {},
    actions: document.createElement("div"),
  };
}

function cap(): Cap {
  return { flash: [], signOut: 0, navs: [] };
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

function lists(
  pending: PendingRequest[] = [PENDING],
  sessions: SessionRow[] = [CONSOLE, DEVICE_A, DEVICE_B],
): Call[] {
  return installFetch((c) => {
    if (c.method === "GET" && c.url === devicePendingUrl()) {
      return json({ pending });
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

let state: AppState | undefined;

beforeEach(() => {
  fakes.clear();
  nextFake = 1_000_000;
  globalThis.setInterval = ((handler: TimerHandler, timeout?: number) => {
    if (timeout === POLL_MS && typeof handler === "function") {
      const id = nextFake++;
      fakes.set(id, { id, timeout, handler: () => (handler as () => void)() });
      return id as unknown as ReturnType<typeof setInterval>;
    }
    return origSetInterval(handler, timeout);
  }) as typeof setInterval;
  globalThis.clearInterval = ((id: string | number | ReturnType<typeof setInterval>) => {
    if (typeof id === "number" && fakes.delete(id)) {
      return;
    }
    origClearInterval(id as ReturnType<typeof setInterval>);
  }) as typeof clearInterval;
});

afterEach(() => {
  if (state !== undefined) {
    leaveDevices(state);
    state = undefined;
  }
  document.body.replaceChildren();
  globalThis.fetch = origFetch;
  globalThis.setInterval = origSetInterval;
  globalThis.clearInterval = origClearInterval;
});

describe("devices helpers", () => {
  test("parsePending keeps named rows and drops the rest", () => {
    expect(
      parsePending({
        pending: [
          {
            user_code: CODE,
            hostname: HOSTNAME,
            eph_pub: EPH,
            created: CREATED,
            expires_in: 260,
          },
          { hostname: "x" },
          "nope",
        ],
      }),
    ).toEqual([PENDING]);
    expect(parsePending({})).toEqual([]);
    expect(parsePending(null)).toEqual([]);
  });

  test("deviceSessions keeps kind=device only", () => {
    expect(deviceSessions([CONSOLE, DEVICE_A, DEVICE_B]).map((s) => s.id)).toEqual([
      "dev-a",
      "dev-b",
    ]);
    expect(parseSessions({ sessions: [CONSOLE, DEVICE_A, { kind: "device" }] })).toEqual([
      CONSOLE,
      DEVICE_A,
    ]);
  });

  test("pending meta, labels, toasts, and fail sentences", () => {
    const now = Date.parse("2026-08-28T09:12:40Z");
    expect(pendingMeta(PENDING, now)).toBe(
      `${HOSTNAME} · requested ${ago(CREATED, now)} · expires in ${countdown(260)}`,
    );
    expect(pendingSentence(1)).toBe("1 device is waiting for approval.");
    expect(pendingSentence(3)).toBe("3 devices are waiting for approval.");
    expect(sessionLabel(DEVICE_A)).toBe("nuc-k3s");
    expect(sessionLabel({ ...DEVICE_A, label: "" })).toBe("dev-a");
    expect(revokedToast(DEVICE_A)).toBe("Revoked device session nuc-k3s");
    expect(failSentence(429)).toBe(RATE_SENTENCE);
    expect(failSentence(400, { error: "already approved" })).toBe("already approved");
    expect(failSentence(500)).toBe(FAIL_SENTENCE);
  });
});

describe("devices screen", () => {
  test("loading copy while sessions are in flight", async () => {
    installFetch(() => new Promise<Response>(() => {}));
    state = makeState();
    const root = document.createElement("div");
    renderDevices(state, root, makeHost(cap()));
    expect(root.querySelector('.page[data-width="900"] .stack')).not.toBeNull();
    expect(root.textContent).toContain(LOADING_SENTENCE);
    expect(root.querySelector(".card-title")?.textContent).toBe(DEVICES_TITLE);
    expect(root.querySelector(".card-hint")?.textContent).toBe(DEVICES_HINT);
    await settled();
  });

  test("empty copy when there are no device sessions", async () => {
    lists([], [CONSOLE]);
    state = makeState();
    const root = document.createElement("div");
    renderDevices(state, root, makeHost(cap()));
    await settled();
    expect(root.textContent).toContain(EMPTY_SENTENCE);
    expect(root.querySelector(".pending-card")).toBeNull();
    expect(state.counts.get().devices).toBe(0);
  });

  test("paints the pending banner and device rows from fetched data", async () => {
    const calls = lists();
    state = makeState();
    const root = document.createElement("div");
    renderDevices(state, root, makeHost(cap()));
    await settled();
    expect(calls.some((c) => c.method === "GET" && c.url === devicePendingUrl())).toBe(true);
    expect(calls.some((c) => c.method === "GET" && c.url === sessionsUrl())).toBe(true);
    const banner = root.querySelector("[data-pending]");
    expect(banner?.classList.contains("pending-card")).toBe(true);
    expect(banner?.querySelector(".pending-label")?.textContent).toBe(PENDING_LABEL);
    expect(banner?.querySelector(".pending-meta")?.textContent).toBe(pendingSentence(1));
    expect(banner?.textContent).not.toContain(CODE);
    expect(asButton(banner?.querySelector('[data-action="open"]') ?? null)?.textContent).toBe(
      OPEN_LABEL,
    );
    expect(root.querySelector('[data-action="approve"]')).toBeNull();
    expect(root.querySelector('[data-action="deny"]')).toBeNull();
    const devices = root.querySelector('[data-card="devices"]');
    expect(devices?.querySelector(`[data-session-id="${DEVICE_A.id}"]`)?.className).toContain(
      "cols-sessions",
    );
    expect(devices?.querySelector(`[data-session-id="${DEVICE_A.id}"]`)?.textContent).toContain(
      "nuc-k3s",
    );
    expect(devices?.querySelector(`[data-session-id="${DEVICE_A.id}"]`)?.textContent).toContain(
      `approved ${dayLabel(DEVICE_A.created)}`,
    );
    expect(devices?.querySelector(`[data-session-id="${DEVICE_A.id}"]`)?.textContent).toContain(
      `last seen ${ago(DEVICE_A.last_seen)}`,
    );
    expect(devices?.querySelector(`[data-session-id="${CONSOLE.id}"]`)).toBeNull();
    const revoke = asButton(
      devices?.querySelector(`[data-session-id="${DEVICE_A.id}"] [data-action="revoke"]`) ?? null,
    );
    expect(revoke?.className).toContain("btn-sm");
    expect(revoke?.className).toContain("btn-danger");
    expect(revoke?.textContent).toBe("Revoke");
    expect(state.counts.get().devices).toBe(2);
  });

  test("Open approval page navigates to /device and touches no request", async () => {
    const calls = lists();
    state = makeState();
    const root = document.createElement("div");
    const c = cap();
    renderDevices(state, root, makeHost(c));
    await settled();
    asButton(root.querySelector('[data-action="open"]'))?.click();
    await settled();
    expect(c.navs).toEqual(["/device"]);
    expect(calls.some((x) => x.method === "POST")).toBe(false);
    expect(state.userCode.get()).toBe("");
  });

  test("Revoke DELETEs the session and toasts the label", async () => {
    const calls = lists();
    state = makeState();
    const root = document.createElement("div");
    const c = cap();
    renderDevices(state, root, makeHost(c));
    await settled();
    asButton(root.querySelector(`[data-session-id="${DEVICE_A.id}"] [data-action="revoke"]`))?.click();
    await settled();
    expect(
      calls.some((x) => x.method === "DELETE" && x.url === sessionRevokePath(DEVICE_A.id)),
    ).toBe(true);
    expect(c.flash).toEqual([revokedToast(DEVICE_A)]);
    expect(root.querySelector(`[data-session-id="${DEVICE_A.id}"]`)).toBeNull();
    expect(state.counts.get().devices).toBe(1);
  });

  test("revoke 401 signs the tab out", async () => {
    installFetch((c) => {
      if (c.method === "GET" && c.url === devicePendingUrl()) {
        return json({ pending: [] });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [DEVICE_A] });
      }
      if (c.method === "DELETE" && c.url === sessionRevokePath(DEVICE_A.id)) {
        return json({}, 401);
      }
      return json({ ok: true });
    });
    state = makeState();
    const root = document.createElement("div");
    const c = cap();
    renderDevices(state, root, makeHost(c));
    await settled();
    asButton(root.querySelector('[data-action="revoke"]'))?.click();
    await settled();
    expect(c.signOut).toBe(1);
  });

  test("401 on pending GET signs the tab out", async () => {
    installFetch((c) => {
      if (c.method === "GET" && c.url === devicePendingUrl()) {
        return json({}, 401);
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [] });
      }
      return json({ ok: true });
    });
    state = makeState();
    const root = document.createElement("div");
    const c = cap();
    renderDevices(state, root, makeHost(c));
    await settled();
    expect(c.signOut).toBe(1);
  });

  test("401 on sessions GET signs the tab out", async () => {
    installFetch((c) => {
      if (c.method === "GET" && c.url === devicePendingUrl()) {
        return json({ pending: [] });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({}, 401);
      }
      return json({ ok: true });
    });
    state = makeState();
    const root = document.createElement("div");
    const c = cap();
    renderDevices(state, root, makeHost(c));
    await settled();
    expect(c.signOut).toBe(1);
  });

  test("a failed sessions GET shows FAIL_SENTENCE and not the empty copy", async () => {
    installFetch((c) => {
      if (c.method === "GET" && c.url === devicePendingUrl()) {
        return json({ pending: [] });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({}, 500);
      }
      return json({ ok: true });
    });
    state = makeState();
    const root = document.createElement("div");
    renderDevices(state, root, makeHost(cap()));
    await settled();
    expect(root.querySelector(".alert-danger")?.textContent).toBe(FAIL_SENTENCE);
    expect(root.textContent).not.toContain(EMPTY_SENTENCE);
  });

  test("the 5s pending poll is cleared on leave and fires another GET", async () => {
    const calls = lists();
    state = makeState();
    const root = document.createElement("div");
    renderDevices(state, root, makeHost(cap()));
    await settled();
    const poll = [...fakes.values()].find((t) => t.timeout === POLL_MS);
    expect(poll).toBeDefined();
    const n = calls.filter((c) => c.method === "GET" && c.url === devicePendingUrl()).length;
    poll?.handler();
    await settled();
    expect(calls.filter((c) => c.method === "GET" && c.url === devicePendingUrl()).length).toBe(
      n + 1,
    );
    leaveDevices(state);
    expect([...fakes.values()].some((t) => t.timeout === POLL_MS)).toBe(false);
    state = undefined;
  });

  test("leave drops an in-flight pending GET", async () => {
    let finish: ((value: Response) => void) | undefined;
    installFetch((c) => {
      if (c.method === "GET" && c.url === devicePendingUrl()) {
        return new Promise<Response>((resolve) => {
          finish = resolve;
        });
      }
      if (c.method === "GET" && c.url === sessionsUrl()) {
        return json({ sessions: [] });
      }
      return json({ ok: true });
    });
    state = makeState();
    const root = document.createElement("div");
    renderDevices(state, root, makeHost(cap()));
    await settled();
    leaveDevices(state);
    bumpLogoutGen();
    finish?.(json({ pending: [PENDING] }));
    await settled();
    expect(root.querySelector(".pending-card")).toBeNull();
    state = undefined;
  });
});
