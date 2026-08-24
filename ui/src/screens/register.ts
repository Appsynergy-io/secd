/** Vault register: list|inspector at ≥900, list→sheet below. Copy is the default action. */

import {
  BREAKPOINT_PX,
  FAIL_SENTENCE,
  NO_DEK_SENTENCE,
  errorMessage,
  layoutMode,
  req,
  vaultRollbackUrl,
  vaultUrl,
  vaultVersionsUrl,
  type LayoutMode,
} from "../lib/api.ts";
import {
  checkName,
  fromHex,
  getDek,
  open,
  seal,
  toHex,
  zeroizeBytes,
} from "../lib/crypto.ts";
import { PROVIDERS } from "../lib/providers.gen.ts";
import { buildPayload, providerByName } from "../lib/providers.ts";

export const MASK = "••••••••";
export const CLIP_FAIL_SENTENCE =
  "The browser refused the clipboard. Select the value and copy it.";
export const CLIP_MISSING_SENTENCE =
  "This browser has no clipboard. Select the value and copy it.";
export const LOADING_SENTENCE = "Loading secrets.";
export const EMPTY_SENTENCE = "No secrets yet.";
export const LOAD_FAIL_SENTENCE = "Secrets did not load.";
export const OPEN_FAIL_SENTENCE = "This secret could not be opened.";
export const FIELD_FAIL_SENTENCE = "This field could not be opened.";
export const NAME_FIRST_SENTENCE = "Name the secret first.";
export const REQUIRED_SENTENCE = "Fill every required field.";
export const BAD_NAME_SENTENCE = "That name is not allowed.";
export const VERSIONS_FAIL_SENTENCE = "Versions did not load.";

export type RegisterHost = {
  path: { get(): string };
  error: { get(): string | undefined; set(v: string | undefined): void };
  pending: { get(): boolean; set(v: boolean): void };
};

type VaultEntry = {
  name: string;
  ciphertext: unknown;
  meta: Record<string, unknown>;
  fieldKeys: string[];
};

type Opened = {
  fields: Record<string, string>;
  error?: string;
};

type VersionInfo = { version: number; created: string };

type Store = {
  gen: number;
  status: "idle" | "loading" | "ready" | "error";
  loadError?: string;
  entries: VaultEntry[];
  opened: Map<string, Opened>;
  selected?: string;
  filter: string;
  wizard: boolean;
  wizardProvider: string;
  wizardName: string;
  wizardValues: Map<string, string>;
  wizardError?: string;
  versions: VersionInfo[];
  versionsStatus: "idle" | "loading" | "ready" | "error";
  revealed?: { name: string; key: string };
  copyState?: { name: string; key: string; label: "Copy" | "Copied" };
  copyFail?: { name: string; key: string; reason: string };
  saving: boolean;
  rolling: boolean;
};

const stores = new WeakMap<object, Store>();

function freshStore(): Store {
  return {
    gen: 0,
    status: "idle",
    entries: [],
    opened: new Map(),
    filter: "",
    wizard: false,
    wizardProvider: PROVIDERS[0]?.name ?? "",
    wizardName: "",
    wizardValues: new Map(),
    versions: [],
    versionsStatus: "idle",
    saving: false,
    rolling: false,
  };
}

function storeOf(host: object): Store {
  let s = stores.get(host);
  if (!s) {
    s = freshStore();
    stores.set(host, s);
  }
  return s;
}

export function abandonRegister(host: object): void {
  const s = stores.get(host);
  if (!s) {
    return;
  }
  s.gen += 1;
  s.opened.clear();
  stores.delete(host);
}

