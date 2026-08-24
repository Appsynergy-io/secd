import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { PROVIDERS } from "./providers.gen.ts";
import { parseProviderRs, providersTs } from "./providers.parse.ts";
import { buildPayload, providerByName } from "./providers.ts";

const rustPath = resolve(import.meta.dir, "../../../crates/secd-core/src/provider.rs");
const genPath = resolve(import.meta.dir, "providers.gen.ts");

describe("providers.gen.ts", () => {
  test("matches secd-core/src/provider.rs or the gate fails stale", () => {
    const rust = readFileSync(rustPath, "utf8");
    const parsed = parseProviderRs(rust);
    expect(parsed.map((p) => p.name)).toEqual(PROVIDERS.map((p) => p.name));
    const want = providersTs(parsed);
    const got = readFileSync(genPath, "utf8");
    expect(got === want).toBe(true);
  });

  test("vault schema keeps twelve fields including unseal shares", () => {
    const vault = providerByName("vault");
    expect(vault?.fields.length).toBe(12);
    const values = (vault?.fields ?? []).map((f, i) => [f.key, `v${i}`] as const);
    const payload = buildPayload("vault", values);
    expect(payload === undefined ? 0 : Object.keys(payload).length).toBe(12);
    const minimal = buildPayload("vault", [
      ["addr", "https://vault.lan"],
      ["role_id", "r"],
      ["secret_id", "s"],
    ]);
    expect(minimal === undefined ? 0 : Object.keys(minimal).length).toBe(3);
    expect(buildPayload("vault", [["addr", "https://vault.lan"]])).toBeUndefined();
    expect(buildPayload("vault", [["addr", "  "]])).toBeUndefined();
    expect(buildPayload("nope", values)).toBeUndefined();
    expect(
      buildPayload("cloudflare", [
        ["account_id", "acct"],
        ["api_token", "tok"],
      ]),
    ).toEqual({ account_id: "acct", api_token: "tok" });
  });
});
