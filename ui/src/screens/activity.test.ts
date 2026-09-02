import { afterEach, describe, expect, test } from "bun:test";
import { auditUrl } from "../lib/api.ts";
import { bumpLogoutGen } from "../lib/gen.ts";
import type { AppState, AuthMethod, Host, NavCounts } from "../lib/host.ts";
import { signal } from "../lib/signal.ts";
import { shortHash } from "../lib/time.ts";
import {
  BROKEN_TITLE,
  EM_DASH,
  FOOT,
  LOAD_FAIL_SENTENCE,
  LOADING_SENTENCE,
  UNVERIFIED,
  VERIFIED_TITLE,
  ZERO_HASH,
  actionBadgeClass,
  auditRow,
  leaveActivity,
  namesLabel,
  parseAudit,
  renderActivity,
  sessionLabel,
  type AuditEvent,
} from "./activity.ts";

const origFetch = globalThis.fetch;

const HEAD = `a91f${"0".repeat(56)}7c4e`;
const FIRST_HASH = "5157a919d53b19d0f49086fda3682a0b1e8dee4b7201a3f0bedfe927e41bfc84";
const SECRET = "secret-value-must-not-render";

const PUT: AuditEvent = {
  seq: 1,
  action: "vault.put",
  names: ["kv/a"],
  sessionId: "s1",
  prev: ZERO_HASH,
  hash: FIRST_HASH,
};

const REVOKE: AuditEvent = {
  seq: 2,
  action: "session.revoke",
  names: [],
  sessionId: undefined,
  prev: FIRST_HASH,
  hash: HEAD,
};

const ROLLBACK: AuditEvent = {
  seq: 3,
  action: "vault.rollback",
  names: ["kv/a", "kv/b"],
  sessionId: "s1",
  prev: HEAD,
  hash: "b".repeat(64),
};

const DEVICE: AuditEvent = {
  seq: 4,
  action: "device.approve",
  names: ["nuc"],
  sessionId: "dev-1",
  prev: "b".repeat(64),
  hash: "c".repeat(64),
};

type Call = { method: string; url: string; body?: unknown };

type Cap = { flash: string[]; signOut: number };

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), { status });
}

function wireEvent(ev: AuditEvent, extra: Record<string, unknown> = {}): Record<string, unknown> {
  const row: Record<string, unknown> = {
    seq: ev.seq,
    action: ev.action,
    names: ev.names,
    prev: ev.prev,
    hash: ev.hash,
    ...extra,
  };
  if (ev.sessionId !== undefined) {
    row["session_id"] = ev.sessionId;
  }
  return row;
}

function chainBody(
  events: readonly AuditEvent[],
  q: { verified?: boolean; head?: string; extra?: Record<string, unknown> } = {},
): unknown {
  return {
    events: events.map((ev) => wireEvent(ev, q.extra ?? {})),
    head: q.head ?? (events.at(-1)?.hash ?? ZERO_HASH),
    verified: q.verified ?? true,
  };
}

function makeState(): AppState {
  return {
    path: signal("/activity"),
    email: signal("ops@imabee.com"),
    password: signal(""),
    error: signal<string | undefined>(undefined),
    pending: signal(false),
    session: signal({
      email: "ops@imabee.com",
      session_id: "s1",
      has_passkey: true,
      has_password: false,
    }),
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
    async loadSession() {},
    actions: document.createElement("div"),
  };
}

