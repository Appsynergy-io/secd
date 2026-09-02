/** Client AEAD and wraps. Same locked params as secd-core.
 *  The DEK lives only in a module binding for the tab's lifetime. */

import { xchacha20poly1305 } from "@noble/ciphers/chacha.js";
import { x25519 } from "@noble/curves/ed25519.js";
import { argon2id } from "@noble/hashes/argon2.js";

export const NONCE_LEN = 24;
export const KEY_LEN = 32;
export const SALT_LEN = 16;
export const ARGON2_M_KIB = 19_456;
export const ARGON2_T = 2;
export const ARGON2_P = 1;
export const ARGON2_VERSION = 0x13;
export const CONSOLE_TTL_MS = 12 * 60 * 60 * 1000;

const MIN_BLOB = NONCE_LEN + 1;
const ALG_X25519 = "x25519-xchacha20poly1305";

export type CryptoError = "key" | "aead" | "truncated" | "hex" | "rng" | "prf" | "factor";

export class CryptoFail extends Error {
  readonly code: CryptoError;
  constructor(code: CryptoError) {
    super(code);
    this.name = "CryptoFail";
    this.code = code;
  }
}

export type Factor = "passkey" | "password";

export type Wrap = {
  factor: Factor;
  blob: string;
  cred_id?: string;
  salt?: string;
};

let dek: Uint8Array | undefined;
let dekDeadline = 0;
let dekTimer: ReturnType<typeof setTimeout> | undefined;
const dekClearListeners = new Set<() => void>();

function fail(code: CryptoError): CryptoFail {
  return new CryptoFail(code);
}

export function zeroizeBytes(buf: Uint8Array): void {
  buf.fill(0);
}

export function mintDek(): Uint8Array {
  return randomBytes(KEY_LEN);
}

export function setDek(bytes: Uint8Array): void {
  if (bytes.length !== KEY_LEN) {
    throw fail("key");
  }
  dropDek();
  dek = new Uint8Array(KEY_LEN);
  dek.set(bytes);
  dekDeadline = Date.now() + CONSOLE_TTL_MS;
  dekTimer = setTimeout(() => {
    clearDek();
  }, CONSOLE_TTL_MS);
}

export function getDek(): Uint8Array | undefined {
  if (dek === undefined) {
    return undefined;
  }
  if (Date.now() >= dekDeadline) {
    clearDek();
    return undefined;
  }
  return dek;
}

/** Milliseconds until the tab drops its DEK; 0 when it holds none. */
export function dekRemainingMs(): number {
  return dek === undefined ? 0 : Math.max(0, dekDeadline - Date.now());
}

/** Fired after the tab DEK is dropped (TTL, sign-out, failed unwrap). Not on replace. */
export function onDekClear(fn: () => void): () => void {
  dekClearListeners.add(fn);
  return () => {
    dekClearListeners.delete(fn);
  };
}

function dropDek(): void {
  if (dekTimer !== undefined) {
    clearTimeout(dekTimer);
    dekTimer = undefined;
  }
  if (dek !== undefined) {
    zeroizeBytes(dek);
    dek = undefined;
  }
  dekDeadline = 0;
}

export function clearDek(): void {
  dropDek();
  for (const fn of [...dekClearListeners]) {
    fn();
  }
}

function randomBytes(n: number): Uint8Array {
  const out = new Uint8Array(n);
  if (globalThis.crypto === undefined || typeof globalThis.crypto.getRandomValues !== "function") {
    throw fail("rng");
  }
  globalThis.crypto.getRandomValues(out);
  return out;
}

function utf8(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}

export function sealWithNonce(
  key: Uint8Array,
  name: string,
  plaintext: Uint8Array,
  nonce: Uint8Array,
): Uint8Array {
  if (key.length !== KEY_LEN || nonce.length !== NONCE_LEN) {
    throw fail("key");
  }
  const aad = utf8(name);
  const cipher = xchacha20poly1305(key, nonce, aad);
  const ct = cipher.encrypt(plaintext);
  const blob = new Uint8Array(NONCE_LEN + ct.length);
  blob.set(nonce, 0);
  blob.set(ct, NONCE_LEN);
  return blob;
}

export function seal(key: Uint8Array, name: string, plaintext: Uint8Array): Uint8Array {
  return sealWithNonce(key, name, plaintext, randomBytes(NONCE_LEN));
}

