/** Built-in provider schemas. Table: providers.gen.ts. */

import { PROVIDERS, type Provider } from "./providers.gen.ts";

export { PROVIDERS, type Provider, type ProviderField } from "./providers.gen.ts";

export function providerByName(name: string): Provider | undefined {
  return PROVIDERS.find((p) => p.name === name);
}

/** Schema-ordered object of non-empty fields. Undefined when unknown or a required field is empty. */
export function buildPayload(
  provider: string,
  values: ReadonlyArray<readonly [string, string]>,
): Record<string, string> | undefined {
  const schema = providerByName(provider);
  if (!schema) {
    return undefined;
  }
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
