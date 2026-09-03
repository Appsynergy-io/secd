/** Dummy-vector AEAD for T_VAULT_TS_SEAL_OPEN. Not a secret. */

import { die, loadConsoleCrypto } from "./console-crypto.ts";

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