export function versionStamp(created: string): string {
  return created.slice(0, 16).replace("T", " ");
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | boolean | undefined> = {},
  children: Array<Node | string> = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === false) {
      continue;
    }
    if (v === true) {
      node.setAttribute(k, "");
    } else {
      node.setAttribute(k, v);
    }
  }
  if (tag === "input" || tag === "select" || tag === "textarea") {
    if (typeof attrs.value === "string") {
      (node as HTMLInputElement).value = attrs.value;
    }
  }
  for (const child of children) {
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

function currentLayout(): LayoutMode {
  const w = globalThis.innerWidth;
  return layoutMode(
    typeof w === "number" && Number.isFinite(w) && w > 0 ? w : BREAKPOINT_PX,
  );
}

function nav(): HTMLElement {
  const items: Array<[string, string, boolean]> = [
    ["/register", "Register", true],
    ["/activity", "Activity", false],
    ["/account", "Account", false],
  ];
  return el(
    "nav",
    { class: "nav", "aria-label": "Console" },
    items.map(([href, label, current]) =>
      el("a", { href, "aria-current": current ? "page" : undefined }, [label]),
    ),
  );
}

function stillHere(host: RegisterHost, gen: number, store: Store): boolean {
  return store.gen === gen && host.path.get() === "/register";
}

function parseEntry(v: unknown): VaultEntry | undefined {
  if (typeof v !== "object" || v === null) {
    return undefined;
  }
  const rec = v as Record<string, unknown>;
  const name = rec["name"];
  if (typeof name !== "string" || name === "") {
    return undefined;
  }
  const metaRaw = rec["meta"];
  const meta =
    typeof metaRaw === "object" && metaRaw !== null && !Array.isArray(metaRaw)
      ? { ...(metaRaw as Record<string, unknown>) }
      : {};
  let fieldKeys: string[] = [];
  const fields = meta["fields"];
  if (Array.isArray(fields)) {
    fieldKeys = fields.filter((x): x is string => typeof x === "string");
  }
  if (fieldKeys.length === 0) {
    const parts = name.split("/");
    const tail = parts[parts.length - 1];
    fieldKeys = [tail !== undefined && tail !== "" ? tail : name];
  }
  return {
    name,
    ciphertext: rec["ciphertext"],
    meta,
    fieldKeys,
  };
}

function parseVault(data: unknown): VaultEntry[] {
  if (typeof data !== "object" || data === null) {
    return [];
  }
  const rows = (data as { entries?: unknown }).entries;
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: VaultEntry[] = [];
  for (const row of rows) {
    const e = parseEntry(row);
    if (e && !out.some((x) => x.name === e.name)) {
      out.push(e);
    }
  }
  return out;
}

function parseVersions(data: unknown): VersionInfo[] {
  if (typeof data !== "object" || data === null) {
    return [];
  }
  const rows = (data as { versions?: unknown }).versions;
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: VersionInfo[] = [];
  for (const row of rows) {
    if (typeof row !== "object" || row === null) {
      continue;
    }
    const rec = row as Record<string, unknown>;
    const version = rec["version"];
    const created = rec["created"];
    if (typeof version !== "number" || !Number.isFinite(version)) {
      continue;
    }
    out.push({
      version,
      created: typeof created === "string" ? created : "",
    });
  }
  return out;
}

function openEntry(dek: Uint8Array, entry: VaultEntry): Opened {
  if (typeof entry.ciphertext !== "string") {
    return { fields: {}, error: OPEN_FAIL_SENTENCE };
  }
  let blob: Uint8Array;
  try {
    blob = fromHex(entry.ciphertext);
  } catch {
    return { fields: {}, error: OPEN_FAIL_SENTENCE };
  }
  let pt: Uint8Array;
  try {
    pt = open(dek, entry.name, blob);
  } catch {
    zeroizeBytes(blob);
    return { fields: {}, error: OPEN_FAIL_SENTENCE };
  }
  zeroizeBytes(blob);
  let text: string;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(pt);
  } catch {
    zeroizeBytes(pt);
    return { fields: {}, error: OPEN_FAIL_SENTENCE };
  }
  zeroizeBytes(pt);
  let parsed: unknown;
  try {
    parsed = JSON.parse(text) as unknown;
  } catch {
    return { fields: {}, error: OPEN_FAIL_SENTENCE };
  }
  const fields: Record<string, string> = {};
  if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
    const rec = parsed as Record<string, unknown>;
    for (const key of entry.fieldKeys) {
      const v = rec[key];
      if (typeof v === "string") {
        fields[key] = v;
      }
    }
  }
  return { fields };
}

