import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { FAIL_SENTENCE, providerDeletePath, providersUrl, vaultUrl } from "../lib/api.ts";
import { asButton, asInput } from "../lib/dom.ts";
import { bumpLogoutGen } from "../lib/gen.ts";
import type { AppState, Host } from "../lib/host.ts";
import { signal } from "../lib/signal.ts";
import {
  BAD_NAME_SENTENCE,
  EMPTY_SENTENCE,
  FIELDS_SENTENCE,
  LEDE,
  LOAD_FAIL_SENTENCE,
  LOADING_SENTENCE,
  NAME_FIRST_SENTENCE,
  NEW_PROVIDER_LABEL,
  PUT_ROUTE,
  SAVED_TOAST,
  TITLE_SENTENCE,
  WIZARD_SUB,
  countUsage,
  deletedToast,
  envJoin,
  failSentence,
  filledFields,
  leaveProviders,
  parseProviders,
  parseVaultProviderNames,
  providerNameOk,
  putProviderBody,
  renderProviders,
  sourceLabel,
  usageLabel,
  type ProviderInfo,
} from "./providers.ts";

const CIPHER = "deadbeef-ciphertext-must-not-paint";
const GH: ProviderInfo = {
  name: "github",
  title: "GitHub",
  builtin: true,
  fields: [
    { key: "token", secret: true, optional: false, env: "GITHUB_TOKEN" },
    { key: "user", secret: false, optional: true, env: "GITHUB_USER" },
  ],
};
const ACME: ProviderInfo = {
  name: "acme",
  title: "Acme",
  builtin: false,
  fields: [{ key: "token", secret: true, optional: false, env: "ACME_TOKEN" }],
};

type Call = { method: string; url: string; body?: unknown };

type Api = {
  calls: Call[];
  puts: unknown[];
  providers: ProviderInfo[];
  vault: { entries: unknown[] };
  providersStatus: number;
  vaultStatus: number;
  putStatus: number;
  deleteStatus: number;
  putError?: string;
  deleteError?: string;
};

const origFetch = globalThis.fetch;

function json(status: number, data: unknown): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function reqUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
}

function mockApi(extra: Partial<Api> = {}): Api {
  const api: Api = {
    calls: [],
    puts: [],
    providers: [GH, ACME],
    vault: {
      entries: [
        { name: "ci/github", ciphertext: CIPHER, meta: { provider: "github" } },
        { name: "prod/github", ciphertext: CIPHER, meta: { provider: "github" } },
        { name: "prod/acme", ciphertext: CIPHER, meta: { provider: "acme" } },
      ],
    },
    providersStatus: 200,
    vaultStatus: 200,
    putStatus: 200,
    deleteStatus: 200,
    ...extra,
  };
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
    api.calls.push({ method, url, body });
    if (method === "GET" && url === providersUrl()) {
      return json(api.providersStatus, { providers: api.providers });
    }
    if (method === "GET" && url === vaultUrl()) {
      return json(api.vaultStatus, api.vault);
    }
    if (method === "PUT" && url === providersUrl()) {
      api.puts.push(body);
      if (api.putStatus === 200 && body !== undefined && typeof body === "object" && body !== null) {
        const rec = body as { name?: string; title?: string; fields?: ProviderInfo["fields"] };
        if (typeof rec.name === "string") {
          api.providers = [
            ...api.providers.filter((p) => p.name !== rec.name),
            {
              name: rec.name,
              title: typeof rec.title === "string" ? rec.title : rec.name,
              builtin: false,
              fields: Array.isArray(rec.fields) ? rec.fields : [],
            },
          ];
        }
      }
      return json(
        api.putStatus,
        api.putStatus === 200 ? { ok: true } : { error: api.putError ?? "schema" },
      );
    }
    if (method === "DELETE" && url.startsWith("/api/v1/providers/")) {
      const name = decodeURIComponent(url.slice("/api/v1/providers/".length));
      if (api.deleteStatus === 200) {
        api.providers = api.providers.filter((p) => p.name !== name);
      }
      return json(
        api.deleteStatus,
        api.deleteStatus === 200 ? { ok: true } : { error: api.deleteError ?? "builtin" },
      );
    }
    return json(404, {});
  }) as unknown as typeof fetch;
  return api;
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