export function open(key: Uint8Array, name: string, blob: Uint8Array): Uint8Array {
  if (blob.length < MIN_BLOB) {
    throw fail("truncated");
  }
  if (key.length !== KEY_LEN) {
    throw fail("key");
  }
  const nonce = blob.subarray(0, NONCE_LEN);
  const ct = blob.subarray(NONCE_LEN);
  const aad = utf8(name);
  const cipher = xchacha20poly1305(key, nonce, aad);
  try {
    return cipher.decrypt(ct);
  } catch {
    throw fail("aead");
  }
}

function prfKek(prf: Uint8Array): Uint8Array {
  if (prf.length < KEY_LEN) {
    throw fail("prf");
  }
  return prf.subarray(0, KEY_LEN);
}

function derivePasswordKek(password: Uint8Array, salt: Uint8Array): Uint8Array {
  return argon2id(password, salt, {
    t: ARGON2_T,
    m: ARGON2_M_KIB,
    p: ARGON2_P,
    dkLen: KEY_LEN,
    version: ARGON2_VERSION,
  });
}

export function wrapPassword(dekBytes: Uint8Array, password: Uint8Array): Wrap {
  if (dekBytes.length !== KEY_LEN) {
    throw fail("key");
  }
  const salt = randomBytes(SALT_LEN);
  const kek = derivePasswordKek(password, salt);
  try {
    const blob = seal(kek, "password", dekBytes);
    return {
      factor: "password",
      blob: toHex(blob),
      salt: toHex(salt),
    };
  } finally {
    zeroizeBytes(kek);
    zeroizeBytes(salt);
  }
}

export function unwrapPassword(wrap: Wrap, password: Uint8Array): Uint8Array {
  if (wrap.factor !== "password") {
    throw fail("factor");
  }
  const saltHex = wrap.salt;
  if (saltHex === undefined) {
    throw fail("factor");
  }
  const salt = fromHex(saltHex);
  if (salt.length !== SALT_LEN) {
    zeroizeBytes(salt);
    throw fail("hex");
  }
  const kek = derivePasswordKek(password, salt);
  zeroizeBytes(salt);
  try {
    const blob = fromHex(wrap.blob);
    try {
      return open(kek, "password", blob);
    } finally {
      zeroizeBytes(blob);
    }
  } finally {
    zeroizeBytes(kek);
  }
}

export function wrapPasskey(dekBytes: Uint8Array, prf: Uint8Array, credId: string): Wrap {
  if (dekBytes.length !== KEY_LEN) {
    throw fail("key");
  }
  const kek = new Uint8Array(prfKek(prf));
  try {
    const blob = seal(kek, "passkey", dekBytes);
    return {
      factor: "passkey",
      blob: toHex(blob),
      cred_id: credId,
    };
  } finally {
    zeroizeBytes(kek);
  }
}

export function unwrapPasskey(wrap: Wrap, prf: Uint8Array): Uint8Array {
  if (wrap.factor !== "passkey") {
    throw fail("factor");
  }
  const kek = new Uint8Array(prfKek(prf));
  try {
    const blob = fromHex(wrap.blob);
    try {
      return open(kek, "passkey", blob);
    } finally {
      zeroizeBytes(blob);
    }
  } finally {
    zeroizeBytes(kek);
  }
}

export function wrapFromJson(v: unknown): Wrap | undefined {
  if (typeof v !== "object" || v === null) {
    return undefined;
  }
  const rec = v as Record<string, unknown>;
  const factor = rec["factor"];
  if (factor !== "passkey" && factor !== "password") {
    return undefined;
  }
  const blob = rec["blob"];
  if (typeof blob !== "string") {
    return undefined;
  }
  const out: Wrap = { factor, blob };
  if (typeof rec["cred_id"] === "string") {
    out.cred_id = rec["cred_id"];
  }
  if (typeof rec["salt"] === "string") {
    out.salt = rec["salt"];
  }
  return out;
}

export function wrapToJson(w: Wrap): Record<string, string> {
  const m: Record<string, string> = { factor: w.factor };
  if (w.cred_id !== undefined) {
    m["cred_id"] = w.cred_id;
  }
  if (w.salt !== undefined) {
    m["salt"] = w.salt;
  }
  m["blob"] = w.blob;
  return m;
}

export function wrapsFromJson(v: unknown): Wrap[] {
  if (typeof v !== "object" || v === null) {
    return [];
  }
  const arr = (v as { wraps?: unknown }).wraps;
  if (!Array.isArray(arr)) {
    return [];
  }
  const out: Wrap[] = [];
  for (const item of arr) {
    const w = wrapFromJson(item);
    if (w) {
      out.push(w);
    }
  }
  return out;
}

