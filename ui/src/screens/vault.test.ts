import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import * as keyholder from "../lib/keyholder.ts";
import { NO_DEK_SENTENCE } from "../lib/api.ts";
import {
  clearDek,
  fromHex,
  mintDek,
  open,
  seal,
  setDek,
  toHex,
} from "../lib/crypto.ts";
import { asButton, asInput } from "../lib/dom.ts";
import { bumpLogoutGen } from "../lib/gen.ts";
import type { AppState, Host } from "../lib/host.ts";
import { signal } from "../lib/signal.ts";
import { stamp } from "../lib/time.ts";
import {
  BAD_NAME_SENTENCE,
  CLIP_MISSING_SENTENCE,
  CLIP_TTL_MS,
  COMMAND_COPIED_TOAST,
  DEFAULT_ENV,
  EMPTY_SENTENCE,
  LOAD_FAIL_SENTENCE,
  LOADING_SENTENCE,
  MASK_CHAR,
  NAME_FIRST_SENTENCE,
  NEW_SECRET_LABEL,
  NO_MATCH_SENTENCE,
  OPEN_FAIL_SENTENCE,
  PREIMAGE_SENTENCE,
  READBACK_SENTENCE,
  REQUIRED_SENTENCE,
  RESTORE_FAIL_SENTENCE,
  SAVED_TOAST,
  UNGROUPED,
  VERSIONS_FAIL_SENTENCE,
  WIZARD_SUB,
  cliLines,
  envDump,
  envName,
  filterEntries,
  groupEntries,
  groupOf,
  leaveVault,
  leafOf,
  mask,
  openEntry,
  parseProviders,
  parseVault,
  parseVersions,
  payloadFor,
  preimage,
  putEntries,
  renderVault,
  versionRows,
  type ProviderSchema,
  type VaultEntry,
} from "./vault.ts";

const CF = "prod/cloudflare";
const GH = "ci/github";
const LOOSE = "orphan";
const TOKEN = "cf_live_fixture_token";
const ACCOUNT = "acct-fixture-1";
const ZONE = "zone-fixture";
const GH_TOKEN = "ghp_fixture_token";
const UPDATED = "2026-08-28T09:12:00Z";
const CREATED_NEW = "2026-08-28T09:12:00Z";
const CREATED_OLD = "2026-01-09T08:20:00Z";

const SCHEMAS: ProviderSchema[] = [
  {
    name: "cloudflare",
    title: "Cloudflare",
    builtin: true,
    fields: [
      { key: "account_id", secret: false, optional: false, env: "CLOUDFLARE_ACCOUNT_ID" },
      { key: "api_token", secret: true, optional: false, env: "CLOUDFLARE_API_TOKEN" },
      { key: "zone_id", secret: false, optional: true, env: "CLOUDFLARE_ZONE_ID" },
    ],
  },
  {
    name: "github",
    title: "GitHub",
    builtin: true,
    fields: [
      { key: "token", secret: true, optional: false, env: "GITHUB_TOKEN" },
      { key: "user", secret: false, optional: true, env: "GITHUB_USER" },
    ],
  },
  {
    name: "forgejo",
    title: "Forgejo",
    builtin: false,
    fields: [
      { key: "token", secret: true, optional: false, env: "FORGEJO_TOKEN" },
      { key: "url", secret: false, optional: false, env: "FORGEJO_URL" },
    ],
  },
];

const CF_SCHEMA = SCHEMAS[0] as ProviderSchema;
const GH_SCHEMA = SCHEMAS[1] as ProviderSchema;
const FJ_SCHEMA = SCHEMAS[2] as ProviderSchema;

type Call = {
  method: string;
  url: string;
  body?: unknown;
  name: string | undefined;
};

type Api = {
  calls: Call[];
  puts: unknown[];
  posts: unknown[];
  vault: { entries: unknown[] };
  vaultStatus: number;
  providersStatus: number;
  versionsStatus: number;
  putStatus: number;
  rollbackStatus: number;
  versions: unknown;
  providers: unknown;
  nextVault: { status: number; data: unknown } | undefined;
};

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

function cfEntry(dek: Uint8Array): Record<string, unknown> {
  return {
    name: CF,
    ciphertext: sealed(dek, CF, { account_id: ACCOUNT, api_token: TOKEN, zone_id: ZONE }),
    meta: { provider: "cloudflare", fields: ["account_id", "api_token", "zone_id"] },
    version: 4,
    updated: UPDATED,
  };
}

function ghEntry(dek: Uint8Array): Record<string, unknown> {
  return {
    name: GH,
    ciphertext: sealed(dek, GH, { token: GH_TOKEN }),
    meta: { provider: "github", fields: ["token"] },
    version: 2,
    updated: UPDATED,
  };
}

function looseEntry(dek: Uint8Array): Record<string, unknown> {
  return {
    name: LOOSE,
    ciphertext: sealed(dek, LOOSE, { token: "loose-token" }),
    meta: { provider: "github", fields: ["token"] },
    version: 1,
    updated: null,
  };
}

