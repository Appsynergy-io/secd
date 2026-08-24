import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { FAIL_SENTENCE, NO_DEK_SENTENCE } from "../lib/api.ts";
import {
  clearDek,
  fromHex,
  getDek,
  mintDek,
  open,
  seal,
  setDek,
  toHex,
} from "../lib/crypto.ts";
import { signal } from "../lib/signal.ts";
import {
  CLIP_FAIL_SENTENCE,
  CLIP_MISSING_SENTENCE,
  CLIP_TTL_MS,
  EMPTY_SENTENCE,
  LOADING_SENTENCE,
  MASK,
  abandonRegister,
  renderRegister,
  type RegisterHost,
} from "./register.ts";

const NAME = "kv/gitea/token";
const FIELD = "fixture-field-value";
const USER = "fixture-user";

function host(): RegisterHost {
  return {
    path: signal("/register"),
    error: signal(undefined),
    pending: signal(false),
  };
}

function setPath(state: RegisterHost, path: string): void {
  (state.path as unknown as { set(v: string): void }).set(path);
}

function json(status: number, data: unknown): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function sealed(dek: Uint8Array, name: string, payload: Record<string, string>): string {
  return toHex(seal(dek, name, new TextEncoder().encode(JSON.stringify(payload))));
}

type Api = {
  puts: unknown[];
  posts: unknown[];
  vault: { entries: unknown[] };
};

function mockApi(vault: { entries: unknown[] }, versions: unknown = { versions: [] }): Api {
  const api: Api = { puts: [], posts: [], vault };
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = reqUrl(input);
    const method = String(init?.method ?? "GET");
    if (method === "GET" && url === "/api/v1/vault") {
      return json(200, api.vault);
    }
    if (method === "GET" && url === "/api/v1/vault/versions") {
      return json(200, versions);
    }
    if (method === "PUT" && url === "/api/v1/vault") {
      const body = JSON.parse(String(init?.body ?? "{}")) as unknown;
      api.puts.push(body);
      return json(200, { ok: true });
    }
    if (method === "POST" && url === "/api/v1/vault/rollback") {
      const body = JSON.parse(String(init?.body ?? "{}")) as unknown;
      api.posts.push(body);
      return json(200, { ok: true });
    }
    return json(404, {});
  }) as unknown as typeof fetch;
  return api;
}

async function flush(): Promise<void> {
  await Bun.sleep(1);
}

