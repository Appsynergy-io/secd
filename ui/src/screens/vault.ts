/** Vault screen: sealed entries in a list, one opened in the detail pane, a
 *  wizard that seals a new one. Plaintext lives only in this module's store. */

import { NO_DEK_SENTENCE, providersUrl, req, vaultRollbackUrl, vaultUrl, vaultVersionsUrl, type Http } from "../lib/api.ts";
import { copyText } from "../lib/clipboard.ts";
import {
  checkName,
  fromHex,
  getDek,
  onDekClear,
  open,
  seal,
  toHex,
  zeroizeBytes,
} from "../lib/crypto.ts";
import { el } from "../lib/dom.ts";
import { currentLogoutGen } from "../lib/gen.ts";
import type { AppState, Host } from "../lib/host.ts";
import { PROVIDERS, type ProviderField } from "../lib/providers.gen.ts";
import { stamp } from "../lib/time.ts";

export const MASK_CHAR = "•";
export const CLIP_TTL_MS = 30_000;
export const LOADING_SENTENCE = "Loading secrets.";
export const EMPTY_SENTENCE = "No secrets yet.";
export const NO_MATCH_SENTENCE = "No names match.";
export const LOAD_FAIL_SENTENCE = "Secrets did not load.";
export const OPEN_FAIL_SENTENCE = "This secret could not be opened.";
export const FIELD_FAIL_SENTENCE = "This field could not be opened.";
export const NAME_FIRST_SENTENCE = "Name the secret first.";
export const REQUIRED_SENTENCE = "Fill every required field.";
export const BAD_NAME_SENTENCE = "That name is not allowed.";
export const VERSIONS_LOADING_SENTENCE = "Loading versions.";
export const VERSIONS_FAIL_SENTENCE = "Versions did not load.";
export const CLIP_FAIL_SENTENCE =
  "The browser refused the clipboard. Select the value and copy it.";
export const CLIP_MISSING_SENTENCE =
  "This browser has no clipboard. Select the value and copy it.";
export const PREIMAGE_SENTENCE = "The vault did not load cleanly. Nothing was saved.";
export const SAVE_FAIL_SENTENCE = "The secret was not saved.";
export const READBACK_SENTENCE = "The saved secret did not read back. Reload before trying again.";
export const RESTORE_FAIL_SENTENCE = "That version was not restored.";
export const NEW_SECRET_LABEL = "New secret";
export const WIZARD_SUB =
  "Sealed in this browser before it is sent. The request carries ciphertext and metadata only.";
export const SEALED_PLACEHOLDER = "sealed before it leaves this browser";
export const SAVED_TOAST = "Sealed and saved · read back and verified";
export const COMMAND_COPIED_TOAST = "Command copied";
export const UNGROUPED = "ungrouped";
export const DEFAULT_ENV = "TOKEN";

export type ProviderSchema = {
  name: string;
  title: string;
  builtin: boolean;
  fields: ProviderField[];
};

export type VaultEntry = {
  name: string;
  ciphertext: unknown;
  meta: Record<string, unknown>;
  provider: string;
  fieldKeys: string[];
  version: number;
  updated: string;
};

export type Opened = {
  fields: Record<string, string>;
  error?: string;
};

export type VersionInfo = { version: number; created: string; provider: string };

export type VersionRow = { version: number; stamp: string; note: string; current: boolean };

export type Group = { name: string; rows: VaultEntry[] };

type PutRow = { name: string; ciphertext: unknown; meta: unknown };

type Store = {
  gen: number;
  status: "idle" | "loading" | "ready" | "error";
  entries: VaultEntry[];
  opened: Map<string, Opened>;
  schemas: ProviderSchema[];
  selected?: string;
  filter: string;
  view: "list" | "detail";
  revealed: Set<string>;
  versions: VersionInfo[];
  versionsStatus: "idle" | "loading" | "ready" | "error";
  wizard: boolean;
  wizardProvider: string;
  wizardName: string;
  wizardValues: Map<string, string>;
  wizardError?: string;
  saving: boolean;
  rolling: boolean;
  alert?: string;
  inerted: HTMLElement[];
  focusHint?: string;
};

type Ctx = { root: HTMLElement; host: Host };

const stores = new WeakMap<object, Store>();
const contexts = new WeakMap<object, Ctx>();
const dekWatches = new WeakMap<object, () => void>();
const clipTimers = new WeakMap<object, ReturnType<typeof setTimeout>>();

/* Pure helpers */

export function groupOf(name: string): string {
  const i = name.indexOf("/");
  return i < 0 ? UNGROUPED : name.slice(0, i);
}

export function leafOf(name: string): string {
  const i = name.indexOf("/");
  return i < 0 ? name : name.slice(i + 1);
}

export function filterEntries(entries: readonly VaultEntry[], query: string): VaultEntry[] {
  const q = query.trim().toLowerCase();
  return entries.filter(
    (e) => q === "" || e.name.toLowerCase().includes(q) || e.provider.toLowerCase().includes(q),
  );
}

