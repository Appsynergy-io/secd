import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  FAIL_SENTENCE,
  LAST_FACTOR_SENTENCE,
  NO_DEK_SENTENCE,
} from "../lib/api.ts";
import { clearDek, mintDek, setDek } from "../lib/crypto.ts";
import { signal } from "../lib/signal.ts";
import {
  CLIP_FAIL_SENTENCE,
  EMPTY_PASSKEYS,
  EMPTY_SESSIONS,
  LOADING_SESSIONS,
  createdDay,
  dekFactors,
  lastFactor,
  parsePasskeys,
  parseSessions,
  renderAccount,
  shortId,
  type AccountHost,
  type PasskeyRow,
  type SessionRow,
} from "./account.ts";

function host(q: {
  has_passkey: boolean;
  has_password: boolean;
  passkeys?: PasskeyRow[] | undefined;
  email?: string;
}): AccountHost {
  return {
    path: signal("/account"),
    error: signal(undefined),
    pending: signal(false),
    session: signal({
      email: q.email ?? "a@b.c",
      has_passkey: q.has_passkey,
      has_password: q.has_password,
      session_id: "s1",
    }),
    passkeys: signal(q.passkeys),
  };
}

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

const SESSIONS: SessionRow[] = [
  {
    id: "sess-this",
    kind: "console",
    label: "this browser",
    created: "2026-01-01T00:00:00Z",
    last_seen: "2026-01-02T00:00:00Z",
    current: true,
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

describe("account helpers", () => {
  test("dekFactors is the unwrap chain", () => {
    expect(dekFactors({ has_passkey: true, has_password: true })).toEqual([
      "passkey",
      "password",
    ]);
    expect(lastFactor(["passkey"])).toBe(true);
    expect(lastFactor(["passkey", "password"])).toBe(false);
  });

  test("parseSessions and parsePasskeys skip empty ids", () => {
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
    expect(shortId("abcdefghijklmn")).toBe("abcdefghijkl\u2026");
    expect(createdDay("2026-01-01T00:00:00Z")).toBe("2026-01-01");
  });
});

describe("account screen", () => {
  const origFetch = globalThis.fetch;
  const origNav = globalThis.navigator;
  const origWidth = globalThis.innerWidth;

  beforeEach(() => {
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 1280,
    });
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url === "/api/v1/sessions") {
        return new Response(JSON.stringify({ sessions: SESSIONS }), { status: 200 });
      }
      if (method === "GET" && url.includes("/passkeys")) {
        return new Response(JSON.stringify({ passkeys: [] }), { status: 200 });
      }
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
  });

  afterEach(() => {
    globalThis.fetch = origFetch;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      writable: true,
      value: origNav,
    });
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: origWidth,
    });
    clearDek();
  });

  test("sessions list Revoke and DELETE /api/v1/sessions/:id", async () => {
    const root = document.createElement("div");
    const calls: Array<{ method: string; url: string }> = [];
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      calls.push({ method, url });
      if (method === "GET" && url === "/api/v1/sessions") {
        return new Response(JSON.stringify({ sessions: SESSIONS }), { status: 200 });
      }
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }) as unknown as typeof fetch;
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(root.querySelector('[data-list="sessions"]')?.textContent).toContain("nuc");
    expect(root.querySelector('[data-session-id="sess-nuc"]')).not.toBeNull();
    const revoke = root.querySelector('[data-session-id="sess-nuc"] [data-action="revoke"]');
    expect(revoke).not.toBeNull();
    (revoke as HTMLButtonElement).click();
    await Bun.sleep(1);
    expect(calls).toContainEqual({ method: "DELETE", url: "/api/v1/sessions/sess-nuc" });
  });

  test("empty and loading sessions have copy from the user's side", async () => {
    const root = document.createElement("div");
    globalThis.fetch = (() => new Promise<Response>(() => {})) as unknown as typeof fetch;
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    expect(root.querySelector('[data-list="sessions"]')?.textContent).toContain(LOADING_SESSIONS);
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ sessions: [] }), { status: 200 })) as unknown as typeof fetch;
    const empty = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(empty, root);
    await Bun.sleep(1);
    expect(root.querySelector('[data-list="sessions"]')?.textContent).toContain(EMPTY_SESSIONS);
  });

  test("one remaining factor breaks the chain, disables Remove, and shows the reason", () => {
    const root = document.createElement("div");
    const state = host({
      has_passkey: true,
      has_password: false,
      passkeys: [{ id: "pk-only", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    expect(root.querySelector('[data-chain="dek"]')?.getAttribute("data-last")).toBe("1");
    expect(root.querySelector('[data-action="remove"]')?.hasAttribute("disabled")).toBe(true);
    expect(root.querySelector(".chain-reason")?.textContent).toBe(LAST_FACTOR_SENTENCE);
  });

  test("Add passkey is disabled without a DEK and the reason is visible", () => {
    const root = document.createElement("div");
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    expect(root.querySelector('[data-action="add-passkey"]')?.hasAttribute("disabled")).toBe(true);
    expect(root.textContent).toContain(NO_DEK_SENTENCE);
  });

  test("Add passkey registers and wraps the held DEK", async () => {
    const root = document.createElement("div");
    const bodies: unknown[] = [];
    const prf = new Uint8Array(32).fill(9);
    const raw = new Uint8Array([10, 11, 12, 13]);
    setDek(mintDek());
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
        clipboard: { writeText: async () => {} },
      },
    });
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "POST" && url.includes("register/start")) {
        return new Response(
          JSON.stringify({
            handle: "h1",
            publicKey: { challenge: "AQIDBA" },
          }),
          { status: 200 },
        );
      }
      if (method === "POST" && url.includes("register/finish")) {
        bodies.push(JSON.parse(String(init?.body ?? "{}")));
        return new Response(JSON.stringify({ ok: true }), { status: 200 });
      }
      if (method === "GET" && url === "/api/session") {
        return new Response(
          JSON.stringify({
            email: "a@b.c",
            session_id: "s1",
            has_passkey: true,
            has_password: true,
          }),
          { status: 200 },
        );
      }
      if (method === "GET" && url === "/api/v1/sessions") {
        return new Response(JSON.stringify({ sessions: [] }), { status: 200 });
      }
      if (method === "GET" && url.includes("/passkeys")) {
        return new Response(
          JSON.stringify({ passkeys: [{ id: "0a0b0c0d", created: "2026-01-01T00:00:00Z" }] }),
          { status: 200 },
        );
      }
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    const add = root.querySelector('[data-action="add-passkey"]') as HTMLButtonElement;
    expect(add.disabled).toBe(false);
    add.click();
    await Bun.sleep(10);
    const finish = bodies[0] as { wrap?: { factor?: string; cred_id?: string } };
    expect(finish.wrap?.factor).toBe("passkey");
    expect(finish.wrap?.cred_id).toBe("0a0b0c0d");
  });

  test("Copy writes the selected passkey id and becomes Copied only after writeText resolves", async () => {
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
    const id = "pk-copy-me-hex";
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id, created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    document.body.append(root);
    (root.querySelector(`[data-passkey-id="${id}"] button:not([data-action])`) as HTMLButtonElement).click();
    const btn = root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement;
    expect(btn.textContent).toBe("Copy");
    btn.click();
    await Bun.sleep(1);
    expect(btn.textContent).toBe("Copy");
    expect(wrote).toBe(id);
    finish?.();
    await Bun.sleep(1);
    expect(
      (root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement)
        .textContent,
    ).toBe("Copied");
    expect(wrote).toBe(id);
    expect(document.activeElement).toBe(
      root.querySelector('[data-pane="inspector"] [data-action="copy"]'),
    );
    root.remove();
  });

  test("clipboard refusal keeps Copy and offers select-to-copy of the id", async () => {
    const root = document.createElement("div");
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {},
    });
    const id = "pk-select-copy";
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id, created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    document.body.append(root);
    (root.querySelector(`[data-passkey-id="${id}"] button:not([data-action])`) as HTMLButtonElement).click();
    (root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement).click();
    await Bun.sleep(1);
    expect(root.querySelector('[data-action="copy"]')?.textContent).toBe("Copy");
    expect(root.querySelector(".error")?.textContent).toBe(CLIP_FAIL_SENTENCE);
    const fallback = root.querySelector("[data-copy-fallback]") as HTMLInputElement | null;
    expect(fallback?.value).toBe(id);
    expect(fallback?.getAttribute("aria-label")).toBe("Identifier");
    expect(document.activeElement).toBe(fallback);
    root.remove();
  });

  test("list | inspector at 900px; below 900 a selection opens the sheet", async () => {
    const root = document.createElement("div");
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 900,
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(root.querySelector('[data-layout="list-inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="list"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();

    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 899,
    });
    renderAccount(state, root, undefined, undefined, 899);
    (root.querySelector('[data-passkey-id="pk-1"] button:not([data-action])') as HTMLButtonElement).click();
    expect(root.querySelector('[data-layout="list-only"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="sheet"][data-sheet="open"]')).not.toBeNull();
  });

  test("sessions GET failure shows the error and does not record an empty list", async () => {
    const root = document.createElement("div");
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url === "/api/v1/sessions") {
        return new Response("{}", { status: 500 });
      }
      return new Response(JSON.stringify({ passkeys: [] }), { status: 200 });
    }) as unknown as typeof fetch;
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(root.querySelector('[data-list="sessions"] .error')?.textContent).toBe(FAIL_SENTENCE);
    expect(root.querySelector('[data-list="sessions"]')?.textContent).not.toContain(EMPTY_SESSIONS);
    expect(root.querySelector('[data-list="passkeys"]')?.textContent).not.toContain(EMPTY_PASSKEYS);
  });

  test("clipboard fallback lives in the inspector next to Copy", async () => {
    const root = document.createElement("div");
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {},
    });
    const id = "pk-select-copy";
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id, created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    document.body.append(root);
    (root.querySelector(`[data-passkey-id="${id}"] button:not([data-action])`) as HTMLButtonElement).click();
    (root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement).click();
    await Bun.sleep(1);
    const pane = root.querySelector('[data-pane="inspector"]');
    expect(pane?.querySelector(".error")?.textContent).toBe(CLIP_FAIL_SENTENCE);
    expect((pane?.querySelector("[data-copy-fallback]") as HTMLInputElement | null)?.value).toBe(id);
    expect(root.querySelector(".secd-overlay + .error")).toBeNull();
    root.remove();
  });

  test("account sheet is a dialog that inerts the page and restores row focus", async () => {
    const root = document.createElement("div");
    document.body.append(root);
    const id = "pk-1";
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id, created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root, undefined, undefined, 899);
    await Bun.sleep(1);
    (root.querySelector(`[data-passkey-id="${id}"] button:not([data-action])`) as HTMLButtonElement).click();
    const overlay = root.querySelector('[data-pane="sheet"]');
    expect(overlay?.getAttribute("role")).toBe("dialog");
    expect(overlay?.getAttribute("aria-modal")).toBe("true");
    expect(root.querySelector("nav")?.hasAttribute("inert")).toBe(true);
    expect(root.querySelector('[data-action="logout"]')?.hasAttribute("inert")).toBe(true);
    overlay?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();
    expect(document.activeElement).toBe(
      root.querySelector(`[data-passkey-id="${id}"] button:not([data-action])`),
    );
    root.remove();
  });

  test("Copy does not paint Account after leaving the screen", async () => {
    const root = document.createElement("div");
    let finish: ((value: void) => void) | undefined;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {
        clipboard: {
          writeText: () =>
            new Promise<void>((resolve) => {
              finish = resolve;
            }),
        },
      },
    });
    const id = "pk-copy-leave";
    const state = host({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id, created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(1);
    (root.querySelector(`[data-passkey-id="${id}"] button:not([data-action])`) as HTMLButtonElement).click();
    const email = root.querySelector('[data-field="email"]');
    (root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement).click();
    state.path.set("/register");
    finish?.();
    await Bun.sleep(1);
    expect(root.querySelector('[data-field="email"]')).toBe(email);
    expect(root.querySelector('[data-pane="inspector"] [data-action="copy"]')?.textContent).toBe(
      "Copy",
    );
  });
});