function putEntries(rows: unknown[]): Array<Record<string, unknown>> {
  const out: Array<Record<string, unknown>> = [];
  for (const row of rows) {
    if (typeof row !== "object" || row === null) {
      continue;
    }
    const rec = row as Record<string, unknown>;
    if (typeof rec["name"] !== "string") {
      continue;
    }
    out.push({
      name: rec["name"],
      ciphertext: rec["ciphertext"],
      meta: rec["meta"] ?? {},
    });
  }
  return out;
}

export function renderRegister(state: RegisterHost, root: HTMLElement): void {
  const store = storeOf(state);
  paint(state, root);
  if (store.status === "idle") {
    void loadVault(state, root);
  }
}

function paint(state: RegisterHost, root: HTMLElement): void {
  const store = storeOf(state);
  const layout = currentLayout();
  const selectedName = store.selected;
  const selected =
    selectedName === undefined
      ? undefined
      : store.entries.find((e) => e.name === selectedName);
  const sheet = layout === "list-only" && selected !== undefined;
  const page = el("div", {
    class: "app",
    "data-page": "register",
    "data-layout": layout,
  });
  page.append(nav());
  const head = el("div", { class: "secd-row" });
  head.append(el("h1", {}, ["Register"]));
  const add = el("button", { type: "button", "data-action": "add" }, ["Add"]);
  add.addEventListener("click", () => {
    store.wizard = true;
    store.wizardProvider = PROVIDERS[0]?.name ?? "";
    store.wizardName = "";
    store.wizardValues = new Map();
    delete store.wizardError;
    paint(state, root);
  });
  head.append(add);
  page.append(head);
  page.append(el("p", {}, ["Secrets stored on this LAN. Copy is the default."]));
  const err = store.loadError ?? state.error.get();
  if (err) {
    page.append(el("p", { class: "error" }, [err]));
  }
  const workspace = el("div", { class: "workspace" });
  workspace.append(listPane(state, root, store));
  workspace.append(inspectorPane(state, root, store, selected));
  page.append(workspace);
  if (sheet && selected) {
    page.append(sheetPane(state, root, store, selected));
  }
  if (store.wizard) {
    page.append(wizardPane(state, root, store));
  }
  root.replaceChildren(page);
}

function listPane(state: RegisterHost, root: HTMLElement, store: Store): HTMLElement {
  const card = el("div", { class: "card", "data-pane": "list" });
  const items = store.entries.filter(
    (e) => store.filter === "" || e.name.includes(store.filter),
  );
  const list = el("div", { class: "list", "data-list": "secrets" });
  if (store.entries.length > 0) {
    const filter = el("input", {
      class: "mono",
      placeholder: "Filter names…",
      value: store.filter,
      autocomplete: "off",
    });
    filter.addEventListener("input", () => {
      store.filter = filter.value;
      for (const node of list.querySelectorAll("[data-name]")) {
        if (!(node instanceof HTMLElement)) {
          continue;
        }
        const name = node.getAttribute("data-name") ?? "";
        node.hidden = store.filter !== "" && !name.includes(store.filter);
      }
    });
    card.append(filter);
  }
  if (store.status === "loading" || store.status === "idle") {
    list.append(el("p", {}, [LOADING_SENTENCE]));
  } else if (store.status === "error") {
    list.append(el("p", {}, [LOAD_FAIL_SENTENCE]));
  } else if (store.entries.length === 0) {
    list.append(el("p", {}, [EMPTY_SENTENCE]));
  } else if (items.length === 0) {
    list.append(el("p", {}, ["No names match."]));
  } else {
    for (const item of items) {
      const btn = el(
        "button",
        {
          type: "button",
          class: "secondary mono",
          "data-name": item.name,
        },
        [item.name],
      );
      btn.addEventListener("click", () => {
        void selectName(state, root, item.name);
      });
      list.append(btn);
    }
  }
  card.append(list);
  return card;
}

