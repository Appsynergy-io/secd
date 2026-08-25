import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { FAIL_SENTENCE, LAST_FACTOR_SENTENCE } from "./lib/api.ts";
import { clearDek, getDek, mintDek, setDek } from "./lib/crypto.ts";
import { signal } from "./lib/signal.ts";
import {
  dekFactors,
  hrefFor,
  initialPath,
  lastFactor,
  render,
  renderAccount,
  renderRegister,
  resolveGate,
  screenFromPath,
  SCREENS,
  type AppState,
} from "./main.ts";
import { abandonRegister } from "./screens/register.ts";

describe("router", () => {
  test("five screens with stable hrefs", () => {
    expect(SCREENS).toEqual(["gate", "device", "register", "activity", "account"]);
    expect(hrefFor("gate")).toBe("/");
    expect(hrefFor("device")).toBe("/device");
    expect(hrefFor("register")).toBe("/register");
    expect(hrefFor("activity")).toBe("/activity");
    expect(hrefFor("account")).toBe("/account");
  });

  test("screenFromPath maps the served routes", () => {
    expect(screenFromPath("/")).toBe("gate");
    expect(screenFromPath("/device")).toBe("device");
    expect(screenFromPath("/register")).toBe("register");
    expect(screenFromPath("/activity")).toBe("activity");
    expect(screenFromPath("/account")).toBe("account");
    expect(screenFromPath("/nope")).toBe("gate");
  });

  test("a user code forces the device screen", () => {
    expect(initialPath("/account", "ABCD-EFGH")).toBe("/device");
    expect(initialPath("/account", "")).toBe("/account");
  });
});

describe("resolveGate", () => {
  test("live session is approve-only", () => {
    const v = resolveGate({
      session: {
        email: "a@b.c",
        has_passkey: true,
        has_password: false,
        session_id: "s1",
      },
    });
    expect(v.kind).toBe("approve-only");
    expect(v.showApprove).toBe(true);
    expect(v.showEmail).toBe(false);
  });

  test("cold start shows email with webauthn autocomplete", () => {
    const v = resolveGate({});
    expect(v.kind).toBe("cold");
    expect(v.showEmail).toBe(true);
    expect(v.emailAutocomplete).toBe("username webauthn");
  });
});

describe("dek chain", () => {
  test("the DEK is the factors that unwrap it", () => {
    expect(dekFactors({ has_passkey: true, has_password: true })).toEqual([
      "passkey",
      "password",
    ]);
    expect(dekFactors({ has_passkey: true, has_password: false })).toEqual([
      "passkey",
    ]);
    expect(dekFactors({ has_passkey: false, has_password: true })).toEqual([
      "password",
    ]);
  });

  test("one remaining factor is last", () => {
    expect(lastFactor(["passkey"])).toBe(true);
    expect(lastFactor(["passkey", "password"])).toBe(false);
    expect(lastFactor([])).toBe(false);
  });
});

type FakeNode = {
  tagName: string;
  nodeType: number;
  disabled: boolean;
  parent: FakeNode | null;
  children: FakeNode[];
  attrs: Map<string, string>;
  listeners: Map<string, Array<() => void>>;
  text: string;
  setAttribute(name: string, value: string): void;
  getAttribute(name: string): string | null;
  hasAttribute(name: string): boolean;
  append(...nodes: Array<FakeNode | string>): void;
  replaceChildren(...nodes: Array<FakeNode | string>): void;
  addEventListener(type: string, fn: () => void): void;
  click(): void;
  submit(): void;
  querySelector(sel: string): FakeNode | null;
  textContent: string;
};

function fakeText(text: string): FakeNode {
  return fakeEl("#text", 3, text);
}

