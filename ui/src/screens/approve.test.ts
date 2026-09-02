import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  FAIL_SENTENCE,
  NO_DEK_SENTENCE,
  NO_EPH_SENTENCE,
  RATE_SENTENCE,
  deviceApproveUrl,
  deviceDenyUrl,
  devicePendingUrl,
} from "../lib/api.ts";
import {
  clearDek,
  fromHex,
  mintDek,
  open,
  setDek,
  toHex,
  x25519Public,
  x25519Shared,
  zeroizeBytes,
} from "../lib/crypto.ts";
import { asButton } from "../lib/dom.ts";
import { bumpLogoutGen } from "../lib/gen.ts";
import type { AppState, AuthMethod, Host, NavCounts, SessionInfo } from "../lib/host.ts";
import { signal } from "../lib/signal.ts";
import { ago, countdown, keyFingerprint } from "../lib/time.ts";
import {
  APPROVED_BODY,
  APPROVED_TITLE,
  APPROVE_LABEL,
  APPROVE_NOTE,
  BACK_LABEL,
  DENIED_BODY,
  DENIED_TITLE,
  DENY_LABEL,
  FOOT_ROUTE,
  HEAD_TITLE,
  MISSING_BODY,
  MISSING_TITLE,
  NO_SESSION_SENTENCE,
  TICK_MS,
  approveFacts,
  deviceDisabledReason,
  ephOk,
  failSentence,
  leaveApprove,
  renderApprove,
  resultCopy,
  splitCode,
} from "./approve.ts";
import type { PendingRequest } from "./devices.ts";

const origFetch = globalThis.fetch;
const origSetInterval = globalThis.setInterval;
const origClearInterval = globalThis.clearInterval;

const CODE = "K4T7-QM92";
const CREATED = "2026-08-28T09:12:00Z";
const HOSTNAME = "thinkpad-x1";
const EMAIL = "ops@imabee.com";

type Call = { method: string; url: string; body?: unknown };

type Cap = { flash: string[]; signOut: number; navs: string[] };

type FakeTimer = { id: number; timeout: number; handler: () => void };

const fakes = new Map<number, FakeTimer>();
let nextFake = 2_000_000;

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), { status });
}

function deviceSk(): Uint8Array {
  return new Uint8Array(32).fill(0x51);
}

function deviceEphHex(): string {
  return toHex(x25519Public(deviceSk()));
}

function pendingRow(eph = deviceEphHex()): PendingRequest {
  return {
    user_code: CODE,
    hostname: HOSTNAME,
    eph_pub: eph,
    created: CREATED,
    expires_in: 260,
  };
}

function sessionInfo(): SessionInfo {
  return {
    email: EMAIL,
    session_id: "s1",
    has_passkey: true,
    has_password: false,
  };
}