function secretBody(
  state: RegisterHost,
  root: HTMLElement,
  store: Store,
  item: VaultEntry | undefined,
): HTMLElement[] {
  if (!item) {
    return [
      el("h2", { class: "mono" }, ["Secret"]),
      el("p", {}, ["Select a secret"]),
      el("p", {}, ["Choose a name from the list."]),
    ];
  }
  const nodes: HTMLElement[] = [el("h2", { class: "mono" }, [item.name])];
  if (getDek() === undefined) {
    nodes.push(el("p", { "data-reason": "" }, [NO_DEK_SENTENCE]));
  }
  const opened = store.opened.get(item.name);
  if (opened?.error) {
    nodes.push(el("p", { class: "error", "data-reason": "" }, [opened.error]));
  }
  for (const key of item.fieldKeys) {
    nodes.push(fieldRow(state, root, store, item, key, opened));
  }
  nodes.push(versionsBlock(state, root, store, item.name));
  return nodes;
}

function inspectorPane(
  state: RegisterHost,
  root: HTMLElement,
  store: Store,
  item: VaultEntry | undefined,
): HTMLElement {
  return el(
    "div",
    { class: "card", "data-pane": "inspector" },
    secretBody(state, root, store, item),
  );
}

function sheetPane(
  state: RegisterHost,
  root: HTMLElement,
  store: Store,
  item: VaultEntry,
): HTMLElement {
  const close = el(
    "button",
    { type: "button", class: "secondary", "data-action": "close" },
    ["Close"],
  );
  close.addEventListener("click", () => {
    delete store.selected;
    store.versions = [];
    store.versionsStatus = "idle";
    delete store.revealed;
    delete store.copyFail;
    paint(state, root);
  });
  return el("div", { class: "secd-overlay", "data-pane": "sheet", "data-sheet": "open" }, [
    el("div", { class: "secd-modal" }, [...secretBody(state, root, store, item), close]),
  ]);
}

function fieldRow(
  state: RegisterHost,
  root: HTMLElement,
  store: Store,
  item: VaultEntry,
  key: string,
  opened: Opened | undefined,
): HTMLElement {
  const value = opened?.fields[key];
  const holding = store.revealed?.name === item.name && store.revealed.key === key;
  const shown = holding && value !== undefined ? value : MASK;
  const copied =
    store.copyState?.name === item.name && store.copyState.key === key
      ? store.copyState.label
      : "Copy";
  const noDek = getDek() === undefined;
  const noValue = value === undefined;
  const disabled = noDek || noValue || store.saving;
  const valueEl = el("span", { class: "mono", "data-value": "" }, [shown]);
  const copy = el(
    "button",
    {
      type: "button",
      "data-action": "copy",
      disabled: disabled ? true : undefined,
    },
    [copied],
  );
  copy.addEventListener("click", () => {
    void onCopy(state, root, item.name, key);
  });
  const show = el(
    "button",
    {
      type: "button",
      class: "secondary",
      "data-action": "show",
      "data-hold": "1",
      disabled: disabled ? true : undefined,
    },
    ["Show"],
  );
  show.addEventListener("pointerdown", (ev) => {
    ev.preventDefault();
    if (value === undefined || disabled) {
      return;
    }
    store.revealed = { name: item.name, key };
    valueEl.textContent = value;
    const doc = show.ownerDocument;
    const hide = () => {
      if (store.revealed?.name === item.name && store.revealed.key === key) {
        delete store.revealed;
        valueEl.textContent = MASK;
      }
      doc.removeEventListener("pointerup", hide);
      doc.removeEventListener("pointercancel", hide);
    };
    doc.addEventListener("pointerup", hide);
    doc.addEventListener("pointercancel", hide);
  });
  const row = el("div", { class: "secd-stack", "data-field": key }, [
    el("p", {}, [key]),
    valueEl,
    el("div", { class: "secd-row" }, [copy, show]),
  ]);
  if (disabled && noValue && !noDek) {
    row.append(el("p", { "data-reason": "" }, [FIELD_FAIL_SENTENCE]));
  }
  const fail =
    store.copyFail?.name === item.name && store.copyFail.key === key
      ? store.copyFail
      : undefined;
  if (fail && value !== undefined) {
    row.append(el("p", { class: "error", "data-reason": "" }, [fail.reason]));
    const fallback = el("input", {
      class: "mono",
      readonly: true,
      "data-select-copy": "",
      value,
    });
    fallback.value = value;
    row.append(fallback);
  }
  return row;
}

