import { afterEach, describe, expect, test } from "bun:test";

import { clearDek, fromHex, mintDek, toHex, wrapPassword, wrapToJson } from "./crypto.ts";
import { type Request, handle } from "./keyops.ts";

const NAME = "kv/gh";
const VALUE = JSON.stringify({ token: "not-a-real-token" });
const PASSWORD = "correct horse battery staple";

afterEach(() => {
  clearDek();
});

/** Every reply, flattened, so a test can prove the key is not in any of them. */
function dump(v: unknown): string {
  return JSON.stringify(v);
}

describe("key operations", () => {
  test("create holds a key and state reports it", () => {
    expect(handle({ op: "state" })).toMatchObject({ ok: true, unlocked: false });
    expect(handle({ op: "create" })).toMatchObject({ ok: true, unlocked: true });
    const s = handle({ op: "state" });
    expect(s).toMatchObject({ ok: true, unlocked: true });
    expect(s.remainingMs).toBeGreaterThan(0);
  });

  test("seal and open round-trip through the holder", () => {
    handle({ op: "create" });
    const sealed = handle({ op: "sealEntry", name: NAME, plaintext: VALUE });
    expect(sealed.ok).toBe(true);
    const blob = sealed.ok && sealed.op === "sealEntry" ? sealed.blob : "";
    expect(blob).not.toBe("");
    const opened = handle({ op: "openEntries", entries: [{ name: NAME, blob }] });
    expect(opened.ok && opened.op === "openEntries" ? opened.opened[NAME] : undefined).toBe(VALUE);
  });

  test("a blob that will not open is null, and does not hide the rest", () => {
    handle({ op: "create" });
    const sealed = handle({ op: "sealEntry", name: NAME, plaintext: VALUE });
    const blob = sealed.ok && sealed.op === "sealEntry" ? sealed.blob : "";
    const opened = handle({
      op: "openEntries",
      entries: [
        { name: "kv/bad", blob: "zz" },
        { name: NAME, blob },
        // Sealed under this name, so opening it under another must fail.
        { name: "kv/other", blob },
      ],
    });
    expect(opened.ok).toBe(true);
    if (opened.ok && opened.op === "openEntries") {
      expect(opened.opened["kv/bad"]).toBeNull();
      expect(opened.opened["kv/other"]).toBeNull();
      expect(opened.opened[NAME]).toBe(VALUE);
    }
  });

  test("unlock takes the wrap a password made, and refuses a wrong one", () => {
    const dek = mintDek();
    const wrap = wrapToJson(wrapPassword(dek, new TextEncoder().encode(PASSWORD)));
    expect(handle({ op: "unlock", wraps: { wraps: [wrap] }, password: PASSWORD })).toMatchObject({
      ok: true,
      unlocked: true,
    });
    clearDek();
    expect(handle({ op: "unlock", wraps: { wraps: [wrap] }, password: "wrong" })).toMatchObject({
      ok: false,
      unlocked: false,
    });
  });

  test("every operation refuses while locked", () => {
    const locked: Request[] = [
      { op: "openEntries", entries: [] },
      { op: "sealEntry", name: NAME, plaintext: VALUE },
      { op: "wrapPassword", password: PASSWORD },
      { op: "wrapPasskey", prf: toHex(mintDek()), credId: "aa" },
      { op: "sealToEph", eph: toHex(mintDek()) },
    ];
    for (const req of locked) {
      const r = handle(req);
      expect(r.ok).toBe(false);
      expect(r.ok === false ? r.error : "").toBe("locked");
    }
  });

  test("lock drops the key", () => {
    handle({ op: "create" });
    expect(handle({ op: "lock" })).toMatchObject({ ok: true, unlocked: false });
    expect(handle({ op: "openEntries", entries: [] }).ok).toBe(false);
  });

  test("sealToEph produces the shape the approve route takes", () => {
    handle({ op: "create" });
    const r = handle({ op: "sealToEph", eph: toHex(mintDek()) });
    expect(r.ok).toBe(true);
    if (r.ok && r.op === "sealToEph") {
      expect(typeof r.sealed.alg).toBe("string");
      expect(fromHex(r.sealed.eph_pub).length).toBe(32);
      expect(r.sealed.blob.length).toBeGreaterThan(0);
    }
  });

  test("no reply carries the key", () => {
    // The whole point of the holder: a caller can ask for work, never for the
    // key. Mint a known key, unlock with it, and prove its bytes appear in no
    // reply of any operation.
    const dek = mintDek();
    const hex = toHex(dek);
    const wrap = wrapToJson(wrapPassword(dek, new TextEncoder().encode(PASSWORD)));
    const replies = [
      handle({ op: "unlock", wraps: { wraps: [wrap] }, password: PASSWORD }),
      handle({ op: "state" }),
      handle({ op: "sealEntry", name: NAME, plaintext: VALUE }),
      handle({ op: "wrapPassword", password: PASSWORD }),
      handle({ op: "sealToEph", eph: toHex(mintDek()) }),
    ];
    for (const r of replies) {
      const text = dump(r);
      expect(text.includes(hex)).toBe(false);
      expect(text.toLowerCase().includes(hex.toLowerCase())).toBe(false);
    }
  });
});