/** Groups in order of first appearance; rows keep the vault's order. */
export function groupEntries(entries: readonly VaultEntry[]): Group[] {
  const out: Group[] = [];
  for (const e of entries) {
    const g = groupOf(e.name);
    const found = out.find((x) => x.name === g);
    if (found) {
      found.rows.push(e);
    } else {
      out.push({ name: g, rows: [e] });
    }
  }
  return out;
}

export function mask(value: string): string {
  return MASK_CHAR.repeat(Math.min(Math.max(value.length, 8), 24));
}

export function schemaField(schema: ProviderSchema | undefined, key: string): ProviderField | undefined {
  return schema?.fields.find((f) => f.key === key);
}

export function envName(key: string, schema: ProviderSchema | undefined): string {
  const f = schemaField(schema, key);
  return f !== undefined && f.env !== "" ? f.env : key.toUpperCase();
}

/** `ENV=value` per opened field, in the entry's key order. */
export function envDump(
  keys: readonly string[],
  fields: Readonly<Record<string, string>>,
  schema: ProviderSchema | undefined,
): string {
  const lines: string[] = [];
  for (const k of keys) {
    const v = fields[k];
    if (v !== undefined) {
      lines.push(`${envName(k, schema)}=${v}`);
    }
  }
  return lines.join("\n");
}

export function cliLines(name: string, schema: ProviderSchema | undefined): string[] {
  const first = schema?.fields[0]?.env;
  const env = first !== undefined && first !== "" ? first : DEFAULT_ENV;
  return [`secd run --with ${env}=${name} -- ./deploy.sh`, `secd info ${name}`];
}

/** Newest first; the newest is "current", the rest carry their provider. */
export function versionRows(versions: readonly VersionInfo[]): VersionRow[] {
  const ordered = [...versions].sort((a, b) => b.version - a.version);
  return ordered.map((v, i) => ({
    version: v.version,
    stamp: stamp(v.created),
    note: i === 0 ? "current" : v.provider,
    current: i === 0,
  }));
}

/** Schema-ordered object of non-empty fields; undefined when a required field is empty. */
export function payloadFor(
  schema: ProviderSchema,
  values: ReadonlyArray<readonly [string, string]>,
): Record<string, string> | undefined {
  const out: Record<string, string> = {};
  for (const f of schema.fields) {
    const raw = values.find(([k]) => k === f.key)?.[1] ?? "";
    const v = raw.trim();
    if (v === "") {
      if (f.optional) {
        continue;
      }
      return undefined;
    }
    out[f.key] = v;
  }
  return out;
}

function record(v: unknown): Record<string, unknown> | undefined {
  return typeof v === "object" && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined;
}

export function parseEntry(v: unknown): VaultEntry | undefined {
  const rec = record(v);
  if (!rec) {
    return undefined;
  }
  const name = rec["name"];
  if (typeof name !== "string" || name === "") {
    return undefined;
  }
  const meta = { ...(record(rec["meta"]) ?? {}) };
  const provider = typeof meta["provider"] === "string" ? meta["provider"] : "";
  const fields = meta["fields"];
  let fieldKeys = Array.isArray(fields)
    ? fields.filter((x): x is string => typeof x === "string")
    : [];
  if (fieldKeys.length === 0) {
    const tail = leafOf(name).split("/").pop();
    fieldKeys = [tail !== undefined && tail !== "" ? tail : name];
  }
  const version = rec["version"];
  const updated = rec["updated"];
  return {
    name,
    ciphertext: rec["ciphertext"],
    meta,
    provider,
    fieldKeys,
    version: typeof version === "number" && Number.isFinite(version) ? version : 1,
    updated: typeof updated === "string" ? updated : "",
  };
}

function entriesOf(data: unknown): unknown[] | undefined {
  const rows = record(data)?.["entries"];
  return Array.isArray(rows) ? rows : undefined;
}

/** Entries by name, first occurrence wins; rows without a name are dropped. */
export function parseVault(data: unknown): VaultEntry[] {
  const out: VaultEntry[] = [];
  for (const row of entriesOf(data) ?? []) {
    const e = parseEntry(row);
    if (e && !out.some((x) => x.name === e.name)) {
      out.push(e);
    }
  }
  return out;
}

