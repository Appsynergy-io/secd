import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
  FAIL_SENTENCE,
  NO_DEK_SENTENCE,
  NO_EPH_SENTENCE,
  RATE_SENTENCE,
  deviceQuery,
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
import { signal } from "../lib/signal.ts";
import {
  CLIP_FAIL_SENTENCE,
  NO_SESSION_SENTENCE,
  deviceDisabledReason,
  ephOk,
  renderDevice,
  seedDeviceQuery,
  type DeviceState,
} from "./device.ts";

type FakeNode = {
  tagName: string;
  nodeType: number;
  disabled: boolean;
  parent: FakeNode | null;
  children: FakeNode[];
  attrs: Map<string, string>;
  listeners: Map<string, Array<(ev?: { preventDefault(): void }) => void>>;
  text: string;
  value: string;
  setAttribute(name: string, value: string): void;
  getAttribute(name: string): string | null;
  hasAttribute(name: string): boolean;
  append(...nodes: Array<FakeNode | string>): void;
  replaceChildren(...nodes: Array<FakeNode | string>): void;
  addEventListener(type: string, fn: (ev?: { preventDefault(): void }) => void): void;
  click(): void;
  submit(): void;
  querySelector(sel: string): FakeNode | null;
  focus(): void;
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
    value: "",
    setAttribute(name: string, value: string) {
      node.attrs.set(name, value);
      if (name === "disabled") {
        node.disabled = true;
      }
      if (name === "value") {
        node.value = value;
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
    addEventListener(type: string, fn: (ev?: { preventDefault(): void }) => void) {
      const list = node.listeners.get(type) ?? [];
      list.push(fn);
      node.listeners.set(type, list);
    },
    click() {
      if (node.disabled) {
        return;
      }
      for (const fn of node.listeners.get("click") ?? []) {
        fn({ preventDefault() {} });
      }
    },
    submit() {
      const ev = { preventDefault() {} };
      for (const fn of node.listeners.get("submit") ?? []) {
        fn(ev);
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
    focus() {
      const doc = globalThis.document as unknown as { activeElement?: FakeNode };
      doc.activeElement = node;
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
  if (sel.startsWith("#")) {
    return node.getAttribute("id") === sel.slice(1);
  }
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

function installDom(): void {
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

function deviceSk(): Uint8Array {
  return new Uint8Array(32).fill(0x51);
}

function deviceEphHex(): string {
  return toHex(x25519Public(deviceSk()));
}

function makeState(q: {
  code?: string;
  eph?: string;
  session?: { email: string; session_id: string } | undefined;
  error?: string | undefined;
  pending?: boolean;
}): DeviceState {
  return {
    userCode: signal(q.code ?? ""),
    eph: signal(q.eph ?? ""),
    session: signal(q.session),
    error: signal(q.error),
    pending: signal(q.pending === true),
  };
}

function signedState(q: { code?: string; eph?: string } = {}): DeviceState {
  return makeState({
    code: q.code ?? "ABCD-EFGH",
    eph: q.eph ?? deviceEphHex(),
    session: { email: "a@b.c", session_id: "s1" },
  });
}

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

describe("device query", () => {
  test("ephOk accepts only a 32-byte hex public key", () => {
    expect(ephOk("11".repeat(32))).toBe(true);
    expect(ephOk("11".repeat(31))).toBe(false);
    expect(ephOk("")).toBe(false);
    expect(ephOk("zz".repeat(32))).toBe(false);
  });

  test("current and legacy query names both seed code and eph", () => {
    const eph = "ab".repeat(32);
    const current = makeState({});
    seedDeviceQuery(current, `?code=ABCD-EFGH&eph=${eph}`);
    expect(current.userCode.get()).toBe("ABCD-EFGH");
    expect(current.eph.get()).toBe(eph);
    const legacy = makeState({});
    seedDeviceQuery(legacy, `?user_code=WXYZ-1234&eph_pub=${eph}`);
    expect(legacy.userCode.get()).toBe("WXYZ-1234");
    expect(legacy.eph.get()).toBe(eph);
    expect(deviceQuery("?code=ABCD-EFGH&eph=aa")).toEqual({
      code: "ABCD-EFGH",
      eph: "aa",
    });
    expect(deviceQuery("user_code=OLD&eph_pub=bb")).toEqual({
      code: "OLD",
      eph: "bb",
    });
  });

  test("disabled reasons cover empty, session, and missing DEK", () => {
    const eph = "11".repeat(32);
    expect(
      deviceDisabledReason({
        userCode: "",
        eph,
        session: { email: "a@b.c", session_id: "s1" },
        hasDek: true,
      }),
    ).toBe(NO_EPH_SENTENCE);
    expect(
      deviceDisabledReason({
        userCode: "ABCD-EFGH",
        eph: "",
        session: { email: "a@b.c", session_id: "s1" },
        hasDek: true,
      }),
    ).toBe(NO_EPH_SENTENCE);
    expect(
      deviceDisabledReason({
        userCode: "ABCD-EFGH",
        eph,
        session: undefined,
        hasDek: true,
      }),
    ).toBe(NO_SESSION_SENTENCE);
    expect(
      deviceDisabledReason({
        userCode: "ABCD-EFGH",
        eph,
        session: { email: "a@b.c", session_id: "s1" },
        hasDek: false,
      }),
    ).toBe(NO_DEK_SENTENCE);
    expect(
      deviceDisabledReason({
        userCode: "ABCD-EFGH",
        eph,
        session: { email: "a@b.c", session_id: "s1" },
        hasDek: true,
      }),
    ).toBeUndefined();
  });
});

describe("device screen", () => {
  const origDocument = globalThis.document;
  const origFetch = globalThis.fetch;
  const origLocation = globalThis.location;
  const origNav = globalThis.navigator;
  const origWidth = globalThis.innerWidth;
  const origHistory = globalThis.history;

  beforeEach(() => {
    installDom();
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 900,
    });
    Object.defineProperty(globalThis, "location", {
      configurable: true,
      writable: true,
      value: { pathname: "/device", search: "" },
    });
    Object.defineProperty(globalThis, "history", {
      configurable: true,
      writable: true,
      value: {
        pushState(_s: unknown, _t: unknown, url: string) {
          (globalThis.location as { pathname: string }).pathname = String(url);
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
    globalThis.fetch = origFetch;
    clearDek();
  });

  test("empty link disables Approve with the CLI-link sentence", () => {
    const root = document.createElement("div");
    const state = makeState({
      session: { email: "a@b.c", session_id: "s1" },
    });
    setDek(mintDek());
    renderDevice(state, root);
    expect(root.querySelector('[data-state="empty"]')).not.toBeNull();
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(
      true,
    );
    expect(root.querySelector("[data-reason]")?.textContent).toBe(NO_EPH_SENTENCE);
    expect(root.querySelector("[data-device]")?.textContent).toBe("No device code.");
  });

  test("shows the device code and the approving session", () => {
    const root = document.createElement("div");
    const state = signedState();
    setDek(mintDek());
    renderDevice(state, root);
    expect(root.querySelector("[data-device]")?.textContent).toBe("ABCD-EFGH");
    expect(root.querySelector("[data-session-email]")?.textContent).toBe("a@b.c");
    expect(root.querySelector("[data-session-id]")?.textContent).toBe("s1");
    expect(root.querySelector('[data-state="ready"]')).not.toBeNull();
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(
      false,
    );
  });

  test("legacy query names fill the screen from location.search", () => {
    const eph = deviceEphHex();
    (globalThis.location as { search: string }).search = `?user_code=WXYZ-1234&eph_pub=${eph}`;
    const root = document.createElement("div");
    const state = makeState({
      session: { email: "op@secd.test", session_id: "sess-9" },
    });
    setDek(mintDek());
    renderDevice(state, root);
    expect(root.querySelector("[data-device]")?.textContent).toBe("WXYZ-1234");
    expect(root.querySelector("[data-session-id]")?.textContent).toBe("sess-9");
    expect(state.eph.get()).toBe(eph);
  });

  test("missing DEK disables Approve with the vault-key sentence", () => {
    const root = document.createElement("div");
    renderDevice(signedState(), root);
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(
      true,
    );
    expect(root.querySelector("[data-reason]")?.textContent).toBe(NO_DEK_SENTENCE);
  });

  test("no session disables Approve and names the empty session", () => {
    const root = document.createElement("div");
    const state = makeState({
      code: "ABCD-EFGH",
      eph: deviceEphHex(),
    });
    setDek(mintDek());
    renderDevice(state, root);
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(
      true,
    );
    expect(root.querySelector("[data-reason]")?.textContent).toBe(NO_SESSION_SENTENCE);
    expect(root.querySelector('[data-session="empty"]')?.textContent).toBe("No session.");
  });

  test("Copy writes the device code and becomes Copied only after writeText resolves", async () => {
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
    setDek(mintDek());
    const state = signedState();
    renderDevice(state, root);
    const btn = root.querySelector('[data-action="copy"]');
    expect(btn?.textContent).toBe("Copy");
    (btn as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(btn?.textContent).toBe("Copy");
    expect(wrote).toBe("ABCD-EFGH");
    finish?.();
    await Bun.sleep(1);
    expect(root.querySelector('[data-action="copy"]')?.textContent).toBe("Copied");
    expect(wrote).toBe("ABCD-EFGH");
  });

  test("clipboard refusal keeps Copy and offers select-to-copy", async () => {
    const root = document.createElement("div");
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {},
    });
    setDek(mintDek());
    renderDevice(signedState(), root);
    (root.querySelector('[data-action="copy"]') as unknown as FakeNode | null)?.click();
    await Bun.sleep(1);
    expect(root.querySelector('[data-action="copy"]')?.textContent).toBe("Copy");
    expect(root.querySelector(".error")?.textContent).toBe(CLIP_FAIL_SENTENCE);
  });

  test("Approve seals the DEK to the CLI eph and does not put the DEK on the wire", async () => {
    const root = document.createElement("div");
    const dek = mintDek();
    setDek(dek);
    const held = new Uint8Array(dek);
    const posts: unknown[] = [];
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      posts.push({
        method: String(init?.method ?? "GET"),
        url: reqUrl(input),
        body: init?.body,
      });
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    }) as unknown as typeof fetch;
    let approved = false;
    const state = signedState();
    renderDevice(state, root, () => {
      approved = true;
    });
    (root.querySelector("form") as unknown as FakeNode | null)?.submit();
    await Bun.sleep(1);
    expect(approved).toBe(true);
    expect(posts.length).toBe(1);
    const post = posts[0] as { method: string; url: string; body: string };
    expect(post.method).toBe("POST");
    expect(post.url).toBe("/api/v1/device/approve");
    const body = JSON.parse(post.body) as {
      user_code: string;
      sealed_dek: { eph_pub: string; blob: string };
    };
    expect(body.user_code).toBe("ABCD-EFGH");
    expect(typeof body.sealed_dek.eph_pub).toBe("string");
    expect(typeof body.sealed_dek.blob).toBe("string");
    const wire = post.body;
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
  });

  test("approve 401 paints FAIL_SENTENCE as selectable error", async () => {
    const root = document.createElement("div");
    setDek(mintDek());
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ error: FAIL_SENTENCE }), {
        status: 401,
      })) as unknown as typeof fetch;
    const state = signedState();
    renderDevice(state, root);
    (root.querySelector("form") as unknown as FakeNode | null)?.submit();
    await Bun.sleep(1);
    expect(root.querySelector('[data-state="error"]')).not.toBeNull();
    expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(
      false,
    );
  });

  test("approve 429 paints the rate sentence", async () => {
    const root = document.createElement("div");
    setDek(mintDek());
    globalThis.fetch = (async () =>
      new Response("{}", { status: 429 })) as unknown as typeof fetch;
    const state = signedState();
    renderDevice(state, root);
    (root.querySelector("form") as unknown as FakeNode | null)?.submit();
    await Bun.sleep(1);
    expect(root.querySelector(".error")?.textContent).toBe(RATE_SENTENCE);
  });

  test("already-approved body error is the rejection text", async () => {
    const root = document.createElement("div");
    setDek(mintDek());
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ error: "already approved" }), {
        status: 400,
      })) as unknown as typeof fetch;
    const state = signedState();
    renderDevice(state, root);
    (root.querySelector("form") as unknown as FakeNode | null)?.submit();
    await Bun.sleep(1);
    expect(root.querySelector(".error")?.textContent).toBe("already approved");
  });

  test("in-flight Approve disables the control", async () => {
    const root = document.createElement("div");
    setDek(mintDek());
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (() =>
      new Promise<Response>((resolve) => {
        finish = resolve;
      })) as unknown as typeof fetch;
    const state = signedState();
    renderDevice(state, root);
    (root.querySelector("form") as unknown as FakeNode | null)?.submit();
    await Bun.sleep(1);
    expect(root.querySelector('[data-state="loading"]')).not.toBeNull();
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(
      true,
    );
    finish?.(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    await Bun.sleep(1);
  });

  test("current query names fill the screen from location.search", () => {
    const eph = deviceEphHex();
    (globalThis.location as { search: string }).search = `?code=ABCD-EFGH&eph=${eph}`;
    const root = document.createElement("div");
    const state = makeState({
      session: { email: "a@b.c", session_id: "s1" },
    });
    setDek(mintDek());
    renderDevice(state, root);
    expect(root.querySelector("[data-device]")?.textContent).toBe("ABCD-EFGH");
    expect(state.eph.get().length).toBe(64);
    expect(ephOk(state.eph.get())).toBe(true);
  });
});