function versionsBlock(
  state: RegisterHost,
  root: HTMLElement,
  store: Store,
  name: string,
): HTMLElement {
  const wrap = el("div", { class: "secd-stack", "data-list": "versions" });
  if (store.versionsStatus === "loading") {
    wrap.append(el("p", {}, ["Loading versions."]));
    return wrap;
  }
  if (store.versionsStatus === "error") {
    wrap.append(el("p", { class: "error" }, [VERSIONS_FAIL_SENTENCE]));
    return wrap;
  }
  if (store.versions.length <= 1) {
    return wrap;
  }
  wrap.append(el("p", {}, ["Versions"]));
  const latest = store.versions.reduce((m, v) => (v.version > m ? v.version : m), 0);
  const ordered = [...store.versions].sort((a, b) => b.version - a.version);
  for (const v of ordered) {
    const label = v.version === latest ? `v${v.version} · current` : `v${v.version}`;
    const row = el("div", { "data-version": String(v.version) }, [
      el("p", { class: "mono" }, [label]),
      el("p", { class: "mono" }, [versionStamp(v.created)]),
    ]);
    if (v.version !== latest) {
      const rb = el(
        "button",
        {
          type: "button",
          class: "secondary",
          "data-action": "rollback",
          disabled: store.rolling ? true : undefined,
        },
        ["Roll back"],
      );
      rb.addEventListener("click", () => {
        void onRollback(state, root, name, v.version);
      });
      row.append(rb);
      if (store.rolling) {
        row.append(el("p", { "data-reason": "" }, ["Rolling back."]));
      }
    }
    wrap.append(row);
  }
  return wrap;
}

function wizardPane(state: RegisterHost, root: HTMLElement, store: Store): HTMLElement {
  const first = PROVIDERS[0];
  const schema = providerByName(store.wizardProvider) ?? first;
  const noDek = getDek() === undefined;
  const form = el("div", { class: "secd-stack" });
  form.append(el("h2", {}, ["Add a secret"]));
  form.append(el("p", {}, ["Name a secret and fill the provider fields."]));
  const sel = el("select", { id: "provider" });
  for (const p of PROVIDERS) {
    const opt = el("option", { value: p.name }, [p.title]);
    if (p.name === store.wizardProvider) {
      opt.selected = true;
    }
    sel.append(opt);
  }
  sel.value = store.wizardProvider;
  sel.addEventListener("change", () => {
    store.wizardProvider = sel.value;
    store.wizardValues = new Map();
    delete store.wizardError;
    paint(state, root);
  });
  form.append(el("label", { for: "provider" }, ["Provider"]), sel);
  const name = el("input", {
    id: "secret_name",
    class: "mono",
    autocomplete: "off",
    placeholder: "kv/service/credential",
    value: store.wizardName,
  });
  name.addEventListener("input", () => {
    store.wizardName = name.value;
  });
  form.append(el("label", { for: "secret_name" }, ["Name"]), name);
  if (schema) {
    for (const f of schema.fields) {
      const label = f.optional ? `${f.key} (optional)` : f.key;
      const input = el("input", {
        class: "mono",
        type: f.secret ? "password" : "text",
        autocomplete: "off",
        "data-wizard-field": f.key,
        value: store.wizardValues.get(f.key) ?? "",
      });
      input.addEventListener("input", () => {
        store.wizardValues.set(f.key, input.value);
      });
      form.append(
        el("div", { "data-wizard-field": f.key }, [el("label", {}, [label]), input]),
      );
    }
  }
  if (store.wizardError) {
    form.append(el("p", { class: "error" }, [store.wizardError]));
  }
  if (noDek) {
    form.append(el("p", { "data-reason": "" }, [NO_DEK_SENTENCE]));
  }
  const save = el(
    "button",
    {
      type: "button",
      "data-action": "save",
      disabled: store.saving || noDek ? true : undefined,
    },
    ["Save"],
  );
  save.addEventListener("click", () => {
    void onSave(state, root);
  });
  const cancel = el(
    "button",
    { type: "button", class: "secondary", "data-action": "cancel" },
    ["Cancel"],
  );
  cancel.addEventListener("click", () => {
    store.wizard = false;
    delete store.wizardError;
    paint(state, root);
  });
  form.append(el("div", { class: "secd-row" }, [save, cancel]));
  return el("div", { class: "secd-overlay", "data-wizard": "open" }, [
    el("div", { class: "secd-modal" }, [form]),
  ]);
}

