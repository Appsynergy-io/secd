/** Dummy-vector AEAD for T_VAULT_TS_SEAL_OPEN. Not a secret. */

type CryptoMod = typeof import("./src/lib/crypto.ts");

function die(msg: string): never {
  throw new Error(`vault-roundtrip: ${msg}`);
}

function looksLikeCrypto(mod: object): mod is CryptoMod {
  const rec = mod as Record<string, unknown>;
  return typeof rec["seal"] === "function" && typeof rec["open"] === "function";
}

async function loadConsoleCrypto(): Promise<CryptoMod> {
  const distDir = new URL("./dist/", import.meta.url);
  const htmlFile = Bun.file(new URL("./index.html", distDir));
  if (!(await htmlFile.exists())) {
    die("missing ui/dist/index.html");
  }
  const html = await htmlFile.text();
  const scripts = [...html.matchAll(/\bsrc="(\.\/[^"]+\.js)"/g)].map((m) => m[1]);
  if (scripts.length !== 1 || scripts[0] === undefined) {
    die("dist/index.html must load exactly one script");
  }
  const entryUrl = new URL(scripts[0], distDir);
  const entryFile = Bun.file(entryUrl);
  if (!(await entryFile.exists())) {
    die("missing console entry");
  }
  const entrySource = await entryFile.text();
  const dyn = [
    ...new Set(
      [...entrySource.matchAll(/\bimport\(\s*"(\.\/[^"]+\.js)"\s*\)/g)].map((m) => m[1]),
    ),
  ];
  if (dyn.length === 0) {
    die("console entry does not dynamically import a chunk");
  }
  let crypto: CryptoMod | undefined;
  for (const rel of dyn) {
    if (rel === undefined) {
      continue;
    }
    let mod: object;
    try {
      mod = (await import(new URL(rel, entryUrl).href)) as object;
    } catch {
      continue;
    }
    if (!looksLikeCrypto(mod)) {
      continue;
    }
    if (crypto !== undefined) {
      die("console entry dynamically imports more than one crypto chunk");
    }
    crypto = mod;
  }
  if (crypto === undefined) {
    die("console entry does not load a crypto chunk");
  }
  return crypto;
}

const op = Bun.argv[2];
const keyHex = Bun.argv[3];
const name = Bun.argv[4];
const payloadHex = Bun.argv[5];
if (
  (op !== "seal" && op !== "open") ||
  keyHex === undefined ||
  name === undefined ||
  payloadHex === undefined
) {
  die("usage: vault-roundtrip.ts seal|open KEY_HEX NAME PAYLOAD_HEX");
}

const crypto = await loadConsoleCrypto();
const key = crypto.fromHex(keyHex);
const payload = crypto.fromHex(payloadHex);
let out: Uint8Array;
if (op === "seal") {
  out = crypto.seal(key, name, payload);
} else {
  out = crypto.open(key, name, payload);
}
const hexOut = crypto.toHex(out);
crypto.zeroizeBytes(key);
crypto.zeroizeBytes(payload);
crypto.zeroizeBytes(out);
await Bun.write(Bun.stdout, hexOut);
