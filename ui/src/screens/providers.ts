/** Providers: field schemas and the env vars the CLI exports. */

import {
  FAIL_SENTENCE,
  errorMessage,
  providerDeletePath,
  providersUrl,
  req,
  vaultUrl,
  type Http,
} from "../lib/api.ts";
import { checkName } from "../lib/crypto.ts";
import { el } from "../lib/dom.ts";
import { currentLogoutGen } from "../lib/gen.ts";
import type { AppState, Host } from "../lib/host.ts";

export const LEDE =
  "A provider is a field schema and the environment variables the CLI exports. Built-ins ship with the binary; custom ones live in the vault store.";
export const NEW_PROVIDER_LABEL = "New provider";
export const WIZARD_SUB = "A field schema. Built-ins cannot be replaced.";
export const LOADING_SENTENCE = "Loading providers.";
export const EMPTY_SENTENCE = "No providers.";
export const LOAD_FAIL_SENTENCE = "Providers did not load.";
export const NAME_FIRST_SENTENCE = "Name the provider first.";
export const BAD_NAME_SENTENCE = "That name is not allowed.";
export const TITLE_SENTENCE = "Give the provider a title.";
export const FIELDS_SENTENCE = "Add at least one field with a key and an env.";
export const SAVED_TOAST = "Provider saved";
export const PUT_ROUTE = "PUT /api/v1/providers";

export type ProviderField = {
  key: string;
  secret: boolean;
  optional: boolean;
  env: string;
};

export type ProviderInfo = {
  name: string;
  title: string;
  builtin: boolean;
  fields: ProviderField[];
};

export type FieldDraft = {
  key: string;
  env: string;
  secret: boolean;
  optional: boolean;
};

type Store = {
  gen: number;
  status: "idle" | "loading" | "ready" | "error";
  providers: ProviderInfo[];
  usage: Map<string, number>;
  alert?: string;
  wizard: boolean;
  wizardName: string;
  wizardTitle: string;
  wizardFields: FieldDraft[];
  wizardError?: string;
  saving: boolean;
  inerted: HTMLElement[];
  focusHint?: string;
};

type Ctx = { root: HTMLElement; host: Host };

const stores = new WeakMap<object, Store>();
const contexts = new WeakMap<object, Ctx>();