function fakeEl(tag: string, nodeType = 1, text = ""): FakeNode {
  const node: FakeNode = {
    tagName: tag.toUpperCase(),
    nodeType,
    disabled: false,
    parent: null,
    children: [],
    attrs: new Map(),
    listeners: new Map(),
    text,
    setAttribute(name: string, value: string) {
      node.attrs.set(name, value);
      if (name === "disabled") {
        node.disabled = true;
      }
    },
    getAttribute(name: string) {
      return node.attrs.has(name) ? (node.attrs.get(name) ?? "") : null;
    },
    hasAttribute(name: string) {
      return node.attrs.has(name);
    },
    append(...nodes: Array<FakeNode | string>) {
      for (const n of nodes) {
        const child = typeof n === "string" ? fakeText(n) : n;
        child.parent = node;
        node.children.push(child);
      }
    },
    replaceChildren(...nodes: Array<FakeNode | string>) {
      node.children = [];
      node.append(...nodes);
    },
    addEventListener(type: string, fn: () => void) {
      const list = node.listeners.get(type) ?? [];
      list.push(fn);
      node.listeners.set(type, list);
    },
    click() {
      if (node.disabled) {
        return;
      }
      for (const fn of node.listeners.get("click") ?? []) {
        fn();
      }
    },
    submit() {
      const ev = { preventDefault() {} };
      for (const fn of node.listeners.get("submit") ?? []) {
        (fn as unknown as (e: { preventDefault(): void }) => void)(ev);
      }
    },
    querySelector(sel: string) {
      const found: FakeNode[] = [];
      walk(node, (n) => {
        if (n !== node && n.nodeType === 1 && matches(n, sel)) {
          found.push(n);
        }
      });
      return found[0] ?? null;
    },
    get textContent() {
      if (node.nodeType === 3) {
        return node.text;
      }
      return node.children.map((c) => c.textContent).join("");
    },
    set textContent(value: string) {
      node.children = [];
      if (value !== "") {
        node.append(value);
      }
    },
  };
  return node;
}

function walk(node: FakeNode, fn: (n: FakeNode) => void): void {
  fn(node);
  for (const child of node.children) {
    walk(child, fn);
  }
}

function matches(node: FakeNode, sel: string): boolean {
  const tag = sel.match(/^[a-zA-Z][\w-]*/);
  let rest = sel;
  if (tag) {
    if (node.tagName !== tag[0].toUpperCase()) {
      return false;
    }
    rest = sel.slice(tag[0].length);
  }
  const parts = rest.match(/(\.[a-zA-Z][\w-]*|\[[^\]]+\])/g) ?? [];
  if (!tag && parts.length === 0) {
    return false;
  }
  for (const part of parts) {
    if (part.startsWith(".")) {
      const cls = node.getAttribute("class") ?? "";
      if (!cls.split(/\s+/).includes(part.slice(1))) {
        return false;
      }
      continue;
    }
    const attr = part.match(/^\[([^\s=\]]+)(?:=\"([^\"]*)\")?\]$/);
    if (!attr) {
      return false;
    }
    const name = attr[1];
    if (name === undefined) {
      return false;
    }
    const got = node.getAttribute(name);
    if (attr[2] !== undefined) {
      if (got !== attr[2]) {
        return false;
      }
    } else if (got === null) {
      return false;
    }
  }
  return true;
}

function installDom(): FakeNode {
  const body = fakeEl("body");
  const document = {
    body,
    createElement(tag: string) {
      return fakeEl(tag);
    },
    createTextNode(text: string) {
      return fakeText(text);
    },
    getElementById(id: string) {
      let found: FakeNode | null = null;
      walk(body, (n) => {
        if (found === null && n.getAttribute("id") === id) {
          found = n;
        }
      });
      return found;
    },
  };
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    writable: true,
    value: document,
  });
  return body;
}

function accountState(q: {
  has_passkey: boolean;
  has_password: boolean;
  passkeys?: Array<{ id: string; created: string }> | undefined;
}): AppState {
  return {
    path: signal("/account"),
    email: signal(""),
    password: signal(""),
    error: signal(undefined),
    pending: signal(false),
    session: signal({
      email: "a@b.c",
      has_passkey: q.has_passkey,
      has_password: q.has_password,
      session_id: "s1",
    }),
    method: signal(undefined),
    different: signal(false),
    revealPassword: signal(false),
    userCode: signal(""),
    eph: signal(""),
    passkeys: signal(q.passkeys),
  };
}

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