function appState(): AppState {
  return {
    path: signal("/providers"),
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
    counts: signal({ vault: 3 }),
    toast: signal(""),
  };
}

type Mount = {
  state: AppState;
  root: HTMLElement;
  host: Host;
  flashes: string[];
  signedOut: { n: number };
};

function mount(): Mount {
  const state = appState();
  const actions = document.createElement("div");
  const root = document.createElement("div");
  const flashes: string[] = [];
  const signedOut = { n: 0 };
  const host: Host = {
    navigate: () => {},
    redraw: () => {},
    flash: (message) => {
      flashes.push(message);
    },
    signOut: async () => {
      signedOut.n += 1;
    },
    loadSession: async () => {},
    actions,
  };
  document.body.append(root, actions);
  return { state, root, host, flashes, signedOut };
}

let current: Mount | undefined;

beforeEach(() => {
  current = undefined;
});

afterEach(() => {
  if (current) {
    leaveProviders(current.state);
  }
  current = undefined;
  document.body.replaceChildren();
  globalThis.fetch = origFetch;
});

function start(): Mount {
  const m = mount();
  current = m;
  return m;
}

function fill(input: HTMLInputElement | null, value: string): void {
  if (!input) {
    return;
  }
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

describe("providers helpers", () => {
  test("usageLabel is em dash, 1 secret, or N secrets", () => {
    expect(usageLabel(0)).toBe("—");
    expect(usageLabel(-1)).toBe("—");
    expect(usageLabel(1)).toBe("1 secret");
    expect(usageLabel(2)).toBe("2 secrets");
  });

  test("envJoin uses two spaces and drops empty env keys", () => {
    expect(envJoin(GH.fields)).toBe("GITHUB_TOKEN  GITHUB_USER");
    expect(envJoin([{ env: "A" }, { env: "" }, { env: "B" }])).toBe("A  B");
    expect(envJoin([])).toBe("");
  });

  test("sourceLabel and deletedToast", () => {
    expect(sourceLabel(true)).toBe("built-in");
    expect(sourceLabel(false)).toBe("custom");
    expect(deletedToast("acme")).toBe("Deleted acme");
  });

  test("providerNameOk uses checkName and refuses a slash", () => {
    expect(providerNameOk("acme")).toBe(true);
    expect(providerNameOk("acme-ci")).toBe(true);
    expect(providerNameOk("")).toBe(false);
    expect(providerNameOk("a/b")).toBe(false);
    expect(providerNameOk("has space")).toBe(false);
  });

  test("parseProviders keeps the first named row and reads builtin/fields", () => {
    const rows = parseProviders({
      providers: [
        {
          name: "forgejo",
          title: "Forgejo",
          builtin: false,
          fields: [{ key: "token", secret: true, env: "FORGEJO_TOKEN" }],
        },
        { name: "forgejo", title: "dup" },
        { title: "no-name" },
        { name: "gitea", title: "Gitea", builtin: true, fields: [{ key: "token", env: "GITEA_TOKEN" }] },
      ],
    });
    expect(rows).toEqual([
      {
        name: "forgejo",
        title: "Forgejo",
        builtin: false,
        fields: [{ key: "token", secret: true, optional: false, env: "FORGEJO_TOKEN" }],
      },
      {
        name: "gitea",
        title: "Gitea",
        builtin: true,
        fields: [{ key: "token", secret: false, optional: false, env: "GITEA_TOKEN" }],
      },
    ]);
    expect(parseProviders({})).toEqual([]);
  });

  test("parseVaultProviderNames reads meta.provider and ignores ciphertext", () => {
    const names = parseVaultProviderNames({
      entries: [
        { name: "a", ciphertext: CIPHER, meta: { provider: "github" } },
        { name: "b", meta: { provider: "github" } },
        { name: "c", meta: { provider: "gitea" } },
        { name: "d", meta: {} },
        { ciphertext: CIPHER },
      ],
    });
    expect(names).toEqual(["github", "github", "gitea"]);
    expect(names.join(" ").includes(CIPHER)).toBe(false);
    expect(countUsage(names).get("github")).toBe(2);
    expect(countUsage(names).get("gitea")).toBe(1);
    expect(parseVaultProviderNames({})).toEqual([]);
  });

  test("filledFields and putProviderBody drop empty rows and never a value", () => {
    const fields = filledFields([
      { key: " token ", env: " ACME_TOKEN ", secret: true, optional: false },
      { key: "", env: "SKIP", secret: false, optional: false },
      { key: "url", env: "", secret: false, optional: true },
    ]);
    expect(fields).toEqual([{ key: "token", env: "ACME_TOKEN", secret: true, optional: false }]);
    const body = putProviderBody("acme", "Acme", [
      { key: "token", env: "ACME_TOKEN", secret: true, optional: false },
    ]);
    expect(body).toEqual({
      name: "acme",
      title: "Acme",
      fields: [{ key: "token", env: "ACME_TOKEN", secret: true, optional: false }],
    });
    expect(JSON.stringify(body).includes("value")).toBe(false);
    expect(putProviderBody("a/b", "Acme", fields)).toBeUndefined();
    expect(putProviderBody("acme", "", fields)).toBeUndefined();
    expect(putProviderBody("acme", "Acme", [])).toBeUndefined();
  });

  test("failSentence prefers the server error field", () => {
    expect(failSentence({ error: "builtin" })).toBe("builtin");
    expect(failSentence({})).toBe(FAIL_SENTENCE);
  });
});

describe("providers screen", () => {
  test("loading then empty", async () => {
    const m = start();
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = reqUrl(input);
      if (url === providersUrl()) {
        return new Promise<Response>((resolve) => {
          finish = resolve;
        });
      }
      if (url === vaultUrl()) {
        return json(200, { entries: [] });
      }
      return json(404, {});
    }) as unknown as typeof fetch;
    renderProviders(m.state, m.root, m.host);
    expect(m.root.textContent?.includes(LOADING_SENTENCE)).toBe(true);
    finish?.(json(200, { providers: [] }));
    await waitFor(() => m.root.textContent?.includes(EMPTY_SENTENCE) === true);
    expect(m.state.counts.get().providers).toBe(0);
  });

  test("list paints built-in and custom rows, usage, and copy", async () => {
    const m = start();
    const api = mockApi();
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-provider="github"]') !== null);
    expect(api.calls.some((c) => c.method === "GET" && c.url === providersUrl())).toBe(true);
    expect(api.calls.some((c) => c.method === "GET" && c.url === vaultUrl())).toBe(true);
    expect(m.root.querySelector('.page[data-width="1100"]')).not.toBeNull();
    expect(m.root.querySelector(".page-lede")?.textContent).toBe(LEDE);
    expect(asButton(m.root.querySelector('[data-action="new"]'))?.className).toBe(
      "btn btn-primary spacer",
    );
    expect(m.root.querySelector('[data-action="new"]')?.textContent).toBe(NEW_PROVIDER_LABEL);
    const head = m.root.querySelector(".grid.grid-head.cols-providers");
    expect(head?.textContent).toBe("ProviderSourceEnvironmentIn use");
    const gh = m.root.querySelector('[data-provider="github"]');
    expect(gh?.className).toBe("grid cols-providers");
    expect(gh?.getAttribute("data-builtin")).toBe("true");
    expect(gh?.querySelector("[data-title]")?.textContent).toBe("GitHub");
    expect(gh?.querySelector("[data-name]")?.textContent).toBe("github");
    expect(gh?.querySelector("[data-name]")?.className).toContain("cell-mono-xs");
    expect(gh?.querySelector(".badge.badge-sm")?.textContent).toBe("built-in");
    expect(gh?.querySelector("[data-env]")?.textContent).toBe("GITHUB_TOKEN  GITHUB_USER");
    expect(gh?.querySelector("[data-usage]")?.textContent).toBe("2 secrets");
    expect(gh?.querySelector('[data-action="delete"]')).toBeNull();
    const acme = m.root.querySelector('[data-provider="acme"]');
    expect(acme?.getAttribute("data-builtin")).toBe("false");
    expect(acme?.querySelector(".badge.badge-sm")?.textContent).toBe("custom");
    expect(acme?.querySelector("[data-usage]")?.textContent).toBe("1 secret");
    expect(asButton(acme?.querySelector('[data-action="delete"]') ?? null)?.className).toBe(
      "btn btn-sm btn-danger",
    );
    expect(m.root.textContent?.includes(CIPHER)).toBe(false);
    expect(m.state.counts.get()).toEqual({ vault: 3, providers: 2 });
  });

  test("unused provider shows an em dash", async () => {
    const m = start();
    mockApi({
      vault: { entries: [{ name: "ci/github", ciphertext: CIPHER, meta: { provider: "github" } }] },
    });
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-provider="acme"]') !== null);
    expect(m.root.querySelector('[data-provider="acme"] [data-usage]')?.textContent).toBe("—");
    expect(m.root.querySelector('[data-provider="github"] [data-usage]')?.textContent).toBe(
      "1 secret",
    );
  });

  test("GET failure is Providers did not load", async () => {
    const m = start();
    mockApi({ providersStatus: 500 });
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.textContent?.includes(LOAD_FAIL_SENTENCE) === true);
    expect(m.root.querySelector('[role="alert"]')?.textContent).toBe(LOAD_FAIL_SENTENCE);
    expect(m.root.querySelector(".alert-danger")).not.toBeNull();
    expect(m.root.textContent?.includes(EMPTY_SENTENCE)).toBe(false);
    expect(m.signedOut.n).toBe(0);
  });

  test("401 on GET /api/v1/providers signs out", async () => {
    const m = start();
    mockApi({ providersStatus: 401 });
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.signedOut.n === 1);
    expect(m.root.textContent?.includes(EMPTY_SENTENCE)).toBe(false);
  });

  test("403 on GET /api/v1/vault signs out", async () => {
    const m = start();
    mockApi({ vaultStatus: 403 });
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.signedOut.n === 1);
  });

  test("New provider overlay copy, inert page, focus, Escape and Cancel", async () => {
    const m = start();
    mockApi({ vault: { entries: [] } });
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-provider="github"]') !== null);
    asButton(m.root.querySelector('[data-action="new"]'))?.click();
    const overlay = m.root.querySelector(".overlay");
    expect(overlay?.className).toBe("overlay");
    expect(overlay?.querySelector(".modal")).not.toBeNull();
    expect(overlay?.getAttribute("role")).toBe("dialog");
    expect(overlay?.getAttribute("aria-modal")).toBe("true");
    expect(m.root.querySelector("#new-provider-heading")?.textContent).toBe(NEW_PROVIDER_LABEL);
    expect(m.root.querySelector(".modal-sub")?.textContent).toBe(WIZARD_SUB);
    expect(m.root.querySelector(".modal-route")?.textContent).toBe(PUT_ROUTE);
    expect(asInput(m.root.querySelector("#new-provider-name"))?.className).toBe("input input-mono");
    expect(document.activeElement).toBe(m.root.querySelector("#new-provider-name"));
    expect(m.root.querySelector(".page")?.hasAttribute("inert")).toBe(true);
    expect(m.root.querySelectorAll("[data-field]").length).toBe(1);
    overlay?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    expect(m.root.querySelector(".overlay")).toBeNull();
    asButton(m.root.querySelector('[data-action="new"]'))?.click();
    asButton(m.root.querySelector('[data-action="cancel"]'))?.click();
    expect(m.root.querySelector(".overlay")).toBeNull();
  });

  test("wizard validation sentences", async () => {
    const m = start();
    const api = mockApi();
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-action="new"]') !== null);
    asButton(m.root.querySelector('[data-action="new"]'))?.click();
    asButton(m.root.querySelector('[data-action="save"]'))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(NAME_FIRST_SENTENCE);
    fill(asInput(m.root.querySelector("#new-provider-name")), "has space");
    asButton(m.root.querySelector('[data-action="save"]'))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(BAD_NAME_SENTENCE);
    fill(asInput(m.root.querySelector("#new-provider-name")), "acme/prod");
    asButton(m.root.querySelector('[data-action="save"]'))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(BAD_NAME_SENTENCE);
    fill(asInput(m.root.querySelector("#new-provider-name")), "forgejo");
    asButton(m.root.querySelector('[data-action="save"]'))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(TITLE_SENTENCE);
    fill(asInput(m.root.querySelector("#new-provider-title")), "Forgejo");
    asButton(m.root.querySelector('[data-action="save"]'))?.click();
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe(FIELDS_SENTENCE);
    expect(api.puts).toEqual([]);
  });

  test("New provider PUT body is name/title/fields and never a secret value", async () => {
    const m = start();
    const api = mockApi();
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-action="new"]') !== null);
    asButton(m.root.querySelector('[data-action="new"]'))?.click();
    fill(asInput(m.root.querySelector("#new-provider-name")), "forgejo");
    fill(asInput(m.root.querySelector("#new-provider-title")), "Forgejo");
    asButton(m.root.querySelector('[data-action="add-field"]'))?.click();
    expect(m.root.querySelectorAll("[data-field]").length).toBe(2);
    fill(asInput(m.root.querySelector('[data-field="0"] #field-0-key')), "token");
    fill(asInput(m.root.querySelector('[data-field="0"] #field-0-env')), "FORGEJO_TOKEN");
    const secret = m.root.querySelector("#field-0-secret") as HTMLInputElement;
    secret.checked = true;
    secret.dispatchEvent(new Event("change", { bubbles: true }));
    fill(asInput(m.root.querySelector('[data-field="1"] #field-1-key')), "url");
    fill(asInput(m.root.querySelector('[data-field="1"] #field-1-env')), "FORGEJO_URL");
    asButton(m.root.querySelector('[data-action="save"]'))?.click();
    await waitFor(() => m.flashes.includes(SAVED_TOAST));
    expect(api.puts).toHaveLength(1);
    const body = api.puts[0] as Record<string, unknown>;
    expect(body).toEqual({
      name: "forgejo",
      title: "Forgejo",
      fields: [
        { key: "token", env: "FORGEJO_TOKEN", secret: true, optional: false },
        { key: "url", env: "FORGEJO_URL", secret: false, optional: false },
      ],
    });
    const dumped = JSON.stringify(body);
    expect(dumped.includes("value")).toBe(false);
    expect(dumped.includes("plaintext")).toBe(false);
    expect(dumped.includes("dek")).toBe(false);
    expect(m.root.querySelector(".overlay")).toBeNull();
    await waitFor(() => m.root.querySelector('[data-provider="forgejo"]') !== null);
    expect(m.root.querySelector('[data-provider="forgejo"] [data-title]')?.textContent).toBe(
      "Forgejo",
    );
    expect(m.state.counts.get().providers).toBe(3);
    const put = api.calls.find((c) => c.method === "PUT" && c.url === providersUrl());
    expect(put?.body).toEqual(body);
  });

  test("delete custom DELETEs the encoded path and reloads", async () => {
    const m = start();
    const api = mockApi();
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-provider="acme"]') !== null);
    asButton(m.root.querySelector('[data-provider="acme"] [data-action="delete"]'))?.click();
    await waitFor(() => m.flashes.includes(deletedToast("acme")));
    expect(api.calls.some((c) => c.method === "DELETE" && c.url === providerDeletePath("acme"))).toBe(
      true,
    );
    await waitFor(() => m.root.querySelector('[data-provider="acme"]') === null);
    expect(m.root.querySelector('[data-provider="github"]')).not.toBeNull();
    expect(m.state.counts.get().providers).toBe(1);
  });

  test("refuse builtin delete in the UI", async () => {
    const m = start();
    const api = mockApi();
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-provider="github"]') !== null);
    expect(m.root.querySelector('[data-provider="github"] [data-action="delete"]')).toBeNull();
    expect(api.calls.some((c) => c.method === "DELETE")).toBe(false);
  });

  test("400 builtin on delete paints the server error", async () => {
    const m = start();
    mockApi({ deleteStatus: 400, deleteError: "builtin" });
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-provider="acme"]') !== null);
    asButton(m.root.querySelector('[data-provider="acme"] [data-action="delete"]'))?.click();
    await waitFor(() => m.root.querySelector('[role="alert"]')?.textContent === "builtin");
    expect(m.root.querySelector(".alert-danger")?.textContent).toBe("builtin");
    expect(m.root.querySelector('[data-provider="acme"]')).not.toBeNull();
    expect(m.flashes).toEqual([]);
    expect(m.signedOut.n).toBe(0);
  });

  test("401 on PUT signs out", async () => {
    const m = start();
    mockApi({ putStatus: 401 });
    renderProviders(m.state, m.root, m.host);
    await waitFor(() => m.root.querySelector('[data-action="new"]') !== null);
    asButton(m.root.querySelector('[data-action="new"]'))?.click();
    fill(asInput(m.root.querySelector("#new-provider-name")), "forgejo");
    fill(asInput(m.root.querySelector("#new-provider-title")), "Forgejo");
    fill(asInput(m.root.querySelector("#field-0-key")), "token");
    fill(asInput(m.root.querySelector("#field-0-env")), "FORGEJO_TOKEN");
    asButton(m.root.querySelector('[data-action="save"]'))?.click();
    await waitFor(() => m.signedOut.n === 1);
  });

  test("leave() cleanup drops a pending response", async () => {
    const m = start();
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = reqUrl(input);
      if (url === providersUrl()) {
        return new Promise<Response>((resolve) => {
          finish = resolve;
        });
      }
      if (url === vaultUrl()) {
        return json(200, { entries: [] });
      }
      return json(404, {});
    }) as unknown as typeof fetch;
    renderProviders(m.state, m.root, m.host);
    expect(m.root.textContent?.includes(LOADING_SENTENCE)).toBe(true);
    leaveProviders(m.state);
    current = undefined;
    expect(m.root.children.length).toBe(0);
    expect(m.host.actions.children.length).toBe(0);
    finish?.(json(200, { providers: [GH] }));
    await settled();
    expect(m.root.textContent?.includes("GitHub")).toBe(false);
  });

  test("a response after logout gen bump is dropped", async () => {
    const m = start();
    let finish: ((value: Response) => void) | undefined;
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = reqUrl(input);
      if (url === providersUrl()) {
        return new Promise<Response>((resolve) => {
          finish = resolve;
        });
      }
      if (url === vaultUrl()) {
        return json(200, { entries: [] });
      }
      return json(404, {});
    }) as unknown as typeof fetch;
    renderProviders(m.state, m.root, m.host);
    bumpLogoutGen();
    finish?.(json(200, { providers: [GH] }));
    await settled();
    expect(m.root.textContent?.includes("GitHub")).toBe(false);
    expect(m.signedOut.n).toBe(0);
  });
});
