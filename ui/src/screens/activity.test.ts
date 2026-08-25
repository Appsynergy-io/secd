import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { FAIL_SENTENCE, RATE_SENTENCE } from "../lib/api.ts";
import { signal } from "../lib/signal.ts";
import {
  CHAIN_BREAK,
  CLIP_FAIL_SENTENCE,
  COPY_NEED_SENTENCE,
  EMPTY_BODY,
  EMPTY_TITLE,
  LOADING_SENTENCE,
  UNVERIFIED,
  VERIFIED,
  ZERO_HASH,
  eventHash,
  leaveActivity,
  parseAudit,
  renderActivity,
  verifyChain,
  type ActivityHost,
} from "./activity.ts";

const PUT_HASH =
  "5157a919d53b19d0f49086fda3682a0b1e8dee4b7201a3f0bedfe927e41bfc84";
const REVOKE_HASH =
  "d8222657f5c0de2c34d798812132387a3e43b07778592dc65fd6fbaf715b2f46";

function host(): ActivityHost {
  return {
    path: signal("/activity"),
    error: signal(undefined),
    pending: signal(false),
  };
}

function eventsOk(): unknown {
  return {
    events: [
      { action: "vault.put", names: ["kv/a"], session_id: "s1" },
      { action: "session.revoke", names: [] },
    ],
  };
}

async function settled(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Bun.sleep(0);
}

describe("audit chain", () => {
  test("hashes prev|action|session|names from genesis the way audit.rs does", () => {
    const first = eventHash(ZERO_HASH, "vault.put", "s1", ["kv/a"]);
    expect(first).toBe(PUT_HASH);
    const second = eventHash(first, "session.revoke", undefined, []);
    expect(second).toBe(REVOKE_HASH);
  });

  test("a row with a stored hash that does not match is unverified", () => {
    const rows = verifyChain([
      {
        action: "vault.put",
        names: ["kv/a"],
        sessionId: "s1",
        prev: ZERO_HASH,
        hash: "0".repeat(64),
        ok: true,
      },
      {
        action: "session.revoke",
        names: [],
        sessionId: undefined,
        prev: undefined,
        hash: undefined,
        ok: true,
      },
    ]);
    expect(rows[0]?.verified).toBe(false);
    expect(rows[0]?.reason).toBe(CHAIN_BREAK);
    expect(rows[1]?.verified).toBe(false);
    expect(rows[0]?.hash).toBe(PUT_HASH);
    expect(rows[1]?.prev).toBe(PUT_HASH);
  });

  test("parseAudit reads the GET /api/v1/audit shape and ignores a value field", () => {
    const parsed = parseAudit({
      events: [
        {
          action: "vault.put",
          names: ["kv/a"],
          session_id: "s1",
          value: "secret-value-must-not-render",
        },
      ],
    });
    expect(parsed).toEqual([
      {
        action: "vault.put",
        names: ["kv/a"],
        sessionId: "s1",
        prev: undefined,
        hash: undefined,
        ok: true,
      },
    ]);
  });
});