export function parseVersions(data: unknown): VersionInfo[] {
  const rows = record(data)?.["versions"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: VersionInfo[] = [];
  for (const row of rows) {
    const rec = record(row);
    const version = rec?.["version"];
    if (rec === undefined || typeof version !== "number" || !Number.isFinite(version)) {
      continue;
    }
    const created = rec["created"];
    const provider = record(rec["meta"])?.["provider"];
    out.push({
      version,
      created: typeof created === "string" ? created : "",
      provider: typeof provider === "string" ? provider : "",
    });
  }
  return out.sort((a, b) => b.version - a.version);
}

function parseField(v: unknown): ProviderField | undefined {
  const rec = record(v);
  const key = rec?.["key"];
  if (rec === undefined || typeof key !== "string" || key === "") {
    return undefined;
  }
  const env = rec["env"];
  return {
    key,
    secret: rec["secret"] === true,
    optional: rec["optional"] === true,
    env: typeof env === "string" ? env : "",
  };
}

export function parseProviders(data: unknown): ProviderSchema[] {
  const rows = record(data)?.["providers"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: ProviderSchema[] = [];
  for (const row of rows) {
    const rec = record(row);
    const name = rec?.["name"];
    if (rec === undefined || typeof name !== "string" || name === "") {
      continue;
    }
    if (out.some((p) => p.name === name)) {
      continue;
    }
    const title = rec["title"];
    const fields = Array.isArray(rec["fields"]) ? rec["fields"] : [];
    out.push({
      name,
      title: typeof title === "string" && title !== "" ? title : name,
      builtin: rec["builtin"] === true,
      fields: fields.map(parseField).filter((f): f is ProviderField => f !== undefined),
    });
  }
  return out;
}

export function builtinSchemas(): ProviderSchema[] {
  return PROVIDERS.map((p) => ({ name: p.name, title: p.title, builtin: true, fields: [...p.fields] }));
}

/** The rows a PUT takes back: name, ciphertext, meta. */
export function putEntries(rows: readonly unknown[]): PutRow[] {
  const out: PutRow[] = [];
  for (const row of rows) {
    const rec = record(row);
    const name = rec?.["name"];
    if (rec === undefined || typeof name !== "string" || name === "") {
      continue;
    }
    out.push({ name, ciphertext: rec["ciphertext"], meta: rec["meta"] ?? {} });
  }
  return out;
}

/** The loaded vault as a PUT body; undefined when the load failed or dropped a row. */
export function preimage(data: unknown): PutRow[] | undefined {
  const raw = entriesOf(data);
  if (raw === undefined) {
    return undefined;
  }
  const rows = putEntries(raw);
  return rows.length === raw.length ? rows : undefined;
}

export function openEntry(dek: Uint8Array, entry: VaultEntry): Opened {
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
  const rec = record(parsed);
  if (rec) {
    for (const key of entry.fieldKeys) {
      const v = rec[key];
      if (typeof v === "string") {
        fields[key] = v;
      }
    }
  }
  return { fields };
}

/* Store */

function freshStore(): Store {
  return {
    gen: 0,
    status: "idle",
    entries: [],
    opened: new Map(),
    schemas: builtinSchemas(),
    filter: "",
    view: "list",
    revealed: new Set(),
    versions: [],
    versionsStatus: "idle",
    wizard: false,
    wizardProvider: "",
    wizardName: "",
    wizardValues: new Map(),
    saving: false,
    rolling: false,
    inerted: [],
  };
}

function storeOf(state: object): Store {
  let s = stores.get(state);
  if (!s) {
    s = freshStore();
    stores.set(state, s);
  }
  return s;
}

function live(state: object, store: Store, gen: number, lg: number): boolean {
  return stores.get(state) === store && store.gen === gen && currentLogoutGen() === lg;
}

function denied(res: Http): boolean {
  return res.status === 401 || res.status === 403;
}

function schemaFor(store: Store, provider: string): ProviderSchema | undefined {
  return provider === "" ? undefined : store.schemas.find((p) => p.name === provider);
}

function selectedEntry(store: Store): VaultEntry | undefined {
  return store.selected === undefined
    ? undefined
    : store.entries.find((e) => e.name === store.selected);
}

function dropPlaintext(store: Store): void {
  store.opened.clear();
  store.wizardValues = new Map();
  store.revealed.clear();
}

function blankClipboard(): void {
  void copyText("");
}

function cancelClipboardBlank(state: object): boolean {
  const prev = clipTimers.get(state);
  if (prev === undefined) {
    return false;
  }
  clearTimeout(prev);
  clipTimers.delete(state);
  return true;
}

function scheduleClipboardBlank(state: object): void {
  cancelClipboardBlank(state);
  clipTimers.set(
    state,
    setTimeout(() => {
      clipTimers.delete(state);
      blankClipboard();
    }, CLIP_TTL_MS),
  );
}

function releaseInert(store: Store): void {
  for (const node of store.inerted) {
    node.removeAttribute("inert");
    node.inert = false;
  }
  store.inerted = [];
}

/** Everything outside the overlay's ancestor chain, up to the body, becomes inert. */
function inertOthers(store: Store, overlay: HTMLElement): void {
  let node: HTMLElement = overlay;
  while (node.parentElement !== null && node !== document.body) {
    const parent = node.parentElement;
    for (const sib of Array.from(parent.children)) {
      if (sib !== node && sib instanceof HTMLElement && !sib.hasAttribute("inert")) {
        sib.setAttribute("inert", "");
        sib.inert = true;
        store.inerted.push(sib);
      }
    }
    node = parent;
  }
}

function watchDek(state: object): void {
  if (dekWatches.has(state)) {
    return;
  }
  dekWatches.set(
    state,
    onDekClear(() => {
      const s = stores.get(state);
      if (!s) {
        return;
      }
      dropPlaintext(s);
      paint(state);
    }),
  );
}

/* Screen contract */

export function renderVault(state: AppState, root: HTMLElement, host: Host): void {
  const store = storeOf(state);
  contexts.set(state, { root, host });
  watchDek(state);
  host.actions.replaceChildren(newSecretButton(state));
  paint(state);
  if (store.status === "idle") {
    void loadVault(state);
  }
}

export function leaveVault(state: object): void {
  dekWatches.get(state)?.();
  dekWatches.delete(state);
  if (cancelClipboardBlank(state)) {
    blankClipboard();
  }
  const ctx = contexts.get(state);
  const s = stores.get(state);
  if (s) {
    s.gen += 1;
    releaseInert(s);
    dropPlaintext(s);
    stores.delete(state);
  }
  if (ctx) {
    ctx.host.actions.replaceChildren();
    ctx.root.replaceChildren();
  }
  contexts.delete(state);
}

/* Painting */

function focusSelector(root: HTMLElement, active: Element | null): string | undefined {
  if (!(active instanceof HTMLElement) || !root.contains(active)) {
    return undefined;
  }
  if (active.id !== "") {
    return `#${active.id}`;
  }
  if (active.hasAttribute("data-search")) {
    return "[data-search]";
  }
  const wizardField = active.closest("[data-wizard-field]")?.getAttribute("data-wizard-field");
  if (wizardField !== undefined && wizardField !== null) {
    return `[data-wizard-field="${wizardField}"] input`;
  }
  const action = active.getAttribute("data-action");
  const name = active.getAttribute("data-name");
  if (name !== null) {
    return `[data-name="${name}"]`;
  }
  if (action === null) {
    return undefined;
  }
  const field = active.closest("[data-field]")?.getAttribute("data-field");
  if (field !== undefined && field !== null) {
    return `[data-field="${field}"] [data-action="${action}"]`;
  }
  const version = active.closest("[data-version]")?.getAttribute("data-version");
  if (version !== undefined && version !== null) {
    return `[data-version="${version}"] [data-action="${action}"]`;
  }
  const line = active.closest("[data-line]")?.getAttribute("data-line");
  if (line !== undefined && line !== null) {
    return `[data-line="${line}"] [data-action="${action}"]`;
  }
  return `[data-action="${action}"]`;
}

function query(scope: ParentNode, selector: string): HTMLElement | null {
  try {
    const found = scope.querySelector(selector);
    return found instanceof HTMLElement ? found : null;
  } catch {
    return null;
  }
}

function paint(state: object): void {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx) {
    return;
  }
  const { root, host } = ctx;
  const prevSel = focusSelector(root, document.activeElement);
  releaseInert(store);
  if (getDek() === undefined) {
    dropPlaintext(store);
  }
  const page = el("div", { class: "vault", "data-view": store.view }, [
    listPane(state, store),
    detailPane(state, store),
  ]);
  const nodes: HTMLElement[] = [page];
  let overlay: HTMLElement | undefined;
  if (store.wizard) {
    overlay = wizardOverlay(state, store);
    nodes.push(overlay);
  }
  root.replaceChildren(...nodes);
  if (overlay !== undefined) {
    inertOthers(store, overlay);
  }
  const hint = store.focusHint;
  delete store.focusHint;
  let target: HTMLElement | null = null;
  if (hint === "actions") {
    target = query(host.actions, '[data-action="new"]');
  } else if (hint !== undefined) {
    target = query(root, hint);
  } else if (prevSel !== undefined) {
    target = query(root, prevSel);
  }
  target?.focus();
}

function newSecretButton(state: object): HTMLButtonElement {
  const btn = el(
    "button",
    { type: "button", class: "btn btn-primary btn-md", "data-action": "new" },
    [NEW_SECRET_LABEL],
  );
  btn.addEventListener("click", () => {
    openWizard(state);
  });
  return btn;
}

function rowSub(store: Store, e: VaultEntry): string {
  const schema = schemaFor(store, e.provider);
  const title = schema?.title ?? (e.provider !== "" ? e.provider : "—");
  const when = stamp(e.updated);
  return `${title} · ${when !== "" ? when : "—"}`;
}

function listBody(state: object, store: Store): HTMLElement[] {
  const empty = (text: string, alert: boolean): HTMLElement =>
    el("div", { class: "empty", role: alert ? "alert" : undefined }, [text]);
  if (store.status === "idle" || store.status === "loading") {
    return [empty(LOADING_SENTENCE, false)];
  }
  if (store.status === "error") {
    return [empty(LOAD_FAIL_SENTENCE, true)];
  }
  if (store.entries.length === 0) {
    return [empty(EMPTY_SENTENCE, false)];
  }
  const visible = filterEntries(store.entries, store.filter);
  if (visible.length === 0) {
    return [empty(NO_MATCH_SENTENCE, false)];
  }
  return groupEntries(visible).map((g) => {
    const rows = g.rows.map((e) => {
      const btn = el(
        "button",
        {
          type: "button",
          class: "row-item",
          role: "option",
          "data-name": e.name,
          "aria-selected": String(e.name === store.selected),
        },
        [
          el("span", { class: "row-bar", "aria-hidden": "true" }),
          el("span", { class: "row-main" }, [
            el("span", { class: "row-leaf" }, [leafOf(e.name)]),
            el("span", { class: "row-sub" }, [rowSub(store, e)]),
          ]),
          el("span", { class: "row-version" }, [`v${e.version}`]),
        ],
      );
      btn.addEventListener("click", () => {
        select(state, e.name);
      });
      return btn;
    });
    return el("div", { role: "group", "aria-label": g.name }, [
      el("div", { class: "group-head" }, [g.name]),
      ...rows,
    ]);
  });
}

function listPane(state: object, store: Store): HTMLElement {
  const search = el("input", {
    type: "search",
    class: "input input-search",
    placeholder: "Filter secrets",
    "aria-label": "Filter secrets",
    autocomplete: "off",
    value: store.filter,
    "data-search": "",
  });
  const body = el("div", { role: "listbox", "aria-label": "Secrets", "data-list": "secrets" });
  const fill = (): void => {
    body.replaceChildren(...listBody(state, store));
  };
  search.addEventListener("input", () => {
    store.filter = search.value;
    fill();
  });
  fill();
  return el("div", { class: "list-pane" }, [el("div", { class: "list-search" }, [search]), body]);
}

function alertEl(text: string): HTMLElement {
  return el("div", { class: "alert alert-danger", role: "alert" }, [text]);
}

function shellSplit(): boolean {
  return document.querySelector(".shell")?.getAttribute("data-split") === "true";
}

function detailPane(state: object, store: Store): HTMLElement {
  const pane = el("div", { class: "detail-pane" });
  if (!shellSplit()) {
    const back = el(
      "button",
      { type: "button", class: "btn btn-md btn-quiet back-list", "data-action": "back" },
      ["‹  All secrets"],
    );
    back.addEventListener("click", () => {
      store.view = "list";
      paint(state);
    });
    pane.append(back);
  }
  if (store.alert !== undefined) {
    pane.append(alertEl(store.alert));
  }
  const entry = selectedEntry(store);
  if (!entry) {
    return pane;
  }
  const schema = schemaFor(store, entry.provider);
  const opened = store.opened.get(entry.name);
  const noDek = getDek() === undefined;
  const fields = opened?.fields ?? {};
  const keys = entry.fieldKeys;
  const secretKeys = keys.filter((k) => fields[k] !== undefined && schemaField(schema, k)?.secret !== false);
  const anyHidden = secretKeys.some((k) => !store.revealed.has(k));

  const revealAll = el("button", { type: "button", class: "btn", "data-action": "reveal-all" }, [
    anyHidden ? "Reveal all" : "Hide all",
  ]);
  revealAll.addEventListener("click", () => {
    store.revealed = anyHidden ? new Set(secretKeys) : new Set();
    paint(state);
  });
  const copyEnv = el("button", { type: "button", class: "btn", "data-action": "copy-env" }, [
    "Copy .env",
  ]);
  const count = keys.filter((k) => fields[k] !== undefined).length;
  copyEnv.addEventListener("click", () => {
    void copyToClipboard(state, envDump(keys, fields, schema), `Copied ${count} values as .env`, true);
  });
  if (noDek || opened === undefined || opened.error !== undefined) {
    revealAll.disabled = true;
    copyEnv.disabled = true;
  }
  const when = stamp(entry.updated);
  pane.append(
    el("div", { class: "detail-head" }, [
      el("div", { class: "detail-id" }, [
        el("div", { class: "secret-name", "data-secret-name": "" }, [entry.name]),
        el("div", { class: "meta-line" }, [
          el("span", { class: "pill", "data-provider": "" }, [
            schema?.title ?? (entry.provider !== "" ? entry.provider : "—"),
          ]),
          el("span", { "data-field-count": "" }, [`${keys.length} fields`]),
          el("span", { class: "meta-sep", "aria-hidden": "true" }, ["·"]),
          el("span", { "data-version-line": "" }, [
            `version ${entry.version} · ${when !== "" ? when : "—"}`,
          ]),
        ]),
      ]),
      el("div", { class: "detail-actions" }, [revealAll, copyEnv]),
    ]),
  );

  if (noDek) {
    pane.append(alertEl(NO_DEK_SENTENCE));
  } else if (opened === undefined || opened.error !== undefined) {
    pane.append(alertEl(opened?.error ?? OPEN_FAIL_SENTENCE));
  } else {
    pane.append(fieldsCard(state, store, entry, schema, opened));
  }
  pane.append(cliCard(state, entry, schema), versionsCard(state, store, entry));
  return pane;
}

function fieldsCard(
  state: object,
  store: Store,
  entry: VaultEntry,
  schema: ProviderSchema | undefined,
  opened: Opened,
): HTMLElement {
  const card = el("div", { class: "card", "data-card": "fields" }, [
    el("div", { class: "grid grid-head cols-fields", "aria-hidden": "true" }, [
      el("div", {}, ["Field"]),
      el("div", {}, ["Value"]),
      el("div", {}),
    ]),
  ]);
  for (const key of entry.fieldKeys) {
    const f = schemaField(schema, key);
    const secret = f === undefined || f.secret;
    const value = opened.fields[key];
    const shown = !secret || store.revealed.has(key);
    const display =
      value === undefined ? FIELD_FAIL_SENTENCE : shown ? value : mask(value);
    const toggle = el(
      "button",
      {
        type: "button",
        class: "btn btn-sm",
        "data-action": "reveal",
        disabled: !secret || value === undefined ? true : undefined,
        "aria-pressed": secret ? String(shown) : undefined,
      },
      [!secret ? "Shown" : shown ? "Hide" : "Reveal"],
    );
    toggle.addEventListener("click", () => {
      if (store.revealed.has(key)) {
        store.revealed.delete(key);
      } else {
        store.revealed.add(key);
      }
      paint(state);
    });
    const copy = el(
      "button",
      {
        type: "button",
        class: "btn btn-sm",
        "data-action": "copy",
        disabled: value === undefined ? true : undefined,
      },
      ["Copy"],
    );
    copy.addEventListener("click", () => {
      if (value !== undefined) {
        void copyToClipboard(state, value, `Copied ${key} · cleared from the clipboard in 30s`, true);
      }
    });
    card.append(
      el("div", { class: "grid cols-fields", "data-field": key }, [
        el("div", { class: "truncate" }, [
          el("div", { class: "field-key" }, [key]),
          el("div", { class: "field-key-env" }, [f?.env ?? ""]),
        ]),
        el("div", { class: "field-value", "data-value": "" }, [display]),
        el("div", { class: "field-actions" }, [toggle, copy]),
      ]),
    );
  }
  return card;
}

function cliCard(state: object, entry: VaultEntry, schema: ProviderSchema | undefined): HTMLElement {
  const lines = cliLines(entry.name, schema).map((text, i) => {
    const copy = el("button", { type: "button", class: "btn btn-xs", "data-action": "copy-cli" }, [
      "Copy",
    ]);
    copy.addEventListener("click", () => {
      void copyToClipboard(state, text, COMMAND_COPIED_TOAST, false);
    });
    return el("div", { class: "cli-line", "data-line": String(i) }, [
      el("span", { class: "cli-prompt", "aria-hidden": "true" }, ["$"]),
      el("span", { class: "cli-text" }, [text]),
      copy,
    ]);
  });
  return el("div", { class: "card", "data-card": "cli" }, [
    el("div", { class: "card-head" }, [
      el("div", {}, [
        el("div", { class: "card-title" }, ["Use it without reading it"]),
        el("div", { class: "card-sub" }, [
          "the CLI injects the value; there is no ",
          el("span", { class: "mono" }, ["secd get"]),
        ]),
      ]),
    ]),
    el("div", { class: "cli-lines" }, lines),
  ]);
}

function versionsCard(state: object, store: Store, entry: VaultEntry): HTMLElement {
  const card = el("div", { class: "card", "data-card": "versions" }, [
    el("div", { class: "card-head" }, [el("div", { class: "card-title" }, ["Version history"])]),
  ]);
  if (store.versionsStatus === "loading" || store.versionsStatus === "idle") {
    card.append(el("div", { class: "card-note" }, [VERSIONS_LOADING_SENTENCE]));
    return card;
  }
  if (store.versionsStatus === "error") {
    card.append(el("div", { class: "card-note", role: "alert" }, [VERSIONS_FAIL_SENTENCE]));
    return card;
  }
  for (const v of versionRows(store.versions)) {
    let btn: HTMLButtonElement;
    if (v.current) {
      btn = el(
        "button",
        { type: "button", class: "btn btn-sm", "data-action": "current", disabled: true },
        ["Current"],
      );
    } else {
      btn = el(
        "button",
        {
          type: "button",
          class: "btn btn-sm",
          "data-action": "restore",
          disabled: store.rolling ? true : undefined,
        },
        ["Restore"],
      );
      btn.addEventListener("click", () => {
        void restore(state, entry.name, v.version);
      });
    }
    card.append(
      el("div", { class: "version-row", "data-version": String(v.version) }, [
        el("div", { class: "version-no" }, [`v${v.version}`]),
        el("div", { class: "cell-mono-sm" }, [v.stamp !== "" ? v.stamp : "—"]),
        el("div", { class: "cell-muted" }, [v.note]),
        el("div", { class: "spacer" }, [btn]),
      ]),
    );
  }
  return card;
}

function wizardOverlay(state: object, store: Store): HTMLElement {
  const schema = schemaFor(store, store.wizardProvider);
  const sel = el("select", { id: "wizard-provider", class: "select field" });
  for (const p of store.schemas) {
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
    store.focusHint = "#wizard-provider";
    paint(state);
  });
  const name = el("input", {
    id: "wizard-name",
    class: "input input-mono field",
    placeholder: "prod/github",
    autocomplete: "off",
    spellcheck: "false",
    value: store.wizardName,
  });
  name.addEventListener("input", () => {
    store.wizardName = name.value;
  });
  const fields = el("div", { class: "stack-sm" });
  for (const f of schema?.fields ?? []) {
    const id = `wizard-f-${f.key}`;
    const input = el("input", {
      id,
      class: "input input-sm input-mono field",
      type: f.secret ? "password" : "text",
      placeholder: f.secret ? SEALED_PLACEHOLDER : undefined,
      autocomplete: "off",
      spellcheck: "false",
    });
    input.value = store.wizardValues.get(f.key) ?? "";
    input.addEventListener("input", () => {
      store.wizardValues.set(f.key, input.value);
    });
    fields.append(
      el("div", { "data-wizard-field": f.key }, [
        el("div", { class: "field-head" }, [
          el("label", { class: "label", for: id }, [f.key]),
          el("span", { class: "field-tag", "data-optional": f.optional ? true : undefined }, [
            f.optional ? "optional" : "required",
          ]),
          el("span", { class: "field-env" }, [f.env]),
        ]),
        input,
      ]),
    );
  }
  const body = el("div", { class: "modal-body" }, [
    el("div", { class: "form-grid" }, [
      el("div", {}, [el("label", { class: "label", for: "wizard-provider" }, ["Provider"]), sel]),
      el("div", {}, [el("label", { class: "label", for: "wizard-name" }, ["Name"]), name]),
    ]),
    fields,
  ]);
  if (store.wizardError !== undefined) {
    body.append(alertEl(store.wizardError));
  }
  const close = el(
    "button",
    { type: "button", class: "btn btn-icon spacer", "data-action": "close", "aria-label": "Close" },
    ["×"],
  );
  close.addEventListener("click", () => {
    closeWizard(state);
  });
  const cancel = el("button", { type: "button", class: "btn", "data-action": "cancel" }, ["Cancel"]);
  cancel.addEventListener("click", () => {
    closeWizard(state);
  });
  const save = el(
    "button",
    {
      type: "button",
      class: "btn btn-primary",
      "data-action": "save",
      disabled: store.saving ? true : undefined,
    },
    ["Seal and save"],
  );
  save.addEventListener("click", () => {
    void saveWizard(state);
  });
  const overlay = el(
    "div",
    {
      class: "overlay",
      role: "dialog",
      "aria-modal": "true",
      "aria-labelledby": "wizard-title",
      "data-wizard": "open",
    },
    [
      el("div", { class: "modal" }, [
        el("div", { class: "modal-head" }, [
          el("div", {}, [
            el("div", { class: "modal-title", id: "wizard-title" }, [NEW_SECRET_LABEL]),
            el("div", { class: "modal-sub" }, [WIZARD_SUB]),
          ]),
          close,
        ]),
        body,
        el("div", { class: "modal-foot" }, [
          el("div", { class: "modal-route" }, ["PUT /api/v1/vault"]),
          el("div", { class: "hrow spacer" }, [cancel, save]),
        ]),
      ]),
    ],
  );
  overlay.addEventListener("keydown", (ev) => {
    if (ev.key !== "Escape") {
      return;
    }
    ev.preventDefault();
    closeWizard(state);
  });
  return overlay;
}

