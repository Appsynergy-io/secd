import { FAIL_SENTENCE } from "./api.ts";

/** PRF salt compiled into the Leptos console; the wrap KEK never leaves the tab. */
export const PRF_SALT: Uint8Array = (() => {
  const salt = new Uint8Array(32);
  salt.set(new TextEncoder().encode("secd-prf-kek-v1"));
  return salt;
})();

function b64Alphabet(): string {
  return "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
}

export function b64urlToBytes(s: string): Uint8Array {
  let t = s.replaceAll("-", "+").replaceAll("_", "/");
  while (t.length % 4 !== 0) {
    t += "=";
  }
  const table = b64Alphabet();
  const val = (c: number): number | undefined => {
    if (c >= 65 && c <= 90) {
      return c - 65;
    }
    if (c >= 97 && c <= 122) {
      return c - 97 + 26;
    }
    if (c >= 48 && c <= 57) {
      return c - 48 + 52;
    }
    if (c === 43) {
      return 62;
    }
    if (c === 47) {
      return 63;
    }
    return undefined;
  };
  const bytes = new TextEncoder().encode(t);
  const out: number[] = [];
  let i = 0;
  while (i + 3 < bytes.length) {
    const a = val(bytes[i] ?? 0);
    const b = val(bytes[i + 1] ?? 0);
    const c = val(bytes[i + 2] ?? 0);
    const d = val(bytes[i + 3] ?? 0);
    if (a !== undefined && b !== undefined) {
      out.push((a << 2) | (b >> 4));
      if (c !== undefined) {
        out.push(((b & 15) << 4) | (c >> 2));
        if (d !== undefined) {
          out.push(((c & 3) << 6) | d);
        }
      }
    }
    i += 4;
  }
  return new Uint8Array(out);
}

export function bytesToB64url(bytes: Uint8Array): string {
  const table = b64Alphabet();
  const ch = (n: number): string => table.charAt(n & 63);
  let s = "";
  let i = 0;
  while (i < bytes.length) {
    const b0 = bytes[i] ?? 0;
    const b1 = bytes[i + 1] ?? 0;
    const b2 = bytes[i + 2] ?? 0;
    s += ch(b0 >> 2);
    s += ch(((b0 & 3) << 4) | (b1 >> 4));
    if (i + 1 < bytes.length) {
      s += ch(((b1 & 15) << 2) | (b2 >> 6));
    }
    if (i + 2 < bytes.length) {
      s += ch(b2);
    }
    i += 3;
  }
  return s.replaceAll("+", "-").replaceAll("/", "_");
}

function asRecord(v: unknown): Record<string, unknown> | undefined {
  if (typeof v !== "object" || v === null || Array.isArray(v)) {
    return undefined;
  }
  return v as Record<string, unknown>;
}

function setBuf(obj: Record<string, unknown>, key: string, s: string): void {
  obj[key] = b64urlToBytes(s).buffer;
}

/** Coerce a WebAuthn publicKey JSON blob into the shape credentials.create/get expect. */
export function coercePublicKey(data: unknown): Record<string, unknown> {
  const root = asRecord(data);
  const pk = asRecord(root?.["publicKey"]) ?? root;
  if (!pk) {
    throw new Error(FAIL_SENTENCE);
  }
  const obj: Record<string, unknown> = { ...pk };
  const challenge = pk["challenge"];
  if (typeof challenge === "string") {
    setBuf(obj, "challenge", challenge);
  }
  const userIn = asRecord(pk["user"]);
  if (userIn) {
    const user = { ...userIn };
    if (typeof userIn["id"] === "string") {
      setBuf(user, "id", userIn["id"]);
    }
    obj["user"] = user;
  }
  for (const listKey of ["excludeCredentials", "allowCredentials"] as const) {
    const list = pk[listKey];
    if (!Array.isArray(list)) {
      continue;
    }
    obj[listKey] = list.map((item) => {
      const rec = asRecord(item) ?? {};
      const next = { ...rec };
      if (typeof rec["id"] === "string") {
        setBuf(next, "id", rec["id"]);
      }
      return next;
    });
  }
  const existing = asRecord(pk["extensions"]) ?? {};
  const ext: Record<string, unknown> = { ...existing };
  ext["prf"] = { eval: { first: PRF_SALT } };
  obj["extensions"] = ext;
  return obj;
}

function readBuf(v: unknown): Uint8Array | undefined {
  if (v instanceof ArrayBuffer) {
    return new Uint8Array(v);
  }
  if (ArrayBuffer.isView(v)) {
    return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
  }
  return undefined;
}

export function serializeCredential(cred: unknown): Record<string, unknown> {
  const rec = asRecord(cred);
  if (!rec) {
    throw new Error(FAIL_SENTENCE);
  }
  const raw = readBuf(rec["rawId"]);
  if (!raw) {
    throw new Error(FAIL_SENTENCE);
  }
  const resp = asRecord(rec["response"]) ?? {};
  const response: Record<string, string> = {};
  for (const name of [
    "attestationObject",
    "clientDataJSON",
    "authenticatorData",
    "signature",
    "userHandle",
  ] as const) {
    const buf = readBuf(resp[name]);
    if (buf) {
      response[name] = bytesToB64url(buf);
    }
  }
  const id = typeof rec["id"] === "string" ? rec["id"] : "";
  return {
    id,
    rawId: bytesToB64url(raw),
    type: "public-key",
    response,
  };
}

export async function createPasskey(
  publicKey: PublicKeyCredentialCreationOptions,
): Promise<PublicKeyCredential> {
  const creds = globalThis.navigator?.credentials;
  if (!creds) {
    throw new Error(FAIL_SENTENCE);
  }
  let got: Credential | null;
  try {
    got = await creds.create({ publicKey });
  } catch {
    throw new Error(FAIL_SENTENCE);
  }
  if (!got || got.type !== "public-key") {
    throw new Error(FAIL_SENTENCE);
  }
  return got as PublicKeyCredential;
}

export async function getPasskey(
  publicKey: PublicKeyCredentialRequestOptions,
  conditional = false,
): Promise<PublicKeyCredential> {
  const creds = globalThis.navigator?.credentials;
  if (!creds) {
    throw new Error(FAIL_SENTENCE);
  }
  const opts: CredentialRequestOptions = { publicKey };
  if (conditional) {
    opts.mediation = "conditional";
  }
  let got: Credential | null;
  try {
    got = await creds.get(opts);
  } catch {
    throw new Error(FAIL_SENTENCE);
  }
  if (!got || got.type !== "public-key") {
    throw new Error(FAIL_SENTENCE);
  }
  return got as PublicKeyCredential;
}

export function prfBytes(cred: unknown): Uint8Array | undefined {
  const rec = asRecord(cred);
  const fn = rec?.["getClientExtensionResults"];
  if (typeof fn !== "function") {
    return undefined;
  }
  let ext: unknown;
  try {
    ext = (fn as () => unknown).call(cred);
  } catch {
    return undefined;
  }
  const first = asRecord(asRecord(asRecord(ext)?.["prf"])?.["results"])?.["first"];
  const buf = readBuf(first);
  if (!buf || buf.length < 32) {
    return undefined;
  }
  return buf.slice(0, 32);
}
