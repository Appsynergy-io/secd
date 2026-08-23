import { describe, expect, test } from "bun:test";
import { FAIL_SENTENCE } from "./api.ts";
import {
  PRF_SALT,
  b64urlToBytes,
  bytesToB64url,
  coercePublicKey,
  createPasskey,
  serializeCredential,
} from "./webauthn.ts";

describe("webauthn", () => {
  test("PRF salt is 32 bytes and starts with secd-prf-kek-v1", () => {
    expect(PRF_SALT.length).toBe(32);
    const prefix = new TextEncoder().encode("secd-prf-kek-v1");
    expect(prefix.length).toBe(15);
    expect([...PRF_SALT.subarray(0, 15)]).toEqual([...prefix]);
    expect(PRF_SALT[15]).toBe(0);
  });

  test("b64url round-trips bytes", () => {
    const raw = new Uint8Array([0, 1, 2, 253, 254, 255]);
    const encoded = bytesToB64url(raw);
    expect(encoded.includes("+")).toBe(false);
    expect(encoded.includes("/")).toBe(false);
    expect([...b64urlToBytes(encoded)]).toEqual([...raw]);
  });

  test("coercePublicKey installs PRF eval and buffer challenge", () => {
    const pk = coercePublicKey({
      publicKey: {
        challenge: bytesToB64url(new Uint8Array([1, 2, 3, 4])),
        user: { id: bytesToB64url(new Uint8Array([9])), name: "a", displayName: "a" },
      },
    });
    expect(pk["challenge"]).toBeInstanceOf(ArrayBuffer);
    const ext = pk["extensions"] as { prf: { eval: { first: Uint8Array } } };
    expect(ext.prf.eval.first).toBe(PRF_SALT);
  });

  test("serializeCredential never puts raw bytes in JSON strings besides b64url", () => {
    const raw = new Uint8Array([10, 11, 12]);
    const json = serializeCredential({
      id: "abc",
      rawId: raw.buffer,
      type: "public-key",
      response: { clientDataJSON: raw.buffer },
    });
    expect(json["type"]).toBe("public-key");
    expect(json["rawId"]).toBe(bytesToB64url(raw));
  });

  test("createPasskey fails closed without credentials", async () => {
    const nav = globalThis.navigator;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {},
    });
    try {
      await expect(createPasskey({} as PublicKeyCredentialCreationOptions)).rejects.toThrow(
        FAIL_SENTENCE,
      );
    } finally {
      Object.defineProperty(globalThis, "navigator", {
        configurable: true,
        value: nav,
      });
    }
  });
});