/* Actions */

function select(state: object, name: string): void {
  const store = stores.get(state);
  if (!store) {
    return;
  }
  store.selected = name;
  store.view = "detail";
  store.revealed.clear();
  delete store.alert;
  paint(state);
  void loadVersions(state, name);
}

function openWizard(state: object): void {
  const store = stores.get(state);
  if (!store) {
    return;
  }
  store.wizard = true;
  store.wizardProvider = store.schemas[0]?.name ?? "";
  store.wizardName = "";
  store.wizardValues = new Map();
  delete store.wizardError;
  store.focusHint = "#wizard-provider";
  paint(state);
}

function closeWizard(state: object): void {
  const store = stores.get(state);
  if (!store) {
    return;
  }
  store.wizard = false;
  store.wizardValues = new Map();
  delete store.wizardError;
  store.focusHint = "actions";
  paint(state);
}

async function copyToClipboard(
  state: object,
  text: string,
  toast: string,
  secret: boolean,
): Promise<void> {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx) {
    return;
  }
  if (globalThis.navigator?.clipboard === undefined) {
    ctx.host.flash(CLIP_MISSING_SENTENCE);
    return;
  }
  const gen = store.gen;
  const lg = currentLogoutGen();
  const ok = await copyText(text);
  if (!live(state, store, gen, lg)) {
    if (ok && secret) {
      blankClipboard();
    }
    return;
  }
  if (!ok) {
    ctx.host.flash(CLIP_FAIL_SENTENCE);
    return;
  }
  if (secret) {
    scheduleClipboardBlank(state);
  } else {
    cancelClipboardBlank(state);
  }
  ctx.host.flash(toast);
}

