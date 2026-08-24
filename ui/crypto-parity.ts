/** Prove the crypto chunk dist/index.html loads opens the secd-core fixture. Dummy vectors, not secrets. */

import { timingSafeEqual } from "node:crypto";
import { fileURLToPath } from "node:url";

type CryptoMod = typeof import("./src/lib/crypto.ts");

type Fixture = {
  aead: { blob: string; k: string; name: string; nonce: string; plaintext: string };
  wrap_passkey: { blob: string; cred_id: string; dek: string; prf: string };
  wrap_password: { blob: string; dek: string; password: string; salt: string };
  x25519: {
    alg: string;
    blob: string;
    dek: string;
    eph_pk: string;
    eph_sk: string;
    nonce: string;
    peer_pk: string;
  };
};

function die(msg: string): never {
  throw new Error(`crypto-parity: ${msg}`);
}

function eq(a: Uint8Array, b: Uint8Array, what: string): void {
  if (a.length !== b.length || !timingSafeEqual(a, b)) {
    die(`${what} mismatch`);
  }
}

function looksLikeCrypto(mod: object): mod is CryptoMod {
  const rec = mod as Record<string, unknown>;
  return typeof rec["sealWithNonce"] === "function" && typeof rec["open"] === "function";
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

const here = fileURLToPath(new URL(".", import.meta.url));
const fixturePath = `${here}../crates/secd-core/tests/fixtures/crypto-parity.json`;

const fixtureFile = Bun.file(fixturePath);
if (!(await fixtureFile.exists())) {
  die("missing fixture");
}

let fixture: Fixture;
try {
  fixture = JSON.parse(await fixtureFile.text()) as Fixture;
} catch {
  die("fixture is not JSON");
}

const crypto = await loadConsoleCrypto();

const aeadKey = crypto.fromHex(fixture.aead.k);
const aeadNonce = crypto.fromHex(fixture.aead.nonce);
const aeadPlain = crypto.fromHex(fixture.aead.plaintext);
const aeadBlob = crypto.fromHex(fixture.aead.blob);
eq(crypto.sealWithNonce(aeadKey, fixture.aead.name, aeadPlain, aeadNonce), aeadBlob, "aead seal");
eq(crypto.open(aeadKey, fixture.aead.name, aeadBlob), aeadPlain, "aead open");

const dek = crypto.fromHex(fixture.wrap_password.dek);
const password = crypto.fromHex(fixture.wrap_password.password);
const pwWrap = {
  factor: "password" as const,
  blob: fixture.wrap_password.blob,
  salt: fixture.wrap_password.salt,
};
eq(crypto.unwrapPassword(pwWrap, password), dek, "password unwrap");

const prf = crypto.fromHex(fixture.wrap_passkey.prf);
const pkWrap = {
  factor: "passkey" as const,
  blob: fixture.wrap_passkey.blob,
  cred_id: fixture.wrap_passkey.cred_id,
};
eq(crypto.unwrapPasskey(pkWrap, prf), dek, "passkey unwrap");

const ephSk = crypto.fromHex(fixture.x25519.eph_sk);
const peerPk = crypto.fromHex(fixture.x25519.peer_pk);
const ephPk = crypto.fromHex(fixture.x25519.eph_pk);
eq(crypto.x25519Public(ephSk), ephPk, "x25519 public");
const shared = crypto.x25519Shared(ephSk, peerPk);
const xNonce = crypto.fromHex(fixture.x25519.nonce);
const xBlob = crypto.fromHex(fixture.x25519.blob);
const xDek = crypto.fromHex(fixture.x25519.dek);
eq(crypto.sealWithNonce(shared, "dek", xDek, xNonce), xBlob, "x25519 seal");
eq(crypto.open(shared, "dek", xBlob), xDek, "x25519 open");
if (fixture.x25519.alg !== "x25519-xchacha20poly1305") {
  die("x25519 alg");
}

crypto.zeroizeBytes(aeadKey);
crypto.zeroizeBytes(aeadPlain);
crypto.zeroizeBytes(dek);
crypto.zeroizeBytes(password);
crypto.zeroizeBytes(prf);
crypto.zeroizeBytes(ephSk);
crypto.zeroizeBytes(peerPk);
crypto.zeroizeBytes(shared);
crypto.zeroizeBytes(xDek);

console.log("crypto-parity: ok");
