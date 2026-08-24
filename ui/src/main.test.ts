import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { LAST_FACTOR_SENTENCE } from "./lib/api.ts";
import { signal } from "./lib/signal.ts";
import {
  dekFactors,
  hrefFor,
  initialPath,
  lastFactor,
  renderAccount,
  resolveGate,
  screenFromPath,
  SCREENS,
  type AppState,
} from "./main.ts";

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
    passkeys: signal(q.passkeys),
  };
}

describe("Account chain", () => {
  const origDocument = globalThis.document;
  const origFetch = globalThis.fetch;

  beforeEach(() => {
    installDom();
  });

  afterEach(() => {
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      writable: true,
      value: origDocument,
    });
    globalThis.fetch = origFetch;
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
});
