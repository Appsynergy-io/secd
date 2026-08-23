/** Prove ui/dist/crypto.js opens the secd-core fixture. Dummy vectors, not secrets. */

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

const here = fileURLToPath(new URL(".", import.meta.url));
const distUrl = new URL("./dist/crypto.js", import.meta.url);
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

let crypto: CryptoMod;
try {
  crypto = (await import(distUrl.href)) as CryptoMod;
} catch {
  die("missing ui/dist/crypto.js");
}

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