function applyVault(state: AppState, store: Store, data: unknown): void {
  store.entries = parseVault(data);
  store.opened.clear();
  const dek = getDek();
  if (dek) {
    for (const e of store.entries) {
      store.opened.set(e.name, openEntry(dek, e));
    }
  }
  store.status = "ready";
  state.counts.set({ ...state.counts.get(), vault: store.entries.length });
  if (selectedEntry(store) === undefined) {
    const first = filterEntries(store.entries, store.filter)[0] ?? store.entries[0];
    if (first === undefined) {
      delete store.selected;
    } else {
      store.selected = first.name;
    }
    store.revealed.clear();
    store.versions = [];
    store.versionsStatus = "idle";
  }
  paint(state);
  if (store.selected !== undefined && store.versionsStatus === "idle") {
    void loadVersions(state, store.selected);
  }
}

async function loadVault(state: AppState): Promise<void> {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx) {
    return;
  }
  const gen = store.gen;
  const lg = currentLogoutGen();
  if (store.status !== "ready") {
    store.status = "loading";
    paint(state);
  }
  try {
    const [vault, providers] = await Promise.all([
      req("GET", vaultUrl()),
      req("GET", providersUrl()).catch((): undefined => undefined),
    ]);
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(vault) || (providers !== undefined && denied(providers))) {
      void ctx.host.signOut();
      return;
    }
    if (providers?.status === 200) {
      const parsed = parseProviders(providers.data);
      if (parsed.length > 0) {
        store.schemas = parsed;
      }
    }
    if (vault.status !== 200) {
      store.status = "error";
      paint(state);
      return;
    }
    applyVault(state, store, vault.data);
  } catch {
    if (!live(state, store, gen, lg)) {
      return;
    }
    store.status = "error";
    paint(state);
  }
}