async function loadVault(state: RegisterHost, root: HTMLElement): Promise<void> {
  const store = storeOf(state);
  const gen = store.gen;
  store.status = "loading";
  delete store.loadError;
  paint(state, root);
  try {
    const res = await req("GET", vaultUrl());
    if (!stillHere(state, gen, store)) {
      return;
    }
    if (res.status !== 200) {
      store.status = "error";
      store.loadError = errorMessage(res.data) ?? FAIL_SENTENCE;
      paint(state, root);
      return;
    }
    store.entries = parseVault(res.data);
    store.opened.clear();
    const dek = getDek();
    if (dek) {
      for (const e of store.entries) {
        store.opened.set(e.name, openEntry(dek, e));
      }
    }
    store.status = "ready";
    delete store.loadError;
    paint(state, root);
  } catch {
    if (!stillHere(state, gen, store)) {
      return;
    }
    store.status = "error";
    store.loadError = FAIL_SENTENCE;
    paint(state, root);
  }
}

async function selectName(
  state: RegisterHost,
  root: HTMLElement,
  name: string,
): Promise<void> {
  const store = storeOf(state);
  store.selected = name;
  delete store.revealed;
  delete store.copyFail;
  store.versions = [];
  store.versionsStatus = "loading";
  paint(state, root);
  const gen = store.gen;
  try {
    const res = await req("GET", vaultVersionsUrl(), undefined, name);
    if (!stillHere(state, gen, store) || store.selected !== name) {
      return;
    }
    if (res.status !== 200) {
      store.versionsStatus = "error";
      paint(state, root);
      return;
    }
    store.versions = parseVersions(res.data);
    store.versionsStatus = "ready";
    paint(state, root);
  } catch {
    if (!stillHere(state, gen, store) || store.selected !== name) {
      return;
    }
    store.versionsStatus = "error";
    paint(state, root);
  }
}

async function onCopy(
  state: RegisterHost,
  root: HTMLElement,
  name: string,
  key: string,
): Promise<void> {
  const store = storeOf(state);
  const value = store.opened.get(name)?.fields[key];
  if (value === undefined || store.saving) {
    return;
  }
  delete store.copyFail;
  store.copyState = { name, key, label: "Copy" };
  const clip = globalThis.navigator?.clipboard;
  if (clip === undefined || typeof clip.writeText !== "function") {
    store.copyFail = { name, key, reason: CLIP_MISSING_SENTENCE };
    paint(state, root);
    return;
  }
  paint(state, root);
  const gen = store.gen;
  try {
    await clip.writeText(value);
    if (!stillHere(state, gen, store)) {
      return;
    }
    store.copyState = { name, key, label: "Copied" };
    paint(state, root);
  } catch {
    if (!stillHere(state, gen, store)) {
      return;
    }
    store.copyFail = { name, key, reason: CLIP_FAIL_SENTENCE };
    store.copyState = { name, key, label: "Copy" };
    paint(state, root);
  }
}