describe("Account chain", () => {
  const origDocument = globalThis.document;
  const origFetch = globalThis.fetch;
  const origLocation = globalThis.location;
  const origHistory = globalThis.history;

  beforeEach(() => {
    installDom();
    const loc = { pathname: "/account" };
    Object.defineProperty(globalThis, "location", {
      configurable: true,
      writable: true,
      value: loc,
    });
    Object.defineProperty(globalThis, "history", {
      configurable: true,
      writable: true,
      value: {
        pushState(_s: unknown, _t: unknown, url: string) {
          loc.pathname = String(url);
        },
      },
    });
  });

  afterEach(() => {
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      writable: true,
      value: origDocument,
    });
    Object.defineProperty(globalThis, "location", {
      configurable: true,
      writable: true,
      value: origLocation,
    });
    Object.defineProperty(globalThis, "history", {
      configurable: true,
      writable: true,
      value: origHistory,
    });
    globalThis.fetch = origFetch;
    clearDek();
  });

  test("one factor sets data-last, disables Remove, and shows chain-reason", () => {
    const root = document.createElement("div");
    const state = accountState({
      has_passkey: true,
      has_password: false,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    const chain = root.querySelector('[data-chain="dek"]');
    expect(chain?.getAttribute("data-last")).toBe("1");
    const remove = root.querySelector('[data-action="remove"]');
    expect(remove?.hasAttribute("disabled")).toBe(true);
    expect(root.querySelector(".chain-reason")?.textContent).toBe(LAST_FACTOR_SENTENCE);
    const calls: string[] = [];
    globalThis.fetch = (async () => {
      calls.push("fetch");
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
    (remove as unknown as FakeNode | null)?.click();
    expect(calls).toEqual([]);
  });

  test("Remove stays disabled until the passkeys list is loaded", async () => {
    const root = document.createElement("div");
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (() =>
      new Promise((resolve) => {
        finish = resolve;
      })) as unknown as typeof fetch;
    const state = accountState({ has_passkey: true, has_password: true });
    renderAccount(state, root);
    expect(root.querySelector('[data-chain="dek"]')?.getAttribute("data-last")).toBe("0");
    expect(root.querySelector(".chain-reason")).toBeNull();
    expect(root.querySelector('[data-action="remove"]')?.hasAttribute("disabled")).toBe(true);
    finish?.(new Response(JSON.stringify({ passkeys: [] }), { status: 200 }));
    await Bun.sleep(1);
  });

  test("two passkeys and no password enable Remove", async () => {
    const root = document.createElement("div");
    const fetches: Array<{ method: string; url: string }> = [];
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      fetches.push({ method: String(init?.method ?? "GET"), url });
      if (String(init?.method ?? "GET") === "GET" && url === "/api/session") {
        return new Response(
          JSON.stringify({
            email: "a@b.c",
            session_id: "s1",
            has_passkey: true,
            has_password: false,
          }),
          { status: 200 },
        );
      }
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }) as unknown as typeof fetch;
    const state = accountState({
      has_passkey: true,
      has_password: false,
      passkeys: [
        { id: "pk-1", created: "2026-01-01T00:00:00Z" },
        { id: "pk-2", created: "2026-01-02T00:00:00Z" },
      ],
    });
    renderAccount(state, root);
    expect(root.querySelector('[data-chain="dek"]')?.getAttribute("data-last")).toBe("0");
    expect(root.querySelector(".chain-reason")).toBeNull();
    const remove = root.querySelector('[data-action="remove"]');
    expect(remove?.hasAttribute("disabled")).toBe(false);
    (remove as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(fetches).toContainEqual({
      method: "DELETE",
      url: "/api/auth/passkeys/pk-1",
    });
  });

  test("two factors enable Remove and DELETE the loaded passkey", async () => {
    const root = document.createElement("div");
    const fetches: Array<{ method: string; url: string }> = [];
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url =
        typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      fetches.push({ method: String(init?.method ?? "GET"), url });
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }) as unknown as typeof fetch;
    const state = accountState({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    const chain = root.querySelector('[data-chain="dek"]');
    expect(chain?.getAttribute("data-last")).toBe("0");
    expect(root.querySelector(".chain-reason")).toBeNull();
    const remove = root.querySelector('[data-action="remove"]');
    expect(remove?.hasAttribute("disabled")).toBe(false);
    (remove as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(fetches).toContainEqual({
      method: "DELETE",
      url: "/api/auth/passkeys/pk-1",
    });
  });

  test("GET /passkeys failure sets error and does not record an empty list", async () => {
    const root = document.createElement("div");
    let n = 0;
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url.includes("/passkeys")) {
        n += 1;
        return new Response("{}", { status: 500 });
      }
      return new Response(JSON.stringify({ sessions: [] }), { status: 200 });
    }) as unknown as typeof fetch;
    const state = accountState({ has_passkey: true, has_password: true });
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(n).toBe(1);
    expect(state.passkeys.get()).toBeUndefined();
    expect(state.error.get()).toBe(FAIL_SENTENCE);
    expect(root.querySelector('[data-action="remove"]')?.hasAttribute("disabled")).toBe(true);
    expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(n).toBe(1);
  });

  test("leaving Account lets the next visit retry GET /passkeys", async () => {
    const root = document.createElement("div");
    let n = 0;
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url.includes("/passkeys")) {
        n += 1;
        if (n === 1) {
          return new Response("{}", { status: 500 });
        }
        return new Response(JSON.stringify({ passkeys: [] }), { status: 200 });
      }
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
    const state = accountState({ has_passkey: true, has_password: true });
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(n).toBe(1);
    expect(state.error.get()).toBe(FAIL_SENTENCE);
    (root.querySelector('[data-action="logout"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(state.session.get()).toBeUndefined();
    expect(state.passkeys.get()).toBeUndefined();
    state.session.set({
      email: "a@b.c",
      has_passkey: true,
      has_password: true,
      session_id: "s1",
    });
    state.path.set("/account");
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(n).toBe(2);
    expect(state.passkeys.get()).toEqual([]);
    expect(state.error.get()).toBeUndefined();
    expect(root.querySelector(".error")).toBeNull();
  });

  test("Sign out clears DEK and session when POST throws", async () => {
    const root = document.createElement("div");
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "POST" && url.includes("/logout")) {
        throw new Error("network");
      }
      return new Response(JSON.stringify({ passkeys: [] }), { status: 200 });
    }) as unknown as typeof fetch;
    setDek(mintDek());
    expect(getDek()?.length).toBe(32);
    const state = accountState({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    (root.querySelector('[data-action="logout"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(getDek()).toBeUndefined();
    expect(state.session.get()).toBeUndefined();
    expect(state.passkeys.get()).toBeUndefined();
    expect(state.path.get()).toBe("/");
    expect(state.error.get()).toBeUndefined();
    expect(root.querySelector(".error")).toBeNull();
  });

  test("in-flight GET /passkeys is ignored after leaving Account", async () => {
    const root = document.createElement("div");
    const finishes: Array<(value: Response) => void> = [];
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url.includes("/passkeys")) {
        return new Promise<Response>((resolve) => {
          finishes.push(resolve);
        });
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;
    const state = accountState({ has_passkey: true, has_password: true });
    renderAccount(state, root);
    expect(finishes.length).toBe(1);
    state.path.set("/activity");
    render(state);
    state.path.set("/account");
    render(state);
    expect(finishes.length).toBe(2);
    finishes[0]?.(new Response("{}", { status: 500 }));
    await Bun.sleep(1);
    expect(state.error.get()).toBeUndefined();
    expect(state.passkeys.get()).toBeUndefined();
    finishes[1]?.(
      new Response(JSON.stringify({ passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }] }), {
        status: 200,
      }),
    );
    await Bun.sleep(1);
    expect(state.error.get()).toBeUndefined();
    expect(state.passkeys.get()).toEqual([{ id: "pk-1", created: "2026-01-01T00:00:00Z" }]);
    expect(root.querySelector(".error")).toBeNull();
  });

  test("retrying GET /passkeys clears a prior error before the response", async () => {
    const root = document.createElement("div");
    let n = 0;
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url.includes("/passkeys")) {
        n += 1;
        if (n === 1) {
          return Promise.resolve(new Response("{}", { status: 500 }));
        }
        return new Promise<Response>((resolve) => {
          finish = resolve;
        });
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;
    const state = accountState({ has_passkey: true, has_password: true });
    renderAccount(state, root);
    await Bun.sleep(1);
    expect(n).toBe(1);
    expect(state.error.get()).toBe(FAIL_SENTENCE);
    state.path.set("/activity");
    render(state);
    state.path.set("/account");
    render(state);
    expect(n).toBe(2);
    expect(state.error.get()).toBeUndefined();
    finish?.(new Response(JSON.stringify({ passkeys: [] }), { status: 200 }));
    await Bun.sleep(1);
    expect(state.error.get()).toBeUndefined();
    expect(state.passkeys.get()).toEqual([]);
    expect(root.querySelector(".error")).toBeNull();
  });

  test("in-flight GET /passkeys is ignored after Sign out", async () => {
    const root = document.createElement("div");
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url.includes("/passkeys")) {
        return new Promise<Response>((resolve) => {
          finish = resolve;
        });
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;
    const state = accountState({ has_passkey: true, has_password: true });
    renderAccount(state, root);
    (root.querySelector('[data-action="logout"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(state.session.get()).toBeUndefined();
    finish?.(
      new Response(JSON.stringify({ passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }] }), {
        status: 200,
      }),
    );
    await Bun.sleep(1);
    expect(state.passkeys.get()).toBeUndefined();
    expect(state.session.get()).toBeUndefined();
  });

  test("in-flight Remove loadSession is dropped after Sign out", async () => {
    const root = document.createElement("div");
    let finishDelete: ((value: Response) => void) | undefined;
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "DELETE") {
        return new Promise<Response>((resolve) => {
          finishDelete = resolve;
        });
      }
      if (method === "GET" && url.includes("/session")) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              email: "a@b.c",
              session_id: "s1",
              has_passkey: true,
              has_password: true,
            }),
            { status: 200 },
          ),
        );
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;
    const state = accountState({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    (root.querySelector('[data-action="remove"]') as unknown as FakeNode | null)?.click();
    (root.querySelector('[data-action="logout"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(state.session.get()).toBeUndefined();
    finishDelete?.(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    await Bun.sleep(1);
    expect(state.session.get()).toBeUndefined();
    expect(state.passkeys.get()).toBeUndefined();
  });

  test("DELETE then GET /session 5xx keeps the session and surfaces the error", async () => {
    const root = document.createElement("div");
    let passkeysGet = 0;
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "DELETE") {
        return Promise.resolve(new Response(JSON.stringify({ ok: true }), { status: 200 }));
      }
      if (method === "GET" && url === "/api/session") {
        return Promise.resolve(new Response("{}", { status: 500 }));
      }
      if (method === "GET" && url.includes("/passkeys")) {
        passkeysGet += 1;
        return Promise.resolve(new Response("{}", { status: 500 }));
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;
    setDek(mintDek());
    const state = accountState({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    (root.querySelector('[data-action="remove"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(state.session.get()?.session_id).toBe("s1");
    expect(state.error.get()).toBe(FAIL_SENTENCE);
    expect(getDek()?.length).toBe(32);
    expect(state.passkeys.get()).toBeUndefined();
    expect(passkeysGet).toBe(1);
    expect(root.querySelector('[data-action="remove"]')?.hasAttribute("disabled")).toBe(true);
    expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
  });

  test("DELETE then GET /session 401 clears the session", async () => {
    const root = document.createElement("div");
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "DELETE") {
        return Promise.resolve(new Response(JSON.stringify({ ok: true }), { status: 200 }));
      }
      if (method === "GET" && url === "/api/session") {
        return Promise.resolve(new Response("{}", { status: 401 }));
      }
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;
    setDek(mintDek());
    const state = accountState({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    (root.querySelector('[data-action="remove"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(state.session.get()).toBeUndefined();
    expect(state.error.get()).toBeUndefined();
    expect(getDek()).toBeUndefined();
    expect(state.path.get()).toBe("/");
  });

  test("GET /sessions failure does not retry through render", async () => {
    const root = document.createElement("div");
    let n = 0;
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "GET" && url === "/api/v1/sessions") {
        n += 1;
        return new Response("{}", { status: 429 });
      }
      if (method === "GET" && url.includes("/passkeys")) {
        return new Response(JSON.stringify({ passkeys: [] }), { status: 200 });
      }
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
    const state = accountState({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    await Bun.sleep(20);
    expect(n).toBe(1);
    renderAccount(state, root);
    await Bun.sleep(20);
    expect(n).toBe(1);
  });

  test("Add passkey does not POST finish after Sign out during WebAuthn", async () => {
    const root = document.createElement("div");
    let finishCreate: ((value: unknown) => void) | undefined;
    const origNav = globalThis.navigator;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {
        credentials: {
          create: () =>
            new Promise((resolve) => {
              finishCreate = resolve;
            }),
        },
      },
    });
    const calls: string[] = [];
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      calls.push(`${method} ${url}`);
      if (method === "POST" && url.includes("register/start")) {
        return new Response(
          JSON.stringify({ handle: "h1", publicKey: { challenge: "AQIDBA" } }),
          { status: 200 },
        );
      }
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }) as unknown as typeof fetch;
    setDek(mintDek());
    const state = accountState({
      has_passkey: true,
      has_password: true,
      passkeys: [{ id: "pk-1", created: "2026-01-01T00:00:00Z" }],
    });
    renderAccount(state, root);
    (root.querySelector('[data-action="add-passkey"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(10);
    (root.querySelector('[data-action="logout"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(state.session.get()).toBeUndefined();
    expect(getDek()).toBeUndefined();
    const prf = new Uint8Array(32).fill(9);
    finishCreate?.({
      id: "cred",
      type: "public-key",
      rawId: new Uint8Array([10, 11, 12, 13]).buffer,
      response: {
        clientDataJSON: new Uint8Array([1]).buffer,
        attestationObject: new Uint8Array([2]).buffer,
      },
      getClientExtensionResults: () => ({
        prf: { results: { first: prf.buffer } },
      }),
    });
    await Bun.sleep(10);
    expect(calls.some((c) => c.includes("register/finish"))).toBe(false);
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: origNav,
    });
  });
});

describe("Gate login", () => {
  const origDocument = globalThis.document;
  const origFetch = globalThis.fetch;
  const origLocation = globalThis.location;
  const origHistory = globalThis.history;

  beforeEach(() => {
    installDom();
    const loc = { pathname: "/" };
    Object.defineProperty(globalThis, "location", {
      configurable: true,
      writable: true,
      value: loc,
    });
    Object.defineProperty(globalThis, "history", {
      configurable: true,
      writable: true,
      value: {
        pushState(_s: unknown, _t: unknown, url: string) {
          loc.pathname = String(url);
        },
      },
    });
  });

  afterEach(() => {
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      writable: true,
      value: origDocument,
    });
    Object.defineProperty(globalThis, "location", {
      configurable: true,
      writable: true,
      value: origLocation,
    });
    Object.defineProperty(globalThis, "history", {
      configurable: true,
      writable: true,
      value: origHistory,
    });
    globalThis.fetch = origFetch;
    clearDek();
  });

  function gateState(): AppState {
    return {
      path: signal("/"),
      email: signal("a@b.c"),
      password: signal("x"),
      error: signal(undefined),
      pending: signal(false),
      session: signal(undefined),
      method: signal("password"),
      different: signal(false),
      revealPassword: signal(false),
      userCode: signal(""),
      eph: signal(""),
      passkeys: signal(undefined),
    };
  }

  for (const status of [401, 500]) {
    test(`POST login 200 then GET /session ${status} keeps path /, shows .error, session undefined`, async () => {
      const root = document.createElement("div");
      globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = reqUrl(input);
        const method = String(init?.method ?? "GET");
        if (method === "POST" && url === "/api/auth/password/login") {
          return new Response("{}", { status: 200 });
        }
        if (method === "GET" && url === "/api/session") {
          return new Response("{}", { status });
        }
        return new Response("{}", { status: 200 });
      }) as unknown as typeof fetch;
      const state = gateState();
      renderAccount(state, root);
      render(state);
      (root.querySelector("form") as unknown as FakeNode | null)?.submit();
      await Bun.sleep(1);
      expect(state.path.get()).toBe("/");
      expect(state.session.get()).toBeUndefined();
      expect(state.error.get()).toBe(FAIL_SENTENCE);
      expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
    });
  }

  test("login with a CLI user code lands on the device screen", async () => {
    const root = document.createElement("div");
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "POST" && url === "/api/auth/password/login") {
        return new Response("{}", { status: 200 });
      }
      if (method === "GET" && url === "/api/session") {
        return new Response(
          JSON.stringify({
            email: "a@b.c",
            session_id: "s1",
            has_passkey: false,
            has_password: true,
          }),
          { status: 200 },
        );
      }
      return new Response("{}", { status: 200 });
    }) as unknown as typeof fetch;
    const state = gateState();
    state.userCode.set("ABCD-EFGH");
    state.eph.set("11".repeat(32));
    renderAccount(state, root);
    render(state);
    (root.querySelector("form") as unknown as FakeNode | null)?.submit();
    await Bun.sleep(1);
    expect(state.path.get()).toBe("/device");
    expect(state.session.get()?.session_id).toBe("s1");
    expect(root.querySelector('[data-page="device"]')).not.toBeNull();
    expect(root.querySelector("[data-device]")?.textContent).toBe("ABCD-EFGH");
    expect(root.querySelector("[data-session-id]")?.textContent).toBe("s1");
  });
});