async function loadVersions(state: object, name: string): Promise<void> {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx) {
    return;
  }
  const gen = store.gen;
  const lg = currentLogoutGen();
  store.versions = [];
  store.versionsStatus = "loading";
  paint(state);
  try {
    const res = await req("GET", vaultVersionsUrl(), undefined, name);
    if (!live(state, store, gen, lg) || store.selected !== name) {
      return;
    }
    if (denied(res)) {
      void ctx.host.signOut();
      return;
    }
    if (res.status !== 200) {
      store.versionsStatus = "error";
      paint(state);
      return;
    }
    store.versions = parseVersions(res.data);
    store.versionsStatus = "ready";
    paint(state);
  } catch {
    if (!live(state, store, gen, lg) || store.selected !== name) {
      return;
    }
    store.versionsStatus = "error";
    paint(state);
  }
}

async function restore(state: object, name: string, version: number): Promise<void> {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx || store.rolling) {
    return;
  }
  store.rolling = true;
  delete store.alert;
  paint(state);
  const gen = store.gen;
  const lg = currentLogoutGen();
  try {
    const res = await req("POST", vaultRollbackUrl(), { name, version });
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(res)) {
      void ctx.host.signOut();
      return;
    }
    if (res.status !== 200) {
      store.alert = RESTORE_FAIL_SENTENCE;
      return;
    }
    ctx.host.flash(`Restored ${name} to v${version}`);
    await loadVault(state as AppState);
    if (!live(state, store, gen, lg)) {
      return;
    }
    store.selected = name;
    await loadVersions(state, name);
  } catch {
    if (live(state, store, gen, lg)) {
      store.alert = RESTORE_FAIL_SENTENCE;
    }
  } finally {
    if (live(state, store, gen, lg)) {
      store.rolling = false;
      paint(state);
    }
  }
}