describe("Register screen", () => {
  const origFetch = globalThis.fetch;
  const origWidth = globalThis.innerWidth;
  const origClip = globalThis.navigator?.clipboard;
  let state: RegisterHost;
  let root: HTMLElement;

  beforeEach(() => {
    state = host();
    root = document.createElement("div");
    document.body.append(root);
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 1024,
    });
  });

  afterEach(() => {
    abandonRegister(state);
    root.remove();
    clearDek();
    globalThis.fetch = origFetch;
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: origWidth,
    });
    if (origClip !== undefined) {
      Object.defineProperty(globalThis.navigator, "clipboard", {
        configurable: true,
        value: origClip,
      });
    }
  });

  test("loading then empty", async () => {
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (() =>
      new Promise((resolve) => {
        finish = resolve;
      })) as unknown as typeof fetch;
    renderRegister(state, root);
    expect(root.textContent?.includes(LOADING_SENTENCE)).toBe(true);
    finish?.(json(200, { entries: [] }));
    await flush();
    expect(root.textContent?.includes(EMPTY_SENTENCE)).toBe(true);
  });

  test("GET failure is an error, not empty", async () => {
    globalThis.fetch = (async () => json(500, {})) as unknown as typeof fetch;
    renderRegister(state, root);
    await flush();
    expect(root.querySelector(".error")?.textContent).toBe(FAIL_SENTENCE);
    expect(root.textContent?.includes(EMPTY_SENTENCE)).toBe(false);
  });

  test("one row per meta.fields key; Copy writes the opened value", async () => {
    const dek = mintDek();
    setDek(dek);
    const ct = sealed(dek, NAME, { token: FIELD, user: USER });
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: ct,
          meta: { provider: "github", fields: ["token", "user"] },
        },
      ],
    });
    let wrote: string | undefined;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          wrote = text;
        },
      },
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    const token = root.querySelector('[data-field="token"]');
    const user = root.querySelector('[data-field="user"]');
    expect(token).not.toBeNull();
    expect(user).not.toBeNull();
    expect(token?.querySelector("[data-value]")?.textContent).toBe(MASK);
    const copy = token?.querySelector('[data-action="copy"]') as HTMLButtonElement | null;
    expect(copy?.textContent).toBe("Copy");
    copy?.click();
    await flush();
    expect(wrote === FIELD).toBe(true);
    expect(
      root.querySelector('[data-field="token"] [data-action="copy"]')?.textContent,
    ).toBe("Copied");
  });

  test("Copy stays Copy until writeText resolves", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    let wrote: string | undefined;
    let release: (() => void) | undefined;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          wrote = text;
          return new Promise<void>((resolve) => {
            release = () => {
              resolve();
            };
          });
        },
      },
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    await flush();
    expect(
      root.querySelector('[data-field="token"] [data-action="copy"]')?.textContent,
    ).toBe("Copy");
    expect(wrote === FIELD).toBe(true);
    release?.();
    await flush();
    expect(
      root.querySelector('[data-field="token"] [data-action="copy"]')?.textContent,
    ).toBe("Copied");
  });

  test("clipboard rejection offers select-to-copy of the opened value", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async () => {
          throw new Error("denied");
        },
      },
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    await flush();
    expect(
      root.querySelector('[data-field="token"] [data-action="copy"]')?.textContent,
    ).toBe("Copy");
    expect(root.textContent?.includes(CLIP_FAIL_SENTENCE)).toBe(true);
    const fallback = root.querySelector(
      "[data-select-copy]",
    ) as HTMLInputElement | null;
    expect(fallback?.value === FIELD).toBe(true);
    expect(fallback?.getAttribute("value") === FIELD).toBe(false);
    expect(fallback?.outerHTML.includes(FIELD)).toBe(false);
    expect(fallback?.getAttribute("aria-label")).toBe("Secret value");
    expect(document.activeElement).toBe(fallback);
  });

  test("missing clipboard API offers select-to-copy", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: undefined,
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    await flush();
    expect(root.textContent?.includes(CLIP_MISSING_SENTENCE)).toBe(true);
    const fallback = root.querySelector(
      "[data-select-copy]",
    ) as HTMLInputElement | null;
    expect(fallback?.value === FIELD).toBe(true);
    expect(fallback?.getAttribute("value") === FIELD).toBe(false);
  });

  test("press-and-hold reveals then hides the opened value", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    const valueEl = root.querySelector("[data-value]");
    const show = root.querySelector('[data-action="show"]');
    expect(valueEl?.textContent).toBe(MASK);
    show?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    expect(valueEl?.textContent === FIELD).toBe(true);
    document.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    expect(valueEl?.textContent).toBe(MASK);
  });

  test("paint during hold remasks and never writes the opened value", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    const show = root.querySelector('[data-action="show"]');
    show?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    expect(root.querySelector("[data-value]")?.textContent === FIELD).toBe(true);
    (root.querySelector('[data-action="add"]') as HTMLButtonElement | null)?.click();
    expect(root.querySelector('[data-field="token"] [data-value]')?.textContent).toBe(MASK);
    expect(root.querySelector("[data-value]")?.textContent === FIELD).toBe(false);
    document.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    expect(root.querySelector('[data-field="token"] [data-value]')?.textContent).toBe(MASK);
  });

  test("Space and Enter hold-to-reveal; repeat is ignored", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    const valueEl = root.querySelector("[data-value]");
    const show = root.querySelector('[data-action="show"]');
    expect(valueEl?.textContent).toBe(MASK);
    show?.dispatchEvent(
      new KeyboardEvent("keydown", { key: " ", bubbles: true, cancelable: true }),
    );
    expect(valueEl?.textContent === FIELD).toBe(true);
    show?.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: " ",
        bubbles: true,
        cancelable: true,
        repeat: true,
      }),
    );
    expect(valueEl?.textContent === FIELD).toBe(true);
    document.dispatchEvent(
      new KeyboardEvent("keyup", { key: " ", bubbles: true, cancelable: true }),
    );
    expect(valueEl?.textContent).toBe(MASK);
    show?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }),
    );
    expect(valueEl?.textContent === FIELD).toBe(true);
    document.dispatchEvent(
      new KeyboardEvent("keyup", { key: "Enter", bubbles: true, cancelable: true }),
    );
    expect(valueEl?.textContent).toBe(MASK);
  });

  test("no DEK disables Copy with the reason visible", async () => {
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: "ab",
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    expect(root.textContent?.includes(NO_DEK_SENTENCE)).toBe(true);
    expect(root.querySelector('[data-action="copy"]')?.hasAttribute("disabled")).toBe(
      true,
    );
  });

  test(">=900px is list | inspector with no sheet; below 900 a selection opens the sheet", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 900,
    });
    renderRegister(state, root);
    await flush();
    expect(root.querySelector('[data-layout="list-inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();
    abandonRegister(state);
    state = host();
    Object.defineProperty(globalThis, "innerWidth", {
      configurable: true,
      writable: true,
      value: 899,
    });
    renderRegister(state, root);
    await flush();
    expect(root.querySelector('[data-layout="list-only"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    expect(root.querySelector('[data-pane="sheet"][data-sheet="open"]')).not.toBeNull();
  });

  test("wizard Save PUTs ciphertext that opens to the filled fields", async () => {
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [] });
    renderRegister(state, root);
    await flush();
    (root.querySelector('[data-action="add"]') as HTMLButtonElement | null)?.click();
    expect(document.activeElement).toBe(root.querySelector("#secret_name"));
    const sel = root.querySelector("#provider") as HTMLSelectElement | null;
    expect(sel).not.toBeNull();
    sel!.value = "github";
    sel!.dispatchEvent(new Event("change", { bubbles: true }));
    const name = root.querySelector("#secret_name") as HTMLInputElement | null;
    name!.value = NAME;
    name!.dispatchEvent(new Event("input", { bubbles: true }));
    const token = root.querySelector(
      'input[data-wizard-field="token"]',
    ) as HTMLInputElement | null;
    expect(token?.id).toBe("wizard-token");
    expect(root.querySelector('label[for="wizard-token"]')).not.toBeNull();
    token!.value = FIELD;
    token!.dispatchEvent(new Event("input", { bubbles: true }));
    (root.querySelector('[data-action="save"]') as HTMLButtonElement | null)?.click();
    await flush();
    expect(api.puts.length).toBe(1);
    const body = api.puts[0] as { entries: Array<Record<string, unknown>> };
    const entry = body.entries[0];
    expect(entry?.name).toBe(NAME);
    expect(JSON.stringify(body).includes(FIELD)).toBe(false);
    const ct = entry?.ciphertext;
    expect(typeof ct).toBe("string");
    const held = getDek();
    expect(held).toBeDefined();
    const pt = open(held!, NAME, fromHex(String(ct)));
    const obj = JSON.parse(new TextDecoder().decode(pt)) as { token?: string };
    expect(obj.token === FIELD).toBe(true);
  });

  test("Roll back POSTs name and version", async () => {
    const dek = mintDek();
    setDek(dek);
    const api = mockApi(
      {
        entries: [
          {
            name: NAME,
            ciphertext: sealed(dek, NAME, { token: FIELD }),
            meta: { provider: "github", fields: ["token"] },
          },
        ],
      },
      {
        versions: [
          { version: 1, created: "2026-08-14T12:34:56Z" },
          { version: 2, created: "2026-08-14T13:00:00Z" },
        ],
      },
    );
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    const rb = root.querySelector('[data-action="rollback"]') as HTMLButtonElement | null;
    expect(rb).not.toBeNull();
    rb?.click();
    await flush();
    expect(api.posts).toEqual([{ name: NAME, version: 1 }]);
  });

  test("clearDek drops opened values and Copy no-ops", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    let wrote: string | undefined;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          wrote = text;
        },
      },
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    clearDek();
    await flush();
    expect(root.textContent?.includes(NO_DEK_SENTENCE)).toBe(true);
    expect(root.querySelector('[data-action="copy"]')?.hasAttribute("disabled")).toBe(
      true,
    );
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    await flush();
    expect(wrote === undefined).toBe(true);
  });

  test("Copy does not write after leave, and abandonRegister blanks the clipboard", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    let wrote: string | undefined;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          wrote = text;
        },
      },
    });
    renderRegister(state, root);
    await flush();
    (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
    await flush();
    setPath(state, "/");
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    await flush();
    expect(wrote === undefined).toBe(true);
    setPath(state, "/register");
    (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
    await flush();
    expect(wrote === FIELD).toBe(true);
    abandonRegister(state);
    await flush();
    expect(wrote === "").toBe(true);
  });

  test("Copy blanks the clipboard after the short TTL", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({
      entries: [
        {
          name: NAME,
          ciphertext: sealed(dek, NAME, { token: FIELD }),
          meta: { provider: "github", fields: ["token"] },
        },
      ],
    });
    let wrote: string | undefined;
    const origTimeout = globalThis.setTimeout;
    const due: Array<() => void> = [];
    globalThis.setTimeout = ((fn: TimerHandler, ms?: number, ...args: unknown[]) => {
      if (ms === CLIP_TTL_MS && typeof fn === "function") {
        due.push(() => {
          (fn as () => void)();
        });
        return 0 as unknown as ReturnType<typeof setTimeout>;
      }
      return origTimeout(fn as TimerHandler, ms, ...args);
    }) as typeof setTimeout;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          wrote = text;
        },
      },
    });
    try {
      renderRegister(state, root);
      await flush();
      (root.querySelector(`[data-name="${NAME}"]`) as HTMLButtonElement | null)?.click();
      await flush();
      (root.querySelector('[data-action="copy"]') as HTMLButtonElement | null)?.click();
      await flush();
      expect(wrote === FIELD).toBe(true);
      expect(due.length).toBe(1);
      due[0]?.();
      await flush();
      expect(wrote === "").toBe(true);
    } finally {
      globalThis.setTimeout = origTimeout;
    }
  });

  test("wizard Cancel and Save drop typed field values", async () => {
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [] });
    renderRegister(state, root);
    await flush();
    (root.querySelector('[data-action="add"]') as HTMLButtonElement | null)?.click();
    const sel = root.querySelector("#provider") as HTMLSelectElement | null;
    sel!.value = "github";
    sel!.dispatchEvent(new Event("change", { bubbles: true }));
    const token = root.querySelector(
      'input[data-wizard-field="token"]',
    ) as HTMLInputElement | null;
    token!.value = FIELD;
    token!.dispatchEvent(new Event("input", { bubbles: true }));
    expect(token?.getAttribute("value") === FIELD).toBe(false);
    expect(token?.outerHTML.includes(FIELD)).toBe(false);
    (root.querySelector('[data-action="cancel"]') as HTMLButtonElement | null)?.click();
    expect(root.querySelector("[data-wizard]")).toBeNull();
    (root.querySelector('[data-action="add"]') as HTMLButtonElement | null)?.click();
    const sel2 = root.querySelector("#provider") as HTMLSelectElement | null;
    sel2!.value = "github";
    sel2!.dispatchEvent(new Event("change", { bubbles: true }));
    const again = root.querySelector(
      'input[data-wizard-field="token"]',
    ) as HTMLInputElement | null;
    expect(again).not.toBeNull();
    expect(again?.value === "").toBe(true);
  });
});