async function onSave(state: RegisterHost, root: HTMLElement): Promise<void> {
  const store = storeOf(state);
  if (store.saving) {
    return;
  }
  const n = store.wizardName.trim();
  if (n === "") {
    store.wizardError = NAME_FIRST_SENTENCE;
    paint(state, root);
    return;
  }
  if (!checkName(n)) {
    store.wizardError = BAD_NAME_SENTENCE;
    paint(state, root);
    return;
  }
  const values = [...store.wizardValues.entries()];
  const payload = buildPayload(store.wizardProvider, values);
  if (payload === undefined) {
    store.wizardError = REQUIRED_SENTENCE;
    paint(state, root);
    return;
  }
  const dek = getDek();
  if (dek === undefined) {
    store.wizardError = NO_DEK_SENTENCE;
    paint(state, root);
    return;
  }
  store.saving = true;
  delete store.wizardError;
  paint(state, root);
  const gen = store.gen;
  try {
    const sealed = toHex(seal(dek, n, new TextEncoder().encode(JSON.stringify(payload))));
    const current = await req("GET", vaultUrl());
    if (!stillHere(state, gen, store)) {
      return;
    }
    if (current.status !== 200) {
      store.wizardError = errorMessage(current.data) ?? FAIL_SENTENCE;
      return;
    }
    const raw =
      typeof current.data === "object" && current.data !== null
        ? (current.data as { entries?: unknown }).entries
        : undefined;
    const rows = Array.isArray(raw)
      ? putEntries(
          raw.filter((e) => {
            if (typeof e !== "object" || e === null) {
              return true;
            }
            return (e as { name?: unknown }).name !== n;
          }),
        )
      : [];
    rows.push({
      name: n,
      ciphertext: sealed,
      meta: { provider: store.wizardProvider, fields: Object.keys(payload) },
    });
    const put = await req("PUT", vaultUrl(), { entries: rows });
    if (!stillHere(state, gen, store)) {
      return;
    }
    if (put.status !== 200) {
      store.wizardError = errorMessage(put.data) ?? FAIL_SENTENCE;
      return;
    }
    store.wizard = false;
    delete store.selected;
    store.status = "idle";
    await loadVault(state, root);
  } catch {
    if (!stillHere(state, gen, store)) {
      return;
    }
    store.wizardError = FAIL_SENTENCE;
    paint(state, root);
  } finally {
    if (stillHere(state, gen, store)) {
      store.saving = false;
      if (store.wizard) {
        paint(state, root);
      }
    }
  }
}

async function onRollback(
  state: RegisterHost,
  root: HTMLElement,
  name: string,
  version: number,
): Promise<void> {
  const store = storeOf(state);
  if (store.rolling) {
    return;
  }
  store.rolling = true;
  paint(state, root);
  const gen = store.gen;
  try {
    const res = await req("POST", vaultRollbackUrl(), { name, version });
    if (!stillHere(state, gen, store)) {
      return;
    }
    if (res.status !== 200) {
      store.loadError = errorMessage(res.data) ?? FAIL_SENTENCE;
      paint(state, root);
      return;
    }
    store.status = "idle";
    await loadVault(state, root);
    if (!stillHere(state, gen, store)) {
      return;
    }
    store.selected = name;
    await selectName(state, root, name);
  } catch {
    if (!stillHere(state, gen, store)) {
      return;
    }
    store.loadError = FAIL_SENTENCE;
    paint(state, root);
  } finally {
    if (stillHere(state, gen, store)) {
      store.rolling = false;
      paint(state, root);
    }
  }
}