function mockApi(vault: { entries: unknown[] }, extra: Partial<Api> = {}): Api {
  const api: Api = {
    calls: [],
    puts: [],
    posts: [],
    vault,
    vaultStatus: 200,
    providersStatus: 200,
    versionsStatus: 200,
    putStatus: 200,
    rollbackStatus: 200,
    versions: {
      versions: [
        { version: 1, created: CREATED_OLD, meta: { provider: "cloudflare" } },
        { version: 4, created: CREATED_NEW, meta: { provider: "cloudflare" } },
      ],
    },
    providers: { providers: SCHEMAS },
    nextVault: undefined,
    ...extra,
  };
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = reqUrl(input);
    const method = String(init?.method ?? "GET");
    const headers = new Headers(init?.headers);
    const name = headers.get("x-secd-name") ?? undefined;
    let body: unknown;
    if (init?.body !== undefined) {
      body = JSON.parse(String(init.body)) as unknown;
    }
    api.calls.push({ method, url, body, name });
    if (method === "GET" && url === "/api/v1/providers") {
      return json(api.providersStatus, api.providers);
    }
    if (method === "GET" && url === "/api/v1/vault") {
      if (api.nextVault !== undefined) {
        const queued = api.nextVault;
        api.nextVault = undefined;
        return json(queued.status, queued.data);
      }
      return json(api.vaultStatus, api.vault);
    }
    if (method === "GET" && url === "/api/v1/vault/versions") {
      return json(api.versionsStatus, api.versions);
    }
    if (method === "PUT" && url === "/api/v1/vault") {
      api.puts.push(body);
      if (api.putStatus === 200 && body !== undefined && typeof body === "object" && body !== null) {
        const entries = (body as { entries?: unknown }).entries;
        if (Array.isArray(entries)) {
          api.vault = { entries };
        }
      }
      return json(api.putStatus, { ok: true });
    }
    if (method === "POST" && url === "/api/v1/vault/rollback") {
      api.posts.push(body);
      return json(api.rollbackStatus, api.rollbackStatus === 200 ? { ok: true } : { error: "version" });
    }
    return json(404, {});
  }) as unknown as typeof fetch;
  return api;
}