function cap(): Cap {
  return { flash: [], signOut: 0 };
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

let state: AppState | undefined;

afterEach(() => {
  if (state !== undefined) {
    leaveActivity(state);
    state = undefined;
  }
  globalThis.fetch = origFetch;
});

describe("audit parse and row mapping", () => {
  test("parseAudit reads seq/action/names/session/prev/hash/head/verified and drops value", () => {
    const parsed = parseAudit({
      events: [
        {
          seq: 1,
          action: "vault.put",
          names: ["kv/a"],
          session_id: "s1",
          prev: ZERO_HASH,
          hash: FIRST_HASH,
          value: SECRET,
        },
        { seq: 2, action: "session.revoke", names: [], prev: FIRST_HASH, hash: HEAD },
      ],
      head: HEAD,
      verified: true,
    });
    expect(parsed).toEqual({
      events: [PUT, REVOKE],
      head: HEAD,
      verified: true,
    });
    expect(JSON.stringify(parsed)).not.toContain(SECRET);
    expect(parseAudit({})).toBeUndefined();
    expect(parseAudit({ events: "nope" })).toBeUndefined();
    expect(parseAudit({ events: [], verified: false })).toEqual({
      events: [],
      head: ZERO_HASH,
      verified: false,
    });
  });

  test("row mapping joins names, dashes a missing session, and tones the action badge", () => {
    expect(namesLabel(["kv/a", "kv/b"])).toBe("kv/a, kv/b");
    expect(namesLabel([])).toBe(EM_DASH);
    expect(sessionLabel("s1")).toBe("s1");
    expect(sessionLabel(undefined)).toBe(EM_DASH);
    expect(sessionLabel("")).toBe(EM_DASH);
    expect(actionBadgeClass("vault.put")).toBe("badge");
    expect(actionBadgeClass("provider.put")).toBe("badge");
    expect(actionBadgeClass("vault.rollback")).toBe("badge badge-warn");
    expect(actionBadgeClass("session.revoke")).toBe("badge badge-accent");
    expect(actionBadgeClass("device.approve")).toBe("badge badge-accent");
    expect(auditRow(REVOKE)).toEqual({
      seq: "2",
      action: "session.revoke",
      badgeClass: "badge badge-accent",
      names: EM_DASH,
      session: EM_DASH,
    });
    expect(auditRow(ROLLBACK).names).toBe("kv/a, kv/b");
  });
});

describe("Activity screen", () => {
  test("loading copy while GET /api/v1/audit is in flight", async () => {
    installFetch(() => new Promise<Response>(() => {}));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await settled();
    expect(root.textContent).toContain(LOADING_SENTENCE);
    expect(root.querySelector('.page[data-width="1000"][data-state="loading"]')).not.toBeNull();
  });

  test("paints seq/action/names/session in API order and never a secret value", async () => {
    const calls = installFetch((c) => {
      if (c.method === "GET" && c.url === auditUrl()) {
        return json(chainBody([PUT, REVOKE, ROLLBACK, DEVICE], { extra: { value: SECRET } }));
      }
      return json({}, 404);
    });
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await waitFor(() => root.querySelector("[data-seq='1']") !== null);
    expect(calls).toEqual([{ method: "GET", url: "/api/v1/audit", body: undefined }]);
    expect(root.querySelector('.page[data-width="1000"]')).not.toBeNull();
    expect(root.querySelector(".grid.grid-head.cols-audit")?.textContent).toBe("SeqActionNamesSession");
    const seqs = [...root.querySelectorAll("[data-seq]")].map((n) => n.getAttribute("data-seq"));
    expect(seqs).toEqual(["1", "2", "3", "4"]);
    const put = root.querySelector('[data-seq="1"]');
    expect(put?.textContent).toContain("1");
    expect(put?.textContent).toContain("vault.put");
    expect(put?.textContent).toContain("kv/a");
    expect(put?.textContent).toContain("s1");
    expect(put?.querySelector(".badge")?.className).toBe("badge");
    const revoke = root.querySelector('[data-seq="2"]');
    expect(revoke?.textContent).toContain("session.revoke");
    expect(revoke?.textContent).toContain(EM_DASH);
    expect(revoke?.querySelector(".badge-accent")).not.toBeNull();
    expect(root.querySelector('[data-seq="3"] .badge-warn')?.textContent).toBe("vault.rollback");
    expect(root.querySelector('[data-seq="3"]')?.textContent).toContain("kv/a, kv/b");
    expect(root.querySelector('[data-seq="4"] .badge-accent')?.textContent).toBe("device.approve");
    expect(root.querySelector('[data-seq="4"]')?.textContent).toContain("nuc");
    expect(root.querySelector('[data-seq="4"]')?.textContent).toContain("dev-1");
    expect(root.querySelector(".page-foot")?.textContent).toBe(FOOT);
    expect(root.textContent).not.toContain(SECRET);
    expect(root.textContent).not.toContain(FIRST_HASH);
    expect(root.textContent).not.toContain(HEAD);
    expect(state.counts.get().activity).toBe(4);
  });

  test("verified banner shows the count and shortened head", async () => {
    installFetch(() => json(chainBody([PUT, REVOKE], { head: HEAD, verified: true })));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await waitFor(() => root.querySelector(".verified-bar") !== null);
    const bar = root.querySelector(".verified-bar");
    expect(bar?.getAttribute("data-verified")).toBe("true");
    expect(bar?.getAttribute("data-chain-status")).toBe("verified");
    expect(bar?.querySelector(".verified-title")?.textContent).toBe(VERIFIED_TITLE);
    expect(bar?.querySelector(".verified-sub")?.textContent).toBe(`2 events · head ${shortHash(HEAD)}`);
    expect(shortHash(HEAD)).toBe("a91f…7c4e");
  });

  test("a broken chain uses the danger banner and UNVERIFIED copy", async () => {
    installFetch(() => json(chainBody([PUT], { verified: false, head: FIRST_HASH })));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await waitFor(() => root.querySelector('[data-verified="false"]') !== null);
    const bar = root.querySelector(".verified-bar");
    expect(bar?.getAttribute("data-chain-status")).toBe("unverified");
    expect(bar?.querySelector(".verified-title")?.textContent).toBe(BROKEN_TITLE);
    expect(bar?.querySelector(".verified-sub")?.textContent).toBe(
      `${UNVERIFIED}. 1 events · head ${shortHash(FIRST_HASH)}`,
    );
    expect(root.textContent).toContain(UNVERIFIED);
  });

  test("an empty verified chain still shows the banner, headers and foot", async () => {
    installFetch(() => json({ events: [], head: ZERO_HASH, verified: true }));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await waitFor(() => root.querySelector(".verified-bar") !== null);
    expect(root.querySelector(".verified-title")?.textContent).toBe(VERIFIED_TITLE);
    expect(root.querySelector(".verified-sub")?.textContent).toBe(
      `0 events · head ${shortHash(ZERO_HASH)}`,
    );
    expect(shortHash(ZERO_HASH)).toBe("0000…0000");
    expect(root.querySelector(".grid.grid-head.cols-audit")).not.toBeNull();
    expect(root.querySelector("[data-seq]")).toBeNull();
    expect(root.querySelector(".page-foot")?.textContent).toBe(FOOT);
    expect(state.counts.get().activity).toBe(0);
  });

  test("401 signs the tab out", async () => {
    const hostCap = cap();
    installFetch(() => json({}, 401));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(hostCap));
    await waitFor(() => hostCap.signOut > 0);
    expect(hostCap.signOut).toBe(1);
    expect(root.querySelector("[data-error]")).toBeNull();
  });

  test("403 signs the tab out", async () => {
    const hostCap = cap();
    installFetch(() => json({}, 403));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(hostCap));
    await waitFor(() => hostCap.signOut > 0);
    expect(hostCap.signOut).toBe(1);
  });

  test("a failed GET paints the load-fail sentence", async () => {
    installFetch(() => json({}, 500));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await waitFor(() => root.querySelector("[data-error]") !== null);
    expect(root.querySelector(".alert-danger[role='alert']")?.textContent).toBe(LOAD_FAIL_SENTENCE);
    expect(root.querySelector(".verified-bar")).toBeNull();
  });

  test("a malformed body is a load failure", async () => {
    installFetch(() => json({ nope: true }));
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await waitFor(() => root.querySelector("[data-error]") !== null);
    expect(root.textContent).toContain(LOAD_FAIL_SENTENCE);
  });

  test("an in-flight GET is ignored after leave", async () => {
    let finish: ((value: Response) => void) | undefined;
    installFetch(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve;
        }),
    );
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await settled();
    expect(root.textContent).toContain(LOADING_SENTENCE);
    leaveActivity(state);
    finish?.(json(chainBody([PUT, REVOKE])));
    await settled();
    expect(root.textContent).toContain(LOADING_SENTENCE);
    expect(root.querySelector("[data-seq]")).toBeNull();
    expect(root.textContent).not.toContain("vault.put");
    state = undefined;
  });

  test("a response after the logout generation moves is dropped", async () => {
    let finish: ((value: Response) => void) | undefined;
    installFetch(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve;
        }),
    );
    state = makeState();
    const root = document.createElement("div");
    renderActivity(state, root, makeHost(cap()));
    await settled();
    bumpLogoutGen();
    finish?.(json(chainBody([PUT])));
    await settled();
    expect(root.textContent).toContain(LOADING_SENTENCE);
    expect(root.querySelector("[data-seq]")).toBeNull();
    expect(state.counts.get().activity).toBeUndefined();
  });
});