describe("device input", () => {
  const origNav = globalThis.navigator;

  afterEach(() => {
    clearDek();
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: origNav,
    });
  });

  test("typing in #user_code updates the code without remounting the field", () => {
    const root = document.createElement("div");
    document.body.append(root);
    const state = makeState({
      eph: deviceEphHex(),
      session: { email: "a@b.c", session_id: "s1" },
    });
    setDek(mintDek());
    renderDevice(state, root);
    const input = root.querySelector("#user_code") as HTMLInputElement | null;
    expect(input).not.toBeNull();
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(true);
    input!.focus();
    input!.value = "ABCD-EFGH";
    input!.setSelectionRange(4, 4);
    input!.dispatchEvent(new Event("input", { bubbles: true }));
    expect(root.querySelector("#user_code")).toBe(input);
    expect(document.activeElement).toBe(input);
    expect(input!.selectionStart).toBe(4);
    expect(state.userCode.get()).toBe("ABCD-EFGH");
    expect(root.querySelector("[data-device]")?.textContent).toBe("ABCD-EFGH");
    expect(root.querySelector('[data-action="approve"]')?.hasAttribute("disabled")).toBe(false);
    root.remove();
  });

  test("Copy does not paint Device after the page has left", async () => {
    const root = document.createElement("div");
    document.body.append(root);
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
    setDek(mintDek());
    renderDevice(signedState(), root);
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    root.replaceChildren();
    finish?.();
    await Bun.sleep(1);
    expect(root.querySelector('[data-page="device"]')).toBeNull();
    root.remove();
  });
});
