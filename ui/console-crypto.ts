/** Load the console's crypto for a check that runs outside the browser.
 *
 *  The console's own chunks carry minified export names, so a checker cannot
 *  reach `open` or `seal` in them by name. This bundles the same module with
 *  the same bundler instead, which is the code the console ships, reachable
 *  by its real names. `ui/dist` must exist first, so a stale or missing build
 *  is still caught here rather than passing quietly.
 */

import { fileURLToPath } from "node:url";

export type CryptoMod = typeof import("./src/lib/crypto.ts");

export function die(msg: string): never {
  throw new Error(`console-crypto: ${msg}`);
}

function looksLikeCrypto(mod: object): mod is CryptoMod {
  const rec = mod as Record<string, unknown>;
  return typeof rec["sealWithNonce"] === "function" && typeof rec["open"] === "function";
}

export async function loadConsoleCrypto(): Promise<CryptoMod> {
  const dist = new URL("./dist/index.html", import.meta.url);
  if (!(await Bun.file(dist).exists())) {
    die("missing ui/dist/index.html -- run scripts/build-ui.sh first");
  }
  const dir = `${process.env["TMPDIR"] ?? "/tmp"}/secd-console-crypto-${process.pid}`;
  const built = await Bun.build({
    entrypoints: [fileURLToPath(new URL("./src/lib/crypto.ts", import.meta.url))],
    outdir: dir,
    target: "browser",
    minify: true,
    sourcemap: "none",
  });
  const first = built.outputs[0];
  if (!built.success || first === undefined) {
    die("could not bundle src/lib/crypto.ts");
  }
  const mod = (await import(first.path)) as object;
  if (!looksLikeCrypto(mod)) {
    die("bundled crypto is missing its exports");
  }
  return mod;
}