describe("Activity screen", () => {
  const origFetch = globalThis.fetch;
  const origClipboard = globalThis.navigator?.clipboard;

  beforeAll(() => {
    try {
      GlobalRegistrator.register({
        url: "https://secd.imabee.com/activity",
        width: 1280,
        height: 800,
      });
    } catch {
      /* preload or another file already registered Happy DOM */
    }
  });

  afterEach(() => {
    globalThis.fetch = origFetch;
    if (origClipboard !== undefined) {
      Object.defineProperty(globalThis.navigator, "clipboard", {
        configurable: true,
        value: origClipboard,
      });
    }
  });

  test("loading, empty, and error each have a designed state", async () => {
    const root = document.createElement("div");
    const state = host();
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (() =>
      new Promise<Response>((resolve) => {
        finish = resolve;
      })) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 1280);
    expect(root.textContent).toContain(LOADING_SENTENCE);
    expect(root.querySelector('[data-state="loading"]')).not.toBeNull();
    expect(root.querySelector('[data-action="copy"]')?.hasAttribute("disabled")).toBe(
      true,
    );

    finish?.(new Response(JSON.stringify({ events: [] }), { status: 200 }));
    await settled();
    expect(root.textContent).toContain(EMPTY_TITLE);
    expect(root.textContent).toContain(EMPTY_BODY);
    expect(root.querySelector('[data-state="empty"]')).not.toBeNull();
    expect(root.querySelector('[data-action="copy"]')?.hasAttribute("disabled")).toBe(
      true,
    );
    expect(root.textContent).toContain(COPY_NEED_SENTENCE);

    leaveActivity(state);
    state.error.set(undefined);
    let n = 0;
    globalThis.fetch = (async () => {
      n += 1;
      return new Response("{}", { status: 500 });
    }) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    expect(n).toBe(1);
    expect(root.querySelector('[data-state="error"]')).not.toBeNull();
    expect(root.textContent).toContain(FAIL_SENTENCE);
  });

  test("429 uses the rate sentence", async () => {
    const root = document.createElement("div");
    const state = host();
    globalThis.fetch = (async () => new Response("{}", { status: 429 })) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    expect(root.textContent).toContain(RATE_SENTENCE);
  });

  test("GET /api/v1/audit rows recompute as a verified chain", async () => {
    const root = document.createElement("div");
    const state = host();
    const urls: string[] = [];
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
      urls.push(url);
      return new Response(JSON.stringify(eventsOk()), { status: 200 });
    }) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    expect(urls).toEqual(["/api/v1/audit"]);
    expect(root.querySelector('[data-chain-status="verified"]')?.textContent).toBe(VERIFIED);
    expect(root.querySelector('[data-seq="0"]')?.textContent).toContain("vault.put");
    expect(root.querySelector('[data-seq="0"] [data-name]')?.textContent).toBe("kv/a");
    expect(root.querySelector('[data-seq="1"]')?.textContent).toContain("session.revoke");
    expect(root.textContent).not.toContain("secret-value-must-not-render");
  });

  test("a stored hash mismatch paints unverified and keeps later rows unverified", async () => {
    const root = document.createElement("div");
    const state = host();
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          events: [
            {
              action: "vault.put",
              names: ["kv/a"],
              session_id: "s1",
              hash: "0".repeat(64),
            },
            { action: "session.revoke", names: [] },
          ],
        }),
        { status: 200 },
      )) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    expect(root.querySelector('[data-chain-status="unverified"]')?.textContent).toBe(
      UNVERIFIED,
    );
    expect(root.querySelector('[data-seq="0"]')?.getAttribute("data-verified")).toBe("0");
    expect(root.querySelector('[data-seq="1"]')?.getAttribute("data-verified")).toBe("0");
  });

  test("selecting a row shows its hash; Copy writes that hash and becomes Copied after writeText", async () => {
    const root = document.createElement("div");
    const state = host();
    globalThis.fetch = (async () =>
      new Response(JSON.stringify(eventsOk()), { status: 200 })) as unknown as typeof fetch;
    let wrote: string | undefined;
    let release: (() => void) | undefined;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) =>
          new Promise<void>((resolve) => {
            wrote = text;
            release = resolve;
          }),
      },
    });
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    const row = root.querySelector('[data-seq="0"]');
    expect(row).not.toBeNull();
    (row as HTMLButtonElement).click();
    expect(root.querySelector('[data-field="hash"]')?.textContent).toBe(PUT_HASH);
    expect(root.querySelector('[data-field="prev"]')?.textContent).toBe(ZERO_HASH);
    const copy = root.querySelector('[data-pane="inspector"] [data-action="copy"]');
    expect(copy?.textContent).toBe("Copy");
    (copy as HTMLButtonElement).click();
    await settled();
    expect(copy?.textContent).toBe("Copy");
    expect(wrote).toBe(PUT_HASH);
    release?.();
    await settled();
    const after = root.querySelector('[data-pane="inspector"] [data-action="copy"]');
    expect(after?.textContent).toBe("Copied");
    expect(wrote).toBe(PUT_HASH);
  });

  test("a refused clipboard keeps Copy and offers select-to-copy of the hash", async () => {
    const root = document.createElement("div");
    const state = host();
    globalThis.fetch = (async () =>
      new Response(JSON.stringify(eventsOk()), { status: 200 })) as unknown as typeof fetch;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async () => {
          throw new Error("denied");
        },
      },
    });
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    (root.querySelector('[data-seq="0"]') as HTMLButtonElement).click();
    document.body.append(root);
    (root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement).click();
    await settled();
    expect(root.querySelector('[data-pane="inspector"] [data-action="copy"]')?.textContent).toBe(
      "Copy",
    );
    expect(root.textContent).toContain(CLIP_FAIL_SENTENCE);
    expect(root.querySelector('[data-field="hash"]')?.textContent).toBe(PUT_HASH);
    const fallback = root.querySelector("[data-select-copy]") as HTMLInputElement | null;
    expect(fallback?.value).toBe(PUT_HASH);
    expect(fallback?.getAttribute("aria-label")).toBe("Event hash");
    expect(document.activeElement).toBe(fallback);
    root.remove();
  });

  test("list-inspector at 900px; list opens a sheet below", async () => {
    const root = document.createElement("div");
    const state = host();
    globalThis.fetch = (async () =>
      new Response(JSON.stringify(eventsOk()), { status: 200 })) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 900);
    await settled();
    expect(root.querySelector("[data-layout]")?.getAttribute("data-layout")).toBe(
      "list-inspector",
    );
    expect(root.querySelector('[data-pane="inspector"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();
    (root.querySelector('[data-seq="0"]') as HTMLButtonElement).click();
    expect(root.querySelector('[data-pane="sheet"]')?.getAttribute("data-sheet")).toBe("open");

    leaveActivity(state);
    const mobile = document.createElement("div");
    const mobileState = host();
    renderActivity(mobileState, mobile, document.createElement("nav"), 899);
    await settled();
    expect(mobile.querySelector("[data-layout]")?.getAttribute("data-layout")).toBe("list-only");
    (mobile.querySelector('[data-seq="0"]') as HTMLButtonElement).click();
    expect(mobile.querySelector('[data-pane="sheet"]')?.getAttribute("data-sheet")).toBe("open");
    expect(mobile.querySelector('[data-field="hash"]')?.textContent).toBe(PUT_HASH);
  });

  test("an in-flight GET is ignored after leaving Activity", async () => {
    const root = document.createElement("div");
    const state = host();
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (() =>
      new Promise<Response>((resolve) => {
        finish = resolve;
      })) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 1280);
    expect(root.textContent).toContain(LOADING_SENTENCE);
    state.path.set("/account");
    leaveActivity(state);
    finish?.(new Response(JSON.stringify(eventsOk()), { status: 200 }));
    await settled();
    expect(root.textContent).toContain(LOADING_SENTENCE);
    expect(root.querySelector('[data-seq="0"]')).toBeNull();
  });

  test("clipboard fallback lives in the inspector next to Copy", async () => {
    const root = document.createElement("div");
    const state = host();
    globalThis.fetch = (async () =>
      new Response(JSON.stringify(eventsOk()), { status: 200 })) as unknown as typeof fetch;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async () => {
          throw new Error("denied");
        },
      },
    });
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    (root.querySelector('[data-seq="0"]') as HTMLButtonElement).click();
    document.body.append(root);
    (root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement).click();
    await settled();
    const pane = root.querySelector('[data-pane="inspector"]');
    expect(pane?.querySelector(".error")?.textContent).toBe(CLIP_FAIL_SENTENCE);
    expect((pane?.querySelector("[data-select-copy]") as HTMLInputElement | null)?.value).toBe(
      PUT_HASH,
    );
    expect(root.querySelector(".secd-overlay + .error")).toBeNull();
    root.remove();
  });

  test("activity sheet is a dialog that inerts the page and restores row focus", async () => {
    const root = document.createElement("div");
    document.body.append(root);
    const state = host();
    globalThis.fetch = (async () =>
      new Response(JSON.stringify(eventsOk()), { status: 200 })) as unknown as typeof fetch;
    renderActivity(state, root, document.createElement("nav"), 899);
    await settled();
    (root.querySelector('[data-seq="0"]') as HTMLButtonElement).click();
    const overlay = root.querySelector('[data-pane="sheet"]');
    expect(overlay?.getAttribute("role")).toBe("dialog");
    expect(overlay?.getAttribute("aria-modal")).toBe("true");
    expect(root.querySelector("nav")?.hasAttribute("inert")).toBe(true);
    overlay?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );
    expect(root.querySelector('[data-pane="sheet"]')).toBeNull();
    expect(document.activeElement).toBe(root.querySelector('[data-seq="0"]'));
    root.remove();
  });

  test("Copy does not paint Activity after leave", async () => {
    const root = document.createElement("div");
    const state = host();
    let finish: ((value: void) => void) | undefined;
    globalThis.fetch = (async () =>
      new Response(JSON.stringify(eventsOk()), { status: 200 })) as unknown as typeof fetch;
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: () =>
          new Promise<void>((resolve) => {
            finish = resolve;
          }),
      },
    });
    renderActivity(state, root, document.createElement("nav"), 1280);
    await settled();
    (root.querySelector('[data-seq="0"]') as HTMLButtonElement).click();
    const hash = root.querySelector('[data-field="hash"]');
    (root.querySelector('[data-pane="inspector"] [data-action="copy"]') as HTMLButtonElement).click();
    state.path.set("/register");
    leaveActivity(state);
    finish?.();
    await settled();
    expect(root.querySelector('[data-field="hash"]')).toBe(hash);
    expect(root.querySelector('[data-pane="inspector"] [data-action="copy"]')?.textContent).toBe(
      "Copy",
    );
  });
});