export function unwrapAny(
  wraps: Wrap[],
  password: Uint8Array | undefined,
  prf: Uint8Array | undefined,
): Uint8Array | undefined {
  if (prf !== undefined) {
    for (const w of wraps) {
      if (w.factor === "passkey") {
        try {
          return unwrapPasskey(w, prf);
        } catch {
          /* next wrap */
        }
      }
    }
  }
  if (password !== undefined) {
    for (const w of wraps) {
      if (w.factor === "password") {
        try {
          return unwrapPassword(w, password);
        } catch {
          /* next wrap */
        }
      }
    }
  }
  return undefined;
}

export function sealDekToEph(
  dekBytes: Uint8Array,
  theirPub: Uint8Array,
): { alg: string; eph_pub: string; blob: string } {
  if (dekBytes.length !== KEY_LEN || theirPub.length !== KEY_LEN) {
    throw fail("key");
  }
  const secret = randomBytes(KEY_LEN);
  const sk = new Uint8Array(secret);
  let shared: Uint8Array | undefined;
  try {
    const pub = x25519.getPublicKey(sk);
    const their = new Uint8Array(theirPub);
    shared = x25519.getSharedSecret(sk, their);
    zeroizeBytes(their);
    const blob = seal(shared, "dek", dekBytes);
    return {
      alg: ALG_X25519,
      eph_pub: toHex(pub),
      blob: toHex(blob),
    };
  } catch (e) {
    if (e instanceof CryptoFail) {
      throw e;
    }
    throw fail("key");
  } finally {
    zeroizeBytes(secret);
    zeroizeBytes(sk);
    if (shared !== undefined) {
      zeroizeBytes(shared);
    }
  }
}

export function x25519Shared(secret: Uint8Array, theirPub: Uint8Array): Uint8Array {
  if (secret.length !== KEY_LEN || theirPub.length !== KEY_LEN) {
    throw fail("key");
  }
  const sk = new Uint8Array(secret);
  const their = new Uint8Array(theirPub);
  try {
    return x25519.getSharedSecret(sk, their);
  } catch {
    throw fail("key");
  } finally {
    zeroizeBytes(sk);
    zeroizeBytes(their);
  }
}

export function x25519Public(secret: Uint8Array): Uint8Array {
  if (secret.length !== KEY_LEN) {
    throw fail("key");
  }
  const sk = new Uint8Array(secret);
  try {
    return x25519.getPublicKey(sk);
  } catch {
    throw fail("key");
  } finally {
    zeroizeBytes(sk);
  }
}

export function toHex(bytes: Uint8Array): string {
  const HEX = "0123456789abcdef";
  let out = "";
  for (const b of bytes) {
    out += HEX.charAt(b >> 4);
    out += HEX.charAt(b & 0x0f);
  }
  return out;
}

export function fromHex(s: string): Uint8Array {
  if (s.length % 2 !== 0) {
    throw fail("hex");
  }
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < s.length; i += 2) {
    const hi = hexVal(s.charCodeAt(i));
    const lo = hexVal(s.charCodeAt(i + 1));
    out[i / 2] = (hi << 4) | lo;
  }
  return out;
}

function hexVal(c: number): number {
  if (c >= 48 && c <= 57) {
    return c - 48;
  }
  if (c >= 97 && c <= 102) {
    return c - 97 + 10;
  }
  if (c >= 65 && c <= 70) {
    return c - 65 + 10;
  }
  throw fail("hex");
}

export function checkName(name: string): boolean {
  if (name.length === 0 || name.length > 256) {
    return false;
  }
  if (name.startsWith("/") || name.endsWith("/") || name.includes("..")) {
    return false;
  }
  return name.split("/").every((seg) => {
    if (seg.length === 0) {
      return false;
    }
    for (let i = 0; i < seg.length; i++) {
      const b = seg.charCodeAt(i);
      const ok =
        (b >= 65 && b <= 90) ||
        (b >= 97 && b <= 122) ||
        (b >= 48 && b <= 57) ||
        b === 46 ||
        b === 95 ||
        b === 64 ||
        b === 45;
      if (!ok) {
        return false;
      }
    }
    return true;
  });
}

export function emailOk(raw: string): string | undefined {
  const s = raw.trim().toLowerCase();
  if (s.length === 0 || s.length > 254) {
    return undefined;
  }
  const at = s.indexOf("@");
  if (at < 0) {
    return undefined;
  }
  const local = s.slice(0, at);
  const domain = s.slice(at + 1);
  if (local.length === 0 || domain.length === 0 || domain.includes("@") || !domain.includes(".")) {
    return undefined;
  }
  return s;
}

export function passwordOk(password: string): boolean {
  const n = [...password].length;
  return n >= 12 && n <= 256;
}