describe("Register layout", () => {
  const origDocument = globalThis.document;
  const origWidth = globalThis.innerWidth;
  const hosts: object[] = [];

  beforeEach(() => {
    installDom();
    hosts.length = 0;
  });

  afterEach(() => {
    for (const h of hosts) {
      abandonRegister(h);
    }
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      writable: true,
      value: origDocument,
    });
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: origWidth,
    });
  });

  function registerState(): AppState {
    return {
      path: signal("/register"),
      email: signal(""),
      password: signal(""),
      error: signal(undefined),
      pending: signal(false),
      session: signal(undefined),
      method: signal(undefined),
      different: signal(false),
      revealPassword: signal(false),
      userCode: signal(""),
      eph: signal(""),
      passkeys: signal(undefined),
    };
  }

  test(">=900px is list | inspector and has no sheet", () => {
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 900,
    });
    const root = document.createElement("div");
    const state = registerState();
    hosts.push(state);
    renderRegister(state, root);
    expect(root.querySelector('[data-layout="list-inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="list"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();
  });

  test("below 900px is list-only: inspector stays in the tree, no sheet until a secret is selected", () => {
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 899,
    });
    const root = document.createElement("div");
    const state = registerState();
    hosts.push(state);
    renderRegister(state, root);
    expect(root.querySelector('[data-layout="list-only"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="list"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();
  });
});