function record(v: unknown): Record<string, unknown> | undefined {
  return typeof v === "object" && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined;
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

export function parseProviders(data: unknown): ProviderInfo[] {
  const rows = record(data)?.["providers"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: ProviderInfo[] = [];
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

/** `meta.provider` from each vault row. Ciphertext is ignored. */
export function parseVaultProviderNames(data: unknown): string[] {
  const rows = record(data)?.["entries"];
  if (!Array.isArray(rows)) {
    return [];
  }
  const out: string[] = [];
  for (const row of rows) {
    const rec = record(row);
    if (rec === undefined) {
      continue;
    }
    const provider = record(rec["meta"])?.["provider"];
    if (typeof provider === "string" && provider !== "") {
      out.push(provider);
    }
  }
  return out;
}

export function countUsage(names: readonly string[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const n of names) {
    out.set(n, (out.get(n) ?? 0) + 1);
  }
  return out;
}

export function usageLabel(n: number): string {
  if (n <= 0) {
    return "—";
  }
  return n === 1 ? "1 secret" : `${n} secrets`;
}

export function envJoin(fields: readonly Pick<ProviderField, "env">[]): string {
  return fields
    .map((f) => f.env)
    .filter((env) => env !== "")
    .join("  ");
}

export function sourceLabel(builtin: boolean): string {
  return builtin ? "built-in" : "custom";
}

export function deletedToast(name: string): string {
  return `Deleted ${name}`;
}

export function providerNameOk(name: string): boolean {
  return checkName(name) && !name.includes("/");
}

export function filledFields(fields: readonly FieldDraft[]): ProviderField[] {
  const out: ProviderField[] = [];
  for (const f of fields) {
    const key = f.key.trim();
    const env = f.env.trim();
    if (key === "" || env === "") {
      continue;
    }
    out.push({ key, env, secret: f.secret, optional: f.optional });
  }
  return out;
}

export function putProviderBody(
  name: string,
  title: string,
  fields: readonly FieldDraft[],
): { name: string; title: string; fields: ProviderField[] } | undefined {
  const n = name.trim();
  const t = title.trim();
  const rows = filledFields(fields);
  if (!providerNameOk(n) || t === "" || rows.length === 0) {
    return undefined;
  }
  return { name: n, title: t, fields: rows };
}

export function failSentence(data: unknown): string {
  const msg = errorMessage(data);
  return msg !== undefined && msg !== "" ? msg : FAIL_SENTENCE;
}

function blankField(): FieldDraft {
  return { key: "", env: "", secret: false, optional: false };
}

function freshStore(): Store {
  return {
    gen: 0,
    status: "idle",
    providers: [],
    usage: new Map(),
    wizard: false,
    wizardName: "",
    wizardTitle: "",
    wizardFields: [blankField()],
    saving: false,
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

export function renderProviders(state: AppState, root: HTMLElement, host: Host): void {
  const store = storeOf(state);
  contexts.set(state, { root, host });
  paint(state);
  if (store.status === "idle") {
    void loadProviders(state);
  }
}

export function leaveProviders(state: object): void {
  const ctx = contexts.get(state);
  const s = stores.get(state);
  if (s) {
    s.gen += 1;
    releaseInert(s);
    stores.delete(state);
  }
  if (ctx) {
    ctx.host.actions.replaceChildren();
    ctx.root.replaceChildren();
  }
  contexts.delete(state);
}

function focusSelector(root: HTMLElement, active: Element | null): string | undefined {
  if (!(active instanceof HTMLElement) || !root.contains(active)) {
    return undefined;
  }
  if (active.id !== "") {
    return `#${active.id}`;
  }
  const action = active.getAttribute("data-action");
  if (action === null) {
    return undefined;
  }
  const name = active.closest("[data-provider]")?.getAttribute("data-provider");
  if (name !== null && name !== undefined) {
    return `[data-provider="${name}"] [data-action="${action}"]`;
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
  const { root } = ctx;
  const prevSel = focusSelector(root, document.activeElement);
  releaseInert(store);
  const page = pageEl(state, store);
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
  if (hint !== undefined) {
    target = query(root, hint);
  } else if (prevSel !== undefined) {
    target = query(root, prevSel);
  }
  target?.focus();
}

function pageEl(state: object, store: Store): HTMLElement {
  const create = el(
    "button",
    { type: "button", class: "btn btn-primary spacer", "data-action": "new" },
    [NEW_PROVIDER_LABEL],
  );
  create.addEventListener("click", () => {
    openWizard(state);
  });
  const stack: HTMLElement[] = [
    el("div", { class: "hrow" }, [el("div", { class: "page-lede" }, [LEDE]), create]),
  ];
  if (store.alert !== undefined) {
    stack.push(el("div", { class: "alert alert-danger", role: "alert" }, [store.alert]));
  }
  stack.push(listCard(state, store));
  return el("div", { class: "page", "data-width": "1100" }, [
    el("div", { class: "stack" }, stack),
  ]);
}

function listCard(state: object, store: Store): HTMLElement {
  const body: HTMLElement[] = [];
  if (store.status === "idle" || store.status === "loading") {
    body.push(el("div", { class: "empty", "data-state": "loading" }, [LOADING_SENTENCE]));
  } else if (store.status === "error") {
    body.push(
      el("div", { class: "alert alert-danger", role: "alert", "data-state": "error" }, [
        LOAD_FAIL_SENTENCE,
      ]),
    );
  } else if (store.providers.length === 0) {
    body.push(el("div", { class: "empty", "data-state": "empty" }, [EMPTY_SENTENCE]));
  } else {
    body.push(
      el("div", { class: "grid grid-head cols-providers" }, [
        el("div", {}, ["Provider"]),
        el("div", {}, ["Source"]),
        el("div", {}, ["Environment"]),
        el("div", {}, ["In use"]),
      ]),
    );
    for (const p of store.providers) {
      body.push(providerRow(state, store, p));
    }
  }
  return el("div", { class: "card", "data-card": "providers", "aria-label": "Providers" }, body);
}

function providerRow(state: object, store: Store, p: ProviderInfo): HTMLElement {
  const used = store.usage.get(p.name) ?? 0;
  const usage = el("span", { class: "cell-dim", "data-usage": "" }, [usageLabel(used)]);
  const last: HTMLElement[] = [usage];
  if (!p.builtin) {
    const del = el(
      "button",
      {
        type: "button",
        class: "btn btn-sm btn-danger",
        "data-action": "delete",
        disabled: store.saving ? true : undefined,
      },
      ["Delete"],
    );
    del.addEventListener("click", () => {
      void deleteProvider(state, p.name);
    });
    last.push(del);
  }
  return el(
    "div",
    {
      class: "grid cols-providers",
      "data-provider": p.name,
      "data-builtin": p.builtin ? "true" : "false",
    },
    [
      el("div", { class: "hrow" }, [
        el("span", { "data-title": "" }, [p.title]),
        el("span", { class: "cell-mono-xs truncate", "data-name": "" }, [p.name]),
      ]),
      el("div", {}, [
        el("span", { class: "badge badge-sm", "data-source": "" }, [sourceLabel(p.builtin)]),
      ]),
      el("div", { class: "cell-mono-sm truncate", "data-env": "" }, [envJoin(p.fields)]),
      el("div", { class: "hrow cell-right" }, last),
    ],
  );
}

function labeledInput(
  id: string,
  label: string,
  className: string,
  value: string,
  onInput: (v: string) => void,
): HTMLElement {
  const input = el("input", {
    id,
    class: className,
    autocomplete: "off",
    spellcheck: "false",
    value,
  });
  input.addEventListener("input", () => {
    onInput(input.value);
  });
  return el("div", {}, [el("label", { class: "label", for: id }, [label]), input]);
}

function checkLabel(
  id: string,
  label: string,
  checked: boolean,
  onChange: (v: boolean) => void,
): HTMLElement {
  const box = el("input", { type: "checkbox", id });
  box.checked = checked;
  box.addEventListener("change", () => {
    onChange(box.checked);
  });
  return el("label", { class: "hrow", for: id }, [box, label]);
}

function wizardOverlay(state: object, store: Store): HTMLElement {
  const fields = el("div", { class: "stack-sm", "data-fields": "" });
  for (let i = 0; i < store.wizardFields.length; i++) {
    const f = store.wizardFields[i];
    if (f === undefined) {
      continue;
    }
    const idx = i;
    const keyId = `field-${idx}-key`;
    const envId = `field-${idx}-env`;
    fields.append(
      el("div", { class: "stack-xs", "data-field": String(idx) }, [
        el("div", { class: "form-grid" }, [
          labeledInput(keyId, "Key", "input input-mono field", f.key, (v) => {
            const row = store.wizardFields[idx];
            if (row) {
              row.key = v;
            }
          }),
          labeledInput(envId, "Env", "input input-mono field", f.env, (v) => {
            const row = store.wizardFields[idx];
            if (row) {
              row.env = v;
            }
          }),
        ]),
        el("div", { class: "hrow" }, [
          checkLabel(`field-${idx}-secret`, "Secret", f.secret, (v) => {
            const row = store.wizardFields[idx];
            if (row) {
              row.secret = v;
            }
          }),
          checkLabel(`field-${idx}-optional`, "Optional", f.optional, (v) => {
            const row = store.wizardFields[idx];
            if (row) {
              row.optional = v;
            }
          }),
        ]),
      ]),
    );
  }
  const add = el("button", { type: "button", class: "btn btn-sm", "data-action": "add-field" }, [
    "Add field",
  ]);
  add.addEventListener("click", () => {
    addField(state);
  });
  fields.append(add);
  const bodyKids: HTMLElement[] = [
    el("div", { class: "form-grid" }, [
      labeledInput("new-provider-name", "Name", "input input-mono", store.wizardName, (v) => {
        store.wizardName = v;
      }),
      labeledInput("new-provider-title", "Title", "input", store.wizardTitle, (v) => {
        store.wizardTitle = v;
      }),
    ]),
    fields,
  ];
  if (store.wizardError !== undefined) {
    bodyKids.push(el("div", { class: "alert alert-danger", role: "alert" }, [store.wizardError]));
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
    ["Save"],
  );
  save.addEventListener("click", () => {
    void saveProvider(state);
  });
  const overlay = el(
    "div",
    {
      class: "overlay",
      role: "dialog",
      "aria-modal": "true",
      "aria-labelledby": "new-provider-heading",
      "data-wizard": "open",
    },
    [
      el("div", { class: "modal" }, [
        el("div", { class: "modal-head" }, [
          el("div", {}, [
            el("div", { class: "modal-title", id: "new-provider-heading" }, [NEW_PROVIDER_LABEL]),
            el("div", { class: "modal-sub" }, [WIZARD_SUB]),
          ]),
          close,
        ]),
        el("div", { class: "modal-body" }, bodyKids),
        el("div", { class: "modal-foot" }, [
          el("div", { class: "modal-route" }, [PUT_ROUTE]),
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

function openWizard(state: object): void {
  const store = stores.get(state);
  if (!store) {
    return;
  }
  store.wizard = true;
  store.wizardName = "";
  store.wizardTitle = "";
  store.wizardFields = [blankField()];
  delete store.wizardError;
  store.focusHint = "#new-provider-name";
  paint(state);
}

function closeWizard(state: object): void {
  const store = stores.get(state);
  if (!store) {
    return;
  }
  store.wizard = false;
  store.wizardName = "";
  store.wizardTitle = "";
  store.wizardFields = [blankField()];
  delete store.wizardError;
  store.focusHint = '[data-action="new"]';
  paint(state);
}

function addField(state: object): void {
  const store = stores.get(state);
  if (!store) {
    return;
  }
  store.wizardFields.push(blankField());
  store.focusHint = `#field-${store.wizardFields.length - 1}-key`;
  paint(state);
}

function wizardFail(store: Store, sentence: string, state: object): void {
  store.wizardError = sentence;
  store.focusHint = "#new-provider-name";
  paint(state);
}

async function loadProviders(state: object): Promise<void> {
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
    const [providers, vault] = await Promise.all([
      req("GET", providersUrl()),
      req("GET", vaultUrl()),
    ]);
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(providers) || denied(vault)) {
      void ctx.host.signOut();
      return;
    }
    if (providers.status !== 200) {
      store.status = "error";
      paint(state);
      return;
    }
    const list = parseProviders(providers.data);
    store.providers = list;
    store.usage = countUsage(
      vault.status === 200 ? parseVaultProviderNames(vault.data) : [],
    );
    store.status = "ready";
    delete store.alert;
    const counts = (state as AppState).counts;
    counts.set({ ...counts.get(), providers: list.length });
    paint(state);
  } catch {
    if (!live(state, store, gen, lg)) {
      return;
    }
    store.status = "error";
    paint(state);
  }
}

async function saveProvider(state: object): Promise<void> {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx || store.saving) {
    return;
  }
  const name = store.wizardName.trim();
  const title = store.wizardTitle.trim();
  if (name === "") {
    wizardFail(store, NAME_FIRST_SENTENCE, state);
    return;
  }
  if (!providerNameOk(name)) {
    wizardFail(store, BAD_NAME_SENTENCE, state);
    return;
  }
  if (title === "") {
    wizardFail(store, TITLE_SENTENCE, state);
    return;
  }
  const fields = filledFields(store.wizardFields);
  if (fields.length === 0) {
    wizardFail(store, FIELDS_SENTENCE, state);
    return;
  }
  const body = { name, title, fields };
  store.saving = true;
  delete store.wizardError;
  paint(state);
  const gen = store.gen;
  const lg = currentLogoutGen();
  try {
    const res = await req("PUT", providersUrl(), body);
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(res)) {
      void ctx.host.signOut();
      return;
    }
    if (res.status !== 200) {
      store.saving = false;
      wizardFail(store, failSentence(res.data), state);
      return;
    }
    store.wizard = false;
    store.wizardName = "";
    store.wizardTitle = "";
    store.wizardFields = [blankField()];
    delete store.wizardError;
    store.saving = false;
    ctx.host.flash(SAVED_TOAST);
    await loadProviders(state);
  } catch {
    if (!live(state, store, gen, lg)) {
      return;
    }
    store.saving = false;
    wizardFail(store, FAIL_SENTENCE, state);
  }
}

async function deleteProvider(state: object, name: string): Promise<void> {
  const store = stores.get(state);
  const ctx = contexts.get(state);
  if (!store || !ctx || store.saving) {
    return;
  }
  const row = store.providers.find((p) => p.name === name);
  if (row === undefined || row.builtin) {
    return;
  }
  store.saving = true;
  delete store.alert;
  paint(state);
  const gen = store.gen;
  const lg = currentLogoutGen();
  try {
    const res = await req("DELETE", providerDeletePath(name));
    if (!live(state, store, gen, lg)) {
      return;
    }
    if (denied(res)) {
      void ctx.host.signOut();
      return;
    }
    if (res.status !== 200) {
      store.saving = false;
      store.alert = failSentence(res.data);
      paint(state);
      return;
    }
    store.saving = false;
    ctx.host.flash(deletedToast(name));
    await loadProviders(state);
  } catch {
    if (!live(state, store, gen, lg)) {
      return;
    }
    store.saving = false;
    store.alert = FAIL_SENTENCE;
    paint(state);
  }
}