async function saveWizard(state: object): Promise<void> {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx || store.saving) {
    return;
  }
  const refuse = (sentence: string): void => {
    store.wizardError = sentence;
    paint(state);
  };
  const n = store.wizardName.trim();
  if (n === "") {
    refuse(NAME_FIRST_SENTENCE);
    return;
  }
  if (!checkName(n)) {
    refuse(BAD_NAME_SENTENCE);
    return;
  }
  const schema = schemaFor(store, store.wizardProvider);
  const payload = schema === undefined ? undefined : payloadFor(schema, [...store.wizardValues]);
  if (payload === undefined) {
    refuse(REQUIRED_SENTENCE);
    return;
  }
  const dek = getDek();
  if (dek === undefined) {
    refuse(NO_DEK_SENTENCE);
    return;
  }
  store.saving = true;
  delete store.wizardError;
  paint(state);
  const gen = store.gen;
  const lg = currentLogoutGen();
  try {
    const pt = new TextEncoder().encode(JSON.stringify(payload));
    let sealed: string;
    try {
      sealed = toHex(seal(dek, n, pt));
    } finally {
      zeroizeBytes(pt);
    }
    const current = await req("GET", vaultUrl());
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(current)) {
      void ctx.host.signOut();
      return;
    }
    const pre = current.status === 200 ? preimage(current.data) : undefined;
    if (pre === undefined) {
      store.wizardError = PREIMAGE_SENTENCE;
      return;
    }
    const rows = pre.filter((r) => r.name !== n);
    rows.push({
      name: n,
      ciphertext: sealed,
      meta: { provider: store.wizardProvider, fields: Object.keys(payload) },
    });
    const put = await req("PUT", vaultUrl(), { entries: rows });
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(put)) {
      void ctx.host.signOut();
      return;
    }
    if (put.status !== 200) {
      store.wizardError = SAVE_FAIL_SENTENCE;
      return;
    }
    const back = await req("GET", vaultUrl());
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(back)) {
      void ctx.host.signOut();
      return;
    }
    const saved = back.status === 200 ? parseVault(back.data).find((e) => e.name === n) : undefined;
    if (saved === undefined || saved.ciphertext !== sealed) {
      store.wizardError = READBACK_SENTENCE;
      return;
    }
    store.wizard = false;
    store.wizardValues = new Map();
    store.selected = n;
    store.view = "detail";
    store.revealed.clear();
    store.versions = [];
    store.versionsStatus = "idle";
    store.focusHint = "actions";
    ctx.host.flash(SAVED_TOAST);
    applyVault(state as AppState, store, back.data);
  } catch {
    if (live(state, store, gen, lg)) {
      store.wizardError = SAVE_FAIL_SENTENCE;
    }
  } finally {
    if (live(state, store, gen, lg)) {
      store.saving = false;
      paint(state);
    }
  }
}