function makeState(q: { code?: string; eph?: string; session?: SessionInfo } = {}): AppState {
  return {
    path: signal("/device"),
    email: signal(EMAIL),
    password: signal(""),
    error: signal<string | undefined>(undefined),
    pending: signal(false),
    session: signal(q.session ?? sessionInfo()),
    method: signal<AuthMethod | undefined>(undefined),
    different: signal(false),
    revealPassword: signal(false),
    userCode: signal(q.code ?? CODE),
    eph: signal(q.eph ?? ""),
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

function apis(
  q: {
    pending?: PendingRequest[];
    pendingStatus?: number;
    approve?: number;
    approveBody?: unknown;
    deny?: number;
    denyBody?: unknown;
    hangApprove?: boolean;
    hangDeny?: boolean;
    finish?: { resolve?: (value: Response) => void };
  } = {},
): Call[] {
  const row = pendingRow();
  return installFetch((c) => {
    if (c.method === "GET" && c.url === devicePendingUrl()) {
      if (q.pendingStatus !== undefined && q.pendingStatus !== 200) {
        return json({ error: FAIL_SENTENCE }, q.pendingStatus);
      }
      return json({ pending: q.pending ?? [row] });
    }
    if (c.method === "POST" && c.url === deviceApproveUrl()) {
      if (q.hangApprove === true) {
        return new Promise<Response>((resolve) => {
          if (q.finish !== undefined) {
            q.finish.resolve = resolve;
          }
        });
      }
      return json(q.approveBody ?? { ok: true }, q.approve ?? 200);
    }
    if (c.method === "POST" && c.url === deviceDenyUrl()) {
      if (q.hangDeny === true) {
        return new Promise<Response>((resolve) => {
          if (q.finish !== undefined) {
            q.finish.resolve = resolve;
          }
        });
      }
      return json(q.denyBody ?? { ok: true }, q.deny ?? 200);
    }
    return json({ ok: true });
  });
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) {
    return false;
  }
  let x = 0;
  for (let i = 0; i < a.length; i++) {
    x |= (a[i] ?? 0) ^ (b[i] ?? 0);
  }
  return x === 0;
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
  nextFake = 2_000_000;
  globalThis.setInterval = ((handler: TimerHandler, timeout?: number) => {
    if (timeout === TICK_MS && typeof handler === "function") {
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
    leaveApprove(state);
    state = undefined;
  }
  document.body.replaceChildren();
  globalThis.fetch = origFetch;
  globalThis.setInterval = origSetInterval;
  globalThis.clearInterval = origClearInterval;
  clearDek();
});

describe("approve helpers", () => {
  test("ephOk accepts only a 32-byte hex public key", () => {
    expect(ephOk("11".repeat(32))).toBe(true);
    expect(ephOk("11".repeat(31))).toBe(false);
    expect(ephOk("")).toBe(false);
    expect(ephOk("zz".repeat(32))).toBe(false);
  });

  test("disabled reasons cover empty, session, and missing DEK", () => {
    const eph = "11".repeat(32);
    const session = sessionInfo();
    expect(deviceDisabledReason({ userCode: "", eph, session, hasDek: true })).toBe(NO_EPH_SENTENCE);
    expect(deviceDisabledReason({ userCode: CODE, eph: "", session, hasDek: true })).toBe(
      NO_EPH_SENTENCE,
    );
    expect(deviceDisabledReason({ userCode: CODE, eph, session: undefined, hasDek: true })).toBe(
      NO_SESSION_SENTENCE,
    );
    expect(deviceDisabledReason({ userCode: CODE, eph, session, hasDek: false })).toBe(
      NO_DEK_SENTENCE,
    );
    expect(deviceDisabledReason({ userCode: CODE, eph, session, hasDek: true })).toBeUndefined();
  });

  test("splitCode, facts, result copy, and fail sentences", () => {
    expect(splitCode(CODE)).toEqual(["K", "4", "T", "7", "-", "Q", "M", "9", "2"]);
    const eph = "4f0c91abd7e23b58" + "f".repeat(48);
    const now = Date.parse("2026-08-28T09:12:40Z");
    expect(
      approveFacts({
        hostname: HOSTNAME,
        created: CREATED,
        expiresIn: 260,
        eph,
        email: EMAIL,
        nowMs: now,
      }),
    ).toEqual([
      { label: "Requesting host", value: HOSTNAME },
      { label: "Requested", value: ago(CREATED, now) },
      { label: "Expires", value: `in ${countdown(260)}` },
      { label: "Ephemeral key", value: keyFingerprint(eph) },
      { label: "Signed in as", value: EMAIL },
    ]);
    expect(resultCopy("approved")).toEqual({ title: APPROVED_TITLE, body: APPROVED_BODY });
    expect(resultCopy("denied")).toEqual({ title: DENIED_TITLE, body: DENIED_BODY });
    expect(resultCopy("missing")).toEqual({ title: MISSING_TITLE, body: MISSING_BODY });
    expect(failSentence(429)).toBe(RATE_SENTENCE);
    expect(failSentence(400, { error: "already approved" })).toBe("already approved");
    expect(failSentence(500)).toBe(FAIL_SENTENCE);
  });
});

describe("approve screen", () => {
  test("paints the approval chrome, code boxes, facts, and footer", async () => {
    const eph = deviceEphHex();
    const calls = apis({ pending: [pendingRow(eph)] });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    expect(calls.some((c) => c.method === "GET" && c.url === devicePendingUrl())).toBe(true);
    expect(root.querySelector(".approve")).not.toBeNull();
    expect(root.querySelector(".approve-top")?.textContent).toContain("Device approval");
    expect(root.querySelector(".brand-mark-sm")?.textContent).toBe("s");
    expect(root.querySelector("[data-host]")?.textContent).toContain(
      globalThis.location.host,
    );
    expect(root.querySelector(".approve-head")?.textContent).toContain(HEAD_TITLE);
    expect(root.querySelector(".approve-head")?.textContent).toContain("secd");
    const boxes = [...root.querySelectorAll(".code-box")].map((n) => n.textContent);
    expect(boxes).toEqual(["K", "4", "T", "7", "Q", "M", "9", "2"]);
    expect(root.querySelectorAll(".code-gap")).toHaveLength(1);
    const facts = [...root.querySelectorAll(".fact")].map((n) => ({
      label: n.querySelector(".fact-label")?.textContent,
      value: n.querySelector(".fact-value")?.textContent,
    }));
    expect(facts[0]).toEqual({ label: "Requesting host", value: HOSTNAME });
    expect(facts[1]).toEqual({ label: "Requested", value: ago(CREATED) });
    expect(facts[2]?.label).toBe("Expires");
    expect(facts[2]?.value?.startsWith("in ")).toBe(true);
    expect(facts[3]).toEqual({ label: "Ephemeral key", value: keyFingerprint(eph) });
    expect(facts[4]).toEqual({ label: "Signed in as", value: EMAIL });
    expect(root.querySelector(".note")?.textContent).toBe(APPROVE_NOTE);
    expect(asButton(root.querySelector('[data-action="approve"]'))?.textContent).toBe(
      APPROVE_LABEL,
    );
    expect(asButton(root.querySelector('[data-action="deny"]'))?.textContent).toBe(DENY_LABEL);
    expect(root.querySelector(".approve-foot")?.textContent).toContain(FOOT_ROUTE);
    expect(root.querySelector('[data-action="console"]')?.textContent).toBe("console");
  });

  test("uses state.eph when it is 64-hex and still GETs pending for the host", async () => {
    const stateEph = "11".repeat(32);
    const pendingEph = "22".repeat(32);
    apis({ pending: [pendingRow(pendingEph)] });
    state = makeState({ eph: stateEph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    expect(root.textContent).toContain(keyFingerprint(stateEph));
    expect(root.textContent).not.toContain(keyFingerprint(pendingEph));
    expect(root.textContent).toContain(HOSTNAME);
  });

  test("fills eph_pub from pending when state.eph is empty", async () => {
    const eph = deviceEphHex();
    apis({ pending: [pendingRow(eph)] });
    state = makeState({ eph: "" });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    expect(root.textContent).toContain(keyFingerprint(eph));
  });

  test("Approve seals the DEK to the CLI eph and does not put the DEK on the wire", async () => {
    const eph = deviceEphHex();
    const calls = apis({ pending: [pendingRow(eph)] });
    const dek = mintDek();
    setDek(dek);
    const held = new Uint8Array(dek);
    state = makeState({ eph });
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    asButton(root.querySelector('[data-action="approve"]'))?.click();
    await settled();
    const post = calls.find((c) => c.method === "POST" && c.url === deviceApproveUrl());
    expect(post).toBeDefined();
    const body = post?.body as {
      user_code: string;
      sealed_dek: { alg: string; eph_pub: string; blob: string };
    };
    expect(body.user_code).toBe(CODE);
    expect(body.sealed_dek.alg).toBe("x25519-xchacha20poly1305");
    expect(typeof body.sealed_dek.eph_pub).toBe("string");
    expect(typeof body.sealed_dek.blob).toBe("string");
    const wire = JSON.stringify(body);
    expect(wire.includes(toHex(held))).toBe(false);
    const sk = deviceSk();
    const their = fromHex(body.sealed_dek.eph_pub);
    const shared = x25519Shared(sk, their);
    const opened = open(shared, "dek", fromHex(body.sealed_dek.blob));
    expect(sameBytes(opened, held)).toBe(true);
    zeroizeBytes(opened);
    zeroizeBytes(shared);
    zeroizeBytes(their);
    zeroizeBytes(sk);
    zeroizeBytes(held);
    expect(root.querySelector('.result-dot[data-result="approved"]')).not.toBeNull();
    expect(root.querySelector(".result-title")?.textContent).toBe(APPROVED_TITLE);
    expect(root.querySelector(".result-body")?.textContent).toBe(APPROVED_BODY);
  });

  test("missing DEK paints NO_DEK_SENTENCE and does not POST", async () => {
    const eph = deviceEphHex();
    const calls = apis({ pending: [pendingRow(eph)] });
    state = makeState({ eph });
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    expect(root.querySelector(".alert-danger")?.textContent).toBe(NO_DEK_SENTENCE);
    expect(asButton(root.querySelector('[data-action="approve"]'))?.disabled).toBe(true);
    asButton(root.querySelector('[data-action="approve"]'))?.click();
    await settled();
    expect(calls.some((c) => c.method === "POST" && c.url === deviceApproveUrl())).toBe(false);
  });

  test("approve 401 signs the tab out", async () => {
    const eph = deviceEphHex();
    apis({ pending: [pendingRow(eph)], approve: 401, approveBody: { error: FAIL_SENTENCE } });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    const c = cap();
    renderApprove(state, root, makeHost(c));
    await settled();
    asButton(root.querySelector('[data-action="approve"]'))?.click();
    await settled();
    expect(c.signOut).toBe(1);
  });

  test("approve 400 already-approved paints the rejection text", async () => {
    const eph = deviceEphHex();
    apis({
      pending: [pendingRow(eph)],
      approve: 400,
      approveBody: { error: "already approved" },
    });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    asButton(root.querySelector('[data-action="approve"]'))?.click();
    await settled();
    expect(root.querySelector(".alert-danger")?.textContent).toBe("already approved");
    expect(root.querySelector(".approve-card")).not.toBeNull();
  });

  test("approve 429 paints the rate sentence", async () => {
    const eph = deviceEphHex();
    apis({ pending: [pendingRow(eph)], approve: 429 });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    asButton(root.querySelector('[data-action="approve"]'))?.click();
    await settled();
    expect(root.querySelector(".alert-danger")?.textContent).toBe(RATE_SENTENCE);
  });

  test("Deny POSTs the user_code and paints the denied result", async () => {
    const eph = deviceEphHex();
    const calls = apis({ pending: [pendingRow(eph)] });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    asButton(root.querySelector('[data-action="deny"]'))?.click();
    await settled();
    const deny = calls.find((c) => c.method === "POST" && c.url === deviceDenyUrl());
    expect(deny?.body).toEqual({ user_code: CODE });
    expect(root.querySelector('.result-dot[data-result="denied"]')).not.toBeNull();
    expect(root.querySelector(".result-title")?.textContent).toBe(DENIED_TITLE);
    expect(root.querySelector(".result-body")?.textContent).toBe(DENIED_BODY);
  });

  test("deny 401 signs the tab out", async () => {
    const eph = deviceEphHex();
    apis({ pending: [pendingRow(eph)], deny: 401 });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    const c = cap();
    renderApprove(state, root, makeHost(c));
    await settled();
    asButton(root.querySelector('[data-action="deny"]'))?.click();
    await settled();
    expect(c.signOut).toBe(1);
  });

  test("pending GET 401 signs the tab out", async () => {
    apis({ pendingStatus: 401 });
    state = makeState({ eph: "" });
    const root = document.createElement("div");
    const c = cap();
    renderApprove(state, root, makeHost(c));
    await settled();
    expect(c.signOut).toBe(1);
  });

  test("no code and no eph paints Request not found", async () => {
    const calls = apis();
    state = makeState({ code: "", eph: "" });
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    expect(root.querySelector(".result-title")?.textContent).toBe(MISSING_TITLE);
    expect(root.querySelector(".result-body")?.textContent).toBe(MISSING_BODY);
    expect(calls.some((c) => c.url === devicePendingUrl())).toBe(false);
  });

  test("a code that is not pending and has no eph paints Request not found", async () => {
    apis({ pending: [] });
    state = makeState({ eph: "" });
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    expect(root.querySelector(".result-title")?.textContent).toBe(MISSING_TITLE);
    expect(root.querySelector(".result-body")?.textContent).toBe(MISSING_BODY);
  });

  test("Back to console navigates to /devices and clears the code", async () => {
    apis({ pending: [] });
    state = makeState({ eph: "" });
    const root = document.createElement("div");
    const c = cap();
    renderApprove(state, root, makeHost(c));
    await settled();
    asButton(root.querySelector('[data-action="back"]'))?.click();
    expect(c.navs).toEqual(["/devices"]);
    expect(state.userCode.get()).toBe("");
    expect(state.eph.get()).toBe("");
  });

  test("the console footer link clears the code and navigates", async () => {
    const eph = deviceEphHex();
    apis({ pending: [pendingRow(eph)] });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    const c = cap();
    renderApprove(state, root, makeHost(c));
    await settled();
    const link = root.querySelector('[data-action="console"]');
    link?.dispatchEvent(new Event("click", { bubbles: true, cancelable: true }));
    expect(c.navs).toEqual(["/devices"]);
    expect(state.userCode.get()).toBe("");
  });

  test("in-flight Approve disables both controls", async () => {
    const eph = deviceEphHex();
    const finish: { resolve?: (value: Response) => void } = {};
    apis({ pending: [pendingRow(eph)], hangApprove: true, finish });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    asButton(root.querySelector('[data-action="approve"]'))?.click();
    await settled();
    expect(asButton(root.querySelector('[data-action="approve"]'))?.disabled).toBe(true);
    expect(asButton(root.querySelector('[data-action="deny"]'))?.disabled).toBe(true);
    finish.resolve?.(json({ ok: true }));
    await settled();
  });

  test("the 1s countdown is cleared on leave", async () => {
    const eph = deviceEphHex();
    apis({ pending: [pendingRow(eph)] });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    expect([...fakes.values()].some((t) => t.timeout === TICK_MS)).toBe(true);
    leaveApprove(state);
    expect([...fakes.values()].some((t) => t.timeout === TICK_MS)).toBe(false);
    state = undefined;
  });

  test("leave drops an in-flight Approve", async () => {
    const eph = deviceEphHex();
    const finish: { resolve?: (value: Response) => void } = {};
    apis({ pending: [pendingRow(eph)], hangApprove: true, finish });
    state = makeState({ eph });
    setDek(mintDek());
    const root = document.createElement("div");
    renderApprove(state, root, makeHost(cap()));
    await settled();
    asButton(root.querySelector('[data-action="approve"]'))?.click();
    await settled();
    leaveApprove(state);
    bumpLogoutGen();
    finish.resolve?.(json({ ok: true }));
    await settled();
    expect(root.querySelector(".approve-result")).toBeNull();
    state = undefined;
  });
});