async function settled(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

async function waitFor(pred: () => boolean): Promise<void> {
  for (let i = 0; i < 40; i++) {
    if (pred()) {
      return;
    }
    await settled();
  }
  throw new Error("waitFor: condition not met");
}

function appState(): AppState {
  return {
    path: signal("/vault"),
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
    counts: signal({}),
    toast: signal(""),
  };
}

function stubClipboard(): { wrote: string | undefined } {
  const box: { wrote: string | undefined } = { wrote: undefined };
  Object.defineProperty(globalThis.navigator, "clipboard", {
    configurable: true,
    value: {
      writeText: async (text: string) => {
        box.wrote = text;
      },
    },
  });
  return box;
}

type Mount = {
  state: AppState;
  root: HTMLElement;
  host: Host;
  actions: HTMLElement;
  flashes: string[];
  signedOut: { n: number };
};

function mount(split?: boolean): Mount {
  const state = appState();
  const actions = document.createElement("div");
  const root = document.createElement("div");
  const flashes: string[] = [];
  const signedOut = { n: 0 };
  const host: Host = {
    navigate: () => {},
    redraw: () => {},
    flash: (message: string) => {
      flashes.push(message);
    },
    signOut: async () => {
      signedOut.n += 1;
    },
    loadSession: async () => {},
    actions,
  };
  if (split === undefined) {
    document.body.append(root, actions);
  } else {
    const shell = document.createElement("div");
    shell.className = "shell";
    shell.setAttribute("data-split", String(split));
    shell.append(root);
    document.body.append(shell, actions);
  }
  return { state, root, host, actions, flashes, signedOut };
}

const origFetch = globalThis.fetch;
const origClip = globalThis.navigator.clipboard;
let current: Mount | undefined;

beforeEach(async () => {

  await keyholder.start();
  current = undefined;
});

afterEach(() => {
  if (current) {
    leaveVault(current.state);
  }
  current = undefined;
  clearDek();
  document.body.replaceChildren();
  globalThis.fetch = origFetch;
  Object.defineProperty(globalThis.navigator, "clipboard", {
    configurable: true,
    value: origClip,
  });
});

function start(split?: boolean): Mount {
  const m = mount(split);
  current = m;
  return m;
}

describe("vault helpers", () => {
  test("groupOf and leafOf split on the first slash", () => {
    expect(groupOf("prod/cloudflare")).toBe("prod");
    expect(leafOf("prod/cloudflare")).toBe("cloudflare");
    expect(groupOf("a/b/c")).toBe("a");
    expect(leafOf("a/b/c")).toBe("b/c");
    expect(groupOf("orphan")).toBe(UNGROUPED);
    expect(leafOf("orphan")).toBe("orphan");
    expect(UNGROUPED).toBe("ungrouped");
  });

  test("filterEntries matches name or provider", () => {
    const rows: VaultEntry[] = [
      {
        name: CF,
        ciphertext: "aa",
        meta: {},
        provider: "cloudflare",
        fieldKeys: ["api_token"],
        version: 1,
        updated: "",
      },
      {
        name: GH,
        ciphertext: "bb",
        meta: {},
        provider: "github",
        fieldKeys: ["token"],
        version: 1,
        updated: "",
      },
    ];
    expect(filterEntries(rows, "").map((e) => e.name)).toEqual([CF, GH]);
    expect(filterEntries(rows, "CLOUD").map((e) => e.name)).toEqual([CF]);
    expect(filterEntries(rows, "github").map((e) => e.name)).toEqual([GH]);
    expect(filterEntries(rows, "nope")).toEqual([]);
  });

  test("groupEntries keeps first-seen group order", () => {
    const rows: VaultEntry[] = [
      {
        name: CF,
        ciphertext: "",
        meta: {},
        provider: "cloudflare",
        fieldKeys: [],
        version: 1,
        updated: "",
      },
      {
        name: LOOSE,
        ciphertext: "",
        meta: {},
        provider: "github",
        fieldKeys: [],
        version: 1,
        updated: "",
      },
      {
        name: "prod/gitea",
        ciphertext: "",
        meta: {},
        provider: "gitea",
        fieldKeys: [],
        version: 1,
        updated: "",
      },
      {
        name: GH,
        ciphertext: "",
        meta: {},
        provider: "github",
        fieldKeys: [],
        version: 1,
        updated: "",
      },
    ];
    expect(groupEntries(rows).map((g) => g.name)).toEqual(["prod", UNGROUPED, "ci"]);
    expect(groupEntries(rows)[0]?.rows.map((e) => e.name)).toEqual([CF, "prod/gitea"]);
  });

  test("mask is bullets of min(max(len, 8), 24)", () => {
    expect(mask("")).toBe(MASK_CHAR.repeat(8));
    expect(mask("ab")).toBe(MASK_CHAR.repeat(8));
    expect(mask("abcdefghij")).toBe(MASK_CHAR.repeat(10));
    expect(mask("x".repeat(40))).toBe(MASK_CHAR.repeat(24));
    expect(MASK_CHAR).toBe("•");
  });

  test("envName, envDump and cliLines follow the schema", () => {
    expect(envName("api_token", CF_SCHEMA)).toBe("CLOUDFLARE_API_TOKEN");
    expect(envName("unknown", CF_SCHEMA)).toBe("UNKNOWN");
    expect(envName("token", undefined)).toBe("TOKEN");
    expect(
      envDump(
        ["account_id", "api_token", "missing"],
        { account_id: ACCOUNT, api_token: TOKEN },
        CF_SCHEMA,
      ),
    ).toBe(`CLOUDFLARE_ACCOUNT_ID=${ACCOUNT}\nCLOUDFLARE_API_TOKEN=${TOKEN}`);
    expect(cliLines(CF, CF_SCHEMA)).toEqual([
      `secd run --with CLOUDFLARE_ACCOUNT_ID=${CF} -- ./deploy.sh`,
      `secd info ${CF}`,
    ]);
    expect(cliLines("n", undefined)).toEqual([
      `secd run --with ${DEFAULT_ENV}=n -- ./deploy.sh`,
      "secd info n",
    ]);
  });

  test("versionRows sorts descending and marks the newest current", () => {
    const rows = versionRows([
      { version: 1, created: CREATED_OLD, provider: "cloudflare" },
      { version: 4, created: CREATED_NEW, provider: "rotated" },
    ]);
    expect(rows.map((v) => v.version)).toEqual([4, 1]);
    expect(rows[0]?.current).toBe(true);
    expect(rows[0]?.note).toBe("current");
    expect(rows[1]?.current).toBe(false);
    expect(rows[1]?.note).toBe("cloudflare");
    expect(rows[0]?.stamp).toBe(stamp(CREATED_NEW));
  });

  test("payloadFor is schema-ordered and refuses a missing required field", () => {
    expect(payloadFor(GH_SCHEMA, [])).toBeUndefined();
    expect(payloadFor(GH_SCHEMA, [["token", "  "]])).toBeUndefined();
    expect(payloadFor(GH_SCHEMA, [["token", GH_TOKEN]])).toEqual({ token: GH_TOKEN });
    expect(payloadFor(GH_SCHEMA, [["user", "ci"], ["token", GH_TOKEN]])).toEqual({
      token: GH_TOKEN,
      user: "ci",
    });
    const custom = payloadFor(FJ_SCHEMA, [
      ["url", "https://git.example"],
      ["token", "t1"],
    ]);
    expect(Object.keys(custom ?? {})).toEqual(["token", "url"]);
    expect(custom).toEqual({ token: "t1", url: "https://git.example" });
  });

  test("parseVault keeps the first named row and drops nameless ones", () => {
    const rows = parseVault({
      entries: [
        { name: CF, ciphertext: "aa", meta: { provider: "cloudflare", fields: ["api_token"] }, version: 4, updated: UPDATED },
        { ciphertext: "bb" },
        { name: CF, ciphertext: "cc" },
        { name: GH, ciphertext: "dd", meta: { fields: ["token"] } },
      ],
    });
    expect(rows.map((e) => e.name)).toEqual([CF, GH]);
    expect(rows[0]?.version).toBe(4);
    expect(rows[0]?.updated).toBe(UPDATED);
    expect(rows[0]?.fieldKeys).toEqual(["api_token"]);
    expect(rows[1]?.version).toBe(1);
    expect(rows[1]?.updated).toBe("");
  });

  test("preimage refuses a load that dropped a row", () => {
    expect(preimage({})).toBeUndefined();
    expect(preimage({ entries: [{ ciphertext: "aa" }, { name: CF, ciphertext: "bb", meta: {} }] })).toBeUndefined();
    expect(putEntries([{ name: CF, ciphertext: "aa", meta: { provider: "cloudflare" } }])).toEqual([
      { name: CF, ciphertext: "aa", meta: { provider: "cloudflare" } },
    ]);
    expect(preimage({ entries: [{ name: CF, ciphertext: "aa", meta: {} }] })).toEqual([
      { name: CF, ciphertext: "aa", meta: {} },
    ]);
  });

  test("parseVersions sorts descending and reads meta.provider", () => {
    const rows = parseVersions({
      versions: [
        { version: 1, created: CREATED_OLD, meta: { provider: "cloudflare" } },
        { version: 4, created: CREATED_NEW, meta: {} },
        { created: CREATED_NEW },
      ],
    });
    expect(rows.map((v) => v.version)).toEqual([4, 1]);
    expect(rows[0]?.provider).toBe("");
    expect(rows[1]?.provider).toBe("cloudflare");
  });

  test("parseProviders keeps custom schemas", () => {
    const rows = parseProviders({
      providers: [
        { name: "forgejo", title: "Forgejo", builtin: false, fields: [{ key: "token", secret: true, env: "FORGEJO_TOKEN" }] },
        { name: "forgejo", title: "dup" },
        { title: "no-name" },
      ],
    });
    expect(rows).toEqual([
      {
        name: "forgejo",
        title: "Forgejo",
        builtin: false,
        fields: [{ key: "token", secret: true, optional: false, env: "FORGEJO_TOKEN" }],
      },
    ]);
  });

  test("openEntry shapes the holder's plaintext and fails closed", () => {
    const dek = mintDek();
    const entry: VaultEntry = {
      name: CF,
      ciphertext: sealed(dek, CF, { account_id: ACCOUNT, api_token: TOKEN }),
      meta: {},
      provider: "cloudflare",
      fieldKeys: ["account_id", "api_token"],
      version: 1,
      updated: "",
    };
    const plain = JSON.stringify({ account_id: ACCOUNT, api_token: TOKEN });
    expect(openEntry(plain, entry).fields).toEqual({ account_id: ACCOUNT, api_token: TOKEN });
    // A blob the holder could not open, and a multi-field blob that is not an
    // object, both fail closed rather than showing a partial entry.
    expect(openEntry(null, entry).error).toBe(OPEN_FAIL_SENTENCE);
    expect(openEntry("not json", entry).error).toBe(OPEN_FAIL_SENTENCE);
    const sibling: VaultEntry = {
      name: "kv/github/token",
      ciphertext: "",
      meta: {},
      provider: "github",
      fieldKeys: ["token"],
      version: 1,
      updated: "",
    };
    expect(openEntry("ghp_fixture", sibling)).toEqual({ fields: { token: "ghp_fixture" } });
  });
});

describe("vault screen", () => {
  test("loading then empty", async () => {
    const m = start();
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = reqUrl(input);
      if (url === "/api/v1/providers") {
        return json(200, { providers: SCHEMAS });
      }
      return new Promise<Response>((resolve) => {
        finish = resolve;
      });
    }) as unknown as typeof fetch;
    renderVault(m.state, m.root, m.host);
    expect(m.root.textContent?.includes(LOADING_SENTENCE)).toBe(true);
    finish?.(json(200, { entries: [] }));
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    expect(m.root.querySelector(".vault")?.getAttribute("data-view")).toBe("list");
    expect(m.state.counts.get().vault).toBe(0);
  });

  test("GET failure is Secrets did not load", async () => {
    const m = start();
    mockApi({ entries: [] }, { vaultStatus: 500 });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(LOAD_FAIL_SENTENCE) === true);
    expect(m.root.querySelector('[role="alert"]')?.textContent).toBe(LOAD_FAIL_SENTENCE);
    expect(m.root.textContent?.includes(EMPTY_SENTENCE)).toBe(false);
    expect(m.signedOut.n).toBe(0);
  });

  test("401 on GET /api/v1/vault signs out", async () => {
    const m = start();
    mockApi({ entries: [] }, { vaultStatus: 401 });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.signedOut.n === 1);
    expect(m.root.textContent?.includes(EMPTY_SENTENCE)).toBe(false);
  });

  test("403 on GET /api/v1/providers signs out", async () => {
    const m = start();
    mockApi({ entries: [] }, { providersStatus: 403 });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.signedOut.n === 1);
  });

  test("paints grouped rows, copy, chips and selects the first visible entry", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [cfEntry(dek), looseEntry(dek), ghEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector(`[data-name="${CF}"]`) !== null);
    expect(api.calls.some((c) => c.method === "GET" && c.url === "/api/v1/vault")).toBe(true);
    expect(api.calls.some((c) => c.method === "GET" && c.url === "/api/v1/providers")).toBe(true);
    expect(m.root.querySelector(".vault")?.className).toBe("vault");
    expect(m.root.querySelector(".list-pane")).not.toBeNull();
    expect(m.root.querySelector(".detail-pane")).not.toBeNull();
    const search = asInput(m.root.querySelector("[data-search]"));
    expect(search?.placeholder).toBe("Filter secrets");
    const groups = [...m.root.querySelectorAll("[role='group']")].map((n) => n.getAttribute("aria-label"));
    expect(groups).toEqual(["prod", UNGROUPED, "ci"]);
    const first = asButton(m.root.querySelector(`[data-name="${CF}"]`));
    expect(first?.getAttribute("aria-selected")).toBe("true");
    expect(first?.querySelector(".row-leaf")?.textContent).toBe("cloudflare");
    expect(first?.querySelector(".row-sub")?.textContent).toBe(`Cloudflare · ${stamp(UPDATED)}`);
    expect(first?.querySelector(".row-version")?.textContent).toBe("v4");
    expect(first?.querySelector(".row-bar")).not.toBeNull();
    expect(m.root.querySelector(`[data-name="${LOOSE}"] .row-leaf`)?.textContent).toBe(LOOSE);
    expect(m.root.querySelector("[data-secret-name]")?.textContent).toBe(CF);
    expect(m.root.querySelector("[data-provider]")?.textContent).toBe("Cloudflare");
    expect(m.root.querySelector("[data-field-count]")?.textContent).toBe("3 fields");
    expect(m.root.querySelector("[data-version-line]")?.textContent).toBe(`version 4 · ${stamp(UPDATED)}`);
    expect(m.state.counts.get().vault).toBe(3);
    expect(m.host.actions.querySelector("[data-action='new']")?.className).toBe("btn btn-primary btn-md");
    expect(m.host.actions.textContent).toBe(NEW_SECRET_LABEL);
  });

  test("filter empty is No names match", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector(`[data-name="${CF}"]`) !== null);
    const search = asInput(m.root.querySelector("[data-search]"));
    search!.value = "zzz";
    search!.dispatchEvent(new Event("input", { bubbles: true }));
    expect(m.root.textContent?.includes(NO_MATCH_SENTENCE)).toBe(true);
  });

  test("selecting a row sets data-view=detail; back returns to list when not split", async () => {
    const m = start(false);
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek), ghEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector(`[data-name="${GH}"]`) !== null);
    expect(m.root.querySelector(".vault")?.getAttribute("data-view")).toBe("list");
    expect(m.root.querySelector("[data-action='back']")?.textContent).toBe("‹  All secrets");
    asButton(m.root.querySelector(`[data-name="${GH}"]`))?.click();
    await waitFor(() => m.root.querySelector("[data-secret-name]")?.textContent === GH);
    expect(m.root.querySelector(".vault")?.getAttribute("data-view")).toBe("detail");
    expect(m.root.querySelector(`[data-name="${GH}"]`)?.getAttribute("aria-selected")).toBe("true");
    asButton(m.root.querySelector("[data-action='back']"))?.click();
    expect(m.root.querySelector(".vault")?.getAttribute("data-view")).toBe("list");
  });

  test("back is omitted when the shell is split", async () => {
    const m = start(true);
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector(`[data-name="${CF}"]`) !== null);
    expect(m.root.querySelector("[data-action='back']")).toBeNull();
  });

  test("GET versions sends x-secd-name and paints Current/Restore", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [cfEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector("[data-action='restore']") !== null);
    const ver = api.calls.find((c) => c.method === "GET" && c.url === "/api/v1/vault/versions");
    expect(ver?.name).toBe(CF);
    expect(m.root.querySelector("[data-version='4'] .version-no")?.textContent).toBe("v4");
    expect(m.root.querySelector("[data-version='4'] .cell-muted")?.textContent).toBe("current");
    const currentBtn = asButton(m.root.querySelector("[data-version='4'] [data-action='current']"));
    expect(currentBtn?.textContent).toBe("Current");
    expect(currentBtn?.disabled).toBe(true);
    expect(m.root.querySelector("[data-version='1'] .cell-muted")?.textContent).toBe("cloudflare");
    expect(m.root.querySelector("[data-version='1'] [data-action='restore']")?.textContent).toBe("Restore");
  });

  test("versions GET failure is Versions did not load", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] }, { versionsStatus: 500 });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(VERSIONS_FAIL_SENTENCE) === true);
  });

  test("Restore POSTs rollback, toasts, and reloads", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [cfEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector("[data-action='restore']") !== null);
    asButton(m.root.querySelector("[data-action='restore']"))?.click();
    await waitFor(() => m.flashes.includes(`Restored ${CF} to v1`));
    expect(api.posts).toEqual([{ name: CF, version: 1 }]);
    expect(api.calls.filter((c) => c.method === "GET" && c.url === "/api/v1/vault").length).toBeGreaterThan(1);
  });

  test("restore 404 is That version was not restored", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] }, { rollbackStatus: 404 });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector("[data-action='restore']") !== null);
    asButton(m.root.querySelector("[data-action='restore']"))?.click();
    await waitFor(() => m.root.textContent?.includes(RESTORE_FAIL_SENTENCE) === true);
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(RESTORE_FAIL_SENTENCE);
    expect(m.flashes).toEqual([]);
  });

  test("401 on rollback signs out", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] }, { rollbackStatus: 401 });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector("[data-action='restore']") !== null);
    asButton(m.root.querySelector("[data-action='restore']"))?.click();
    await waitFor(() => m.signedOut.n === 1);
  });

  test("secret fields mask; non-secret fields are Shown", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-field="api_token"]') !== null);
    expect(m.root.querySelector('[data-field="api_token"] [data-value]')?.textContent).toBe(mask(TOKEN));
    expect(m.root.querySelector('[data-field="api_token"] [data-action="reveal"]')?.textContent).toBe("Reveal");
    expect(m.root.querySelector('[data-field="account_id"] [data-value]')?.textContent).toBe(ACCOUNT);
    expect(m.root.querySelector('[data-field="account_id"] [data-action="reveal"]')?.textContent).toBe("Shown");
    expect(asButton(m.root.querySelector('[data-field="account_id"] [data-action="reveal"]'))?.disabled).toBe(true);
    expect(m.root.querySelector('[data-field="account_id"] .field-key-env')?.textContent).toBe("CLOUDFLARE_ACCOUNT_ID");
    expect(m.root.querySelector("[data-card='fields'] .grid-head")?.textContent).toContain("Field");
    expect(m.root.querySelector("[data-card='fields'] .grid-head")?.textContent).toContain("Value");
    asButton(m.root.querySelector('[data-field="api_token"] [data-action="reveal"]'))?.click();
    expect(m.root.querySelector('[data-field="api_token"] [data-value]')?.textContent).toBe(TOKEN);
    expect(m.root.querySelector('[data-field="api_token"] [data-action="reveal"]')?.textContent).toBe("Hide");
    expect(m.root.querySelector("[data-action='reveal-all']")?.textContent).toBe("Hide all");
    asButton(m.root.querySelector("[data-action='reveal-all']"))?.click();
    expect(m.root.querySelector('[data-field="api_token"] [data-value]')?.textContent).toBe(mask(TOKEN));
    expect(m.root.querySelector("[data-action='reveal-all']")?.textContent).toBe("Reveal all");
  });

  test("Copy a field toasts and blanks the clipboard after 30s", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    const clip = stubClipboard();
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
    try {
      renderVault(m.state, m.root, m.host);
      await waitFor(() => m.root.querySelector('[data-action="copy"]') !== null);
      asButton(m.root.querySelector('[data-field="api_token"] [data-action="copy"]'))?.click();
      await waitFor(() => m.flashes.includes("Copied api_token · cleared from the clipboard in 30s"));
      expect(clip.wrote).toBe(TOKEN);
      expect(CLIP_TTL_MS).toBe(30_000);
      expect(due.length).toBe(1);
      due[0]?.();
      await waitFor(() => clip.wrote === "");
    } finally {
      globalThis.setTimeout = origTimeout;
    }
  });

  test("Copy .env dumps schema env names and toasts", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    const clip = stubClipboard();
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector("[data-action='copy-env']") !== null);
    asButton(m.root.querySelector("[data-action='copy-env']"))?.click();
    await waitFor(() => m.flashes.includes("Copied 3 values as .env"));
    expect(clip.wrote).toBe(
      `CLOUDFLARE_ACCOUNT_ID=${ACCOUNT}\nCLOUDFLARE_API_TOKEN=${TOKEN}\nCLOUDFLARE_ZONE_ID=${ZONE}`,
    );
  });

  test("CLI card copy toasts Command copied and does not leak values", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    const clip = stubClipboard();
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector("[data-card='cli']") !== null);
    expect(m.root.querySelector("[data-card='cli'] .card-title")?.textContent).toBe("Use it without reading it");
    expect(m.root.querySelector("[data-line='0'] .cli-text")?.textContent).toBe(
      `secd run --with CLOUDFLARE_ACCOUNT_ID=${CF} -- ./deploy.sh`,
    );
    expect(m.root.querySelector("[data-line='1'] .cli-text")?.textContent).toBe(`secd info ${CF}`);
    asButton(m.root.querySelector("[data-line='0'] [data-action='copy-cli']"))?.click();
    await waitFor(() => m.flashes.includes(COMMAND_COPIED_TOAST));
    expect(clip.wrote).toBe(`secd run --with CLOUDFLARE_ACCOUNT_ID=${CF} -- ./deploy.sh`);
    expect(m.flashes.every((t) => !t.includes(TOKEN))).toBe(true);
  });

  test("open failure in detail is This secret could not be opened", async () => {
    const m = start();
    setDek(mintDek());
    mockApi({
      entries: [
        {
          name: CF,
          ciphertext: "00",
          meta: { provider: "cloudflare", fields: ["api_token"] },
          version: 1,
          updated: UPDATED,
        },
      ],
    });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(OPEN_FAIL_SENTENCE) === true);
  });

  test("no DEK shows the no-key sentence and disables copy", async () => {
    const m = start();
    mockApi({
      entries: [
        {
          name: CF,
          ciphertext: "aa",
          meta: { provider: "cloudflare", fields: ["api_token"] },
          version: 1,
          updated: UPDATED,
        },
      ],
    });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(NO_DEK_SENTENCE) === true);
    expect(asButton(m.root.querySelector("[data-action='copy-env']"))?.disabled).toBe(true);
  });

  test("clearDek drops plaintext and repaints", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-field="api_token"]') !== null);
    asButton(m.root.querySelector('[data-field="api_token"] [data-action="reveal"]'))?.click();
    expect(m.root.textContent?.includes(TOKEN)).toBe(true);
    clearDek();
    expect(m.root.textContent?.includes(TOKEN)).toBe(false);
    expect(m.root.textContent?.includes(NO_DEK_SENTENCE)).toBe(true);
  });

  test("New secret wizard copy, inert page, focus, Escape and Cancel", async () => {
    const m = start();
    mockApi({ entries: [] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    const overlay = m.root.querySelector(".overlay");
    expect(overlay?.className).toBe("overlay");
    expect(overlay?.querySelector(".modal")).not.toBeNull();
    expect(overlay?.getAttribute("role")).toBe("dialog");
    expect(overlay?.getAttribute("aria-modal")).toBe("true");
    expect(m.root.querySelector("#wizard-title")?.textContent).toBe(NEW_SECRET_LABEL);
    expect(m.root.querySelector(".modal-sub")?.textContent).toBe(WIZARD_SUB);
    expect(m.root.querySelector(".modal-route")?.textContent).toBe("PUT /api/v1/vault");
    expect(asInput(m.root.querySelector("#wizard-name"))?.placeholder).toBe("prod/github");
    expect(document.activeElement).toBe(m.root.querySelector("#wizard-provider"));
    expect(m.root.querySelector(".vault")?.hasAttribute("inert")).toBe(true);
    const opts = [...m.root.querySelectorAll("#wizard-provider option")].map((o) => (o as HTMLOptionElement).value);
    expect(opts).toEqual(["cloudflare", "github", "forgejo"]);
    overlay?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    expect(m.root.querySelector(".overlay")).toBeNull();
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    asButton(m.root.querySelector("[data-action='cancel']"))?.click();
    expect(m.root.querySelector(".overlay")).toBeNull();
  });

  test("wizard validation sentences", async () => {
    const m = start();
    setDek(mintDek());
    mockApi({ entries: [] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(NAME_FIRST_SENTENCE);
    const name = asInput(m.root.querySelector("#wizard-name"));
    name!.value = "has space";
    name!.dispatchEvent(new Event("input", { bubbles: true }));
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(BAD_NAME_SENTENCE);
    name!.value = "prod/github";
    name!.dispatchEvent(new Event("input", { bubbles: true }));
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(REQUIRED_SENTENCE);
  });

  test("Seal and save PUTs hex ciphertext and metadata only, then read-back verifies", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    const sel = m.root.querySelector("#wizard-provider") as HTMLSelectElement;
    sel.value = "github";
    sel.dispatchEvent(new Event("change", { bubbles: true }));
    const name = asInput(m.root.querySelector("#wizard-name"));
    name!.value = GH;
    name!.dispatchEvent(new Event("input", { bubbles: true }));
    const token = asInput(m.root.querySelector('[data-wizard-field="token"] input'));
    expect(token?.getAttribute("type")).toBe("password");
    expect(token?.placeholder).toBe("sealed before it leaves this browser");
    token!.value = GH_TOKEN;
    token!.dispatchEvent(new Event("input", { bubbles: true }));
    expect(token?.getAttribute("value") === GH_TOKEN).toBe(false);
    expect(token?.outerHTML.includes(GH_TOKEN)).toBe(false);
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    await waitFor(() => m.flashes.includes(SAVED_TOAST));
    expect(api.puts).toHaveLength(1);
    const body = api.puts[0] as { entries: Array<Record<string, unknown>> };
    expect(body.entries).toHaveLength(1);
    const entry = body.entries[0];
    expect(entry?.name).toBe(GH);
    expect(typeof entry?.ciphertext).toBe("string");
    expect(entry?.meta).toEqual({ provider: "github", fields: ["token"] });
    const dumped = JSON.stringify(body);
    expect(dumped.includes(GH_TOKEN)).toBe(false);
    expect(dumped.toLowerCase().includes("dek")).toBe(false);
    const pt = open(dek, GH, fromHex(String(entry?.ciphertext)));
    expect(JSON.parse(new TextDecoder().decode(pt))).toEqual({ token: GH_TOKEN });
    expect(m.root.querySelector(".overlay")).toBeNull();
    await waitFor(() => m.root.querySelector("[data-secret-name]")?.textContent === GH);
    expect(m.root.querySelector(".vault")?.getAttribute("data-view")).toBe("detail");
    const put = api.calls.find((c) => c.method === "PUT" && c.url === "/api/v1/vault");
    expect(put?.body).toEqual(body);
    expect(api.calls.filter((c) => c.method === "GET" && c.url === "/api/v1/vault").length).toBeGreaterThanOrEqual(3);
  });

  test("custom provider schema from GET /api/v1/providers is used to seal", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    const sel = m.root.querySelector("#wizard-provider") as HTMLSelectElement;
    sel.value = "forgejo";
    sel.dispatchEvent(new Event("change", { bubbles: true }));
    expect(m.root.querySelector('[data-wizard-field="url"]')).not.toBeNull();
    expect(m.root.querySelector('[data-wizard-field="token"] .field-env')?.textContent).toBe("FORGEJO_TOKEN");
    const name = asInput(m.root.querySelector("#wizard-name"));
    name!.value = "prod/forgejo";
    name!.dispatchEvent(new Event("input", { bubbles: true }));
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.value = "fj-token";
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.dispatchEvent(
      new Event("input", { bubbles: true }),
    );
    asInput(m.root.querySelector('[data-wizard-field="url"] input'))!.value = "https://git.example";
    asInput(m.root.querySelector('[data-wizard-field="url"] input'))!.dispatchEvent(
      new Event("input", { bubbles: true }),
    );
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    await waitFor(() => api.puts.length === 1);
    const body = api.puts[0] as { entries: Array<Record<string, unknown>> };
    const entry = body.entries[0];
    expect(entry?.meta).toEqual({ provider: "forgejo", fields: ["token", "url"] });
    expect(JSON.stringify(body).includes("fj-token")).toBe(false);
    const pt = open(dek, "prod/forgejo", fromHex(String(entry?.ciphertext)));
    expect(JSON.parse(new TextDecoder().decode(pt))).toEqual({
      token: "fj-token",
      url: "https://git.example",
    });
  });

  test("read-back policy refuses a dirty load and does not PUT", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    const sel = m.root.querySelector("#wizard-provider") as HTMLSelectElement;
    sel.value = "github";
    sel.dispatchEvent(new Event("change", { bubbles: true }));
    const name = asInput(m.root.querySelector("#wizard-name"));
    name!.value = GH;
    name!.dispatchEvent(new Event("input", { bubbles: true }));
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.value = GH_TOKEN;
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.dispatchEvent(
      new Event("input", { bubbles: true }),
    );
    api.nextVault = { status: 200, data: { entries: [{ ciphertext: "aa" }, { name: "x", ciphertext: "bb", meta: {} }] } };
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    await waitFor(() => m.root.textContent?.includes(PREIMAGE_SENTENCE) === true);
    expect(api.puts).toEqual([]);
    expect(m.flashes).toEqual([]);
  });

  test("read-back refuses when the saved row is missing", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    const api = mockApi({ entries: [] });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    const sel = m.root.querySelector("#wizard-provider") as HTMLSelectElement;
    sel.value = "github";
    sel.dispatchEvent(new Event("change", { bubbles: true }));
    asInput(m.root.querySelector("#wizard-name"))!.value = GH;
    asInput(m.root.querySelector("#wizard-name"))!.dispatchEvent(new Event("input", { bubbles: true }));
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.value = GH_TOKEN;
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.dispatchEvent(
      new Event("input", { bubbles: true }),
    );
    const origFetch = globalThis.fetch;
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = reqUrl(input);
      const method = String(init?.method ?? "GET");
      if (method === "PUT" && url === "/api/v1/vault") {
        const res = await origFetch(input, init);
        api.vault = { entries: [] };
        return res;
      }
      return origFetch(input, init);
    }) as unknown as typeof fetch;
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    await waitFor(() => m.root.textContent?.includes(READBACK_SENTENCE) === true);
    expect(m.flashes).toEqual([]);
  });

  test("401 on PUT signs out", async () => {
    const m = start();
    setDek(mintDek());
    mockApi({ entries: [] }, { putStatus: 401 });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    asButton(m.host.actions.querySelector("[data-action='new']"))?.click();
    const sel = m.root.querySelector("#wizard-provider") as HTMLSelectElement;
    sel.value = "github";
    sel.dispatchEvent(new Event("change", { bubbles: true }));
    asInput(m.root.querySelector("#wizard-name"))!.value = GH;
    asInput(m.root.querySelector("#wizard-name"))!.dispatchEvent(new Event("input", { bubbles: true }));
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.value = GH_TOKEN;
    asInput(m.root.querySelector('[data-wizard-field="token"] input'))!.dispatchEvent(
      new Event("input", { bubbles: true }),
    );
    asButton(m.root.querySelector("[data-action='save']"))?.click();
    await waitFor(() => m.signedOut.n === 1);
  });

  test("missing clipboard toasts the missing sentence", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    Object.defineProperty(globalThis.navigator, "clipboard", {
      configurable: true,
      value: undefined,
    });
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-action="copy"]') !== null);
    asButton(m.root.querySelector('[data-field="api_token"] [data-action="copy"]'))?.click();
    await waitFor(() => m.flashes.includes(CLIP_MISSING_SENTENCE));
  });

  test("leaveVault drops timers, plaintext and pending responses", async () => {
    const m = start();
    const dek = mintDek();
    setDek(dek);
    mockApi({ entries: [cfEntry(dek)] });
    const clip = stubClipboard();
    renderVault(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-field="api_token"]') !== null);
    asButton(m.root.querySelector('[data-field="api_token"] [data-action="reveal"]'))?.click();
    expect(m.root.textContent?.includes(TOKEN)).toBe(true);
    asButton(m.root.querySelector('[data-field="api_token"] [data-action="copy"]'))?.click();
    await waitFor(() => clip.wrote === TOKEN);
    leaveVault(m.state);
    current = undefined;
    expect(m.root.children.length).toBe(0);
    expect(m.root.textContent?.includes(TOKEN)).toBe(false);
    expect(m.host.actions.children.length).toBe(0);
    await settled();
    expect(clip.wrote).toBe("");
  });

  test("a response after leave or logout gen bump is dropped", async () => {
    const m = start();
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      if (reqUrl(input) === "/api/v1/providers") {
        return json(200, { providers: SCHEMAS });
      }
      return new Promise<Response>((resolve) => {
        finish = resolve;
      });
    }) as unknown as typeof fetch;
    renderVault(m.state, m.root, m.host);
    expect(m.root.textContent?.includes(LOADING_SENTENCE)).toBe(true);
    bumpLogoutGen();
    finish?.(
      json(200, {
        entries: [
          {
            name: CF,
            ciphertext: "aa",
            meta: { provider: "cloudflare", fields: ["api_token"] },
            version: 1,
            updated: UPDATED,
          },
        ],
      }),
    );
    await settled();
    await settled();
    expect(m.root.querySelector(`[data-name="${CF}"]`)).toBeNull();
    expect(m.state.counts.get().vault).toBeUndefined();
  });
});
