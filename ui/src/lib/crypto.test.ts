import { afterEach, describe, expect, test } from "bun:test";
import {
  CONSOLE_TTL_MS,
  KEY_LEN,
  clearDek,
  getDek,
  onDekClear,
  open,
  seal,
  setDek,
  toHex,
  wrapPasskey,
  wrapPassword,
} from "./crypto.ts";

const DEK = Uint8Array.from([
  0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
  0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf,
]);

function dekMarks(bytes: Uint8Array): string[] {
  const hex = toHex(bytes);
  const latin = String.fromCharCode(...bytes);
  const csv = Array.from(bytes).join(",");
  return [hex, latin, csv, hex.toUpperCase()];
}

function containsDek(value: unknown, marks: string[]): boolean {
  if (typeof value === "string") {
    return marks.some((m) => value.includes(m));
  }
  if (value instanceof Uint8Array) {
    return containsDek(toHex(value), marks) || containsDek(Array.from(value).join(","), marks);
  }
  if (Array.isArray(value)) {
    return value.some((v) => containsDek(v, marks));
  }
  if (typeof value === "object" && value !== null) {
    try {
      return containsDek(JSON.stringify(value), marks);
    } catch {
      return false;
    }
  }
  return false;
}

function installStorageSpies(marks: string[]): { sawDek: () => boolean; restore: () => void } {
  let saw = false;
  const note = (...args: unknown[]) => {
    if (args.some((a) => containsDek(a, marks))) {
      saw = true;
    }
  };
  const orig = {
    localSet: globalThis.localStorage?.setItem.bind(globalThis.localStorage),
    localGet: globalThis.localStorage?.getItem.bind(globalThis.localStorage),
    sessSet: globalThis.sessionStorage?.setItem.bind(globalThis.sessionStorage),
    sessGet: globalThis.sessionStorage?.getItem.bind(globalThis.sessionStorage),
    idbOpen: globalThis.indexedDB?.open.bind(globalThis.indexedDB),
    push: globalThis.history?.pushState.bind(globalThis.history),
    replace: globalThis.history?.replaceState.bind(globalThis.history),
  };
  if (globalThis.localStorage) {
    globalThis.localStorage.setItem = ((...args: unknown[]) => {
      note(...args);
      return orig.localSet?.(...(args as [string, string]));
    }) as typeof localStorage.setItem;
    globalThis.localStorage.getItem = ((...args: unknown[]) => {
      note(...args);
      return orig.localGet?.(...(args as [string])) ?? null;
    }) as typeof localStorage.getItem;
  }
  if (globalThis.sessionStorage) {
    globalThis.sessionStorage.setItem = ((...args: unknown[]) => {
      note(...args);
      return orig.sessSet?.(...(args as [string, string]));
    }) as typeof sessionStorage.setItem;
    globalThis.sessionStorage.getItem = ((...args: unknown[]) => {
      note(...args);
      return orig.sessGet?.(...(args as [string])) ?? null;
    }) as typeof sessionStorage.getItem;
  }
  if (globalThis.indexedDB && orig.idbOpen) {
    globalThis.indexedDB.open = ((...args: unknown[]) => {
      note(...args);
      return orig.idbOpen!(...(args as [string, number?]));
    }) as typeof indexedDB.open;
  }
  if (globalThis.history) {
    globalThis.history.pushState = ((...args: unknown[]) => {
      note(...args);
      return orig.push?.(...(args as [unknown, string, string?]));
    }) as typeof history.pushState;
    globalThis.history.replaceState = ((...args: unknown[]) => {
      note(...args);
      return orig.replace?.(...(args as [unknown, string, string?]));
    }) as typeof history.replaceState;
  }
  return {
    sawDek: () => saw,
    restore: () => {
      if (globalThis.localStorage && orig.localSet) {
        globalThis.localStorage.setItem = orig.localSet;
      }
      if (globalThis.localStorage && orig.localGet) {
        globalThis.localStorage.getItem = orig.localGet;
      }
      if (globalThis.sessionStorage && orig.sessSet) {
        globalThis.sessionStorage.setItem = orig.sessSet;
      }
      if (globalThis.sessionStorage && orig.sessGet) {
        globalThis.sessionStorage.getItem = orig.sessGet;
      }
      if (globalThis.indexedDB && orig.idbOpen) {
        globalThis.indexedDB.open = orig.idbOpen;
      }
      if (globalThis.history && orig.push) {
        globalThis.history.pushState = orig.push;
      }
      if (globalThis.history && orig.replace) {
        globalThis.history.replaceState = orig.replace;
      }
    },
  };
}

afterEach(() => {
  clearDek();
});

describe("dek isolation", () => {
  test("module binding holds the DEK and sign-out clears it", () => {
    expect(getDek()).toBeUndefined();
    setDek(DEK);
    const held = getDek();
    expect(held?.length).toBe(KEY_LEN);
    expect(held !== undefined && toHex(held) === toHex(DEK)).toBe(true);
    clearDek();
    expect(getDek()).toBeUndefined();
  });

  test("clearDek notifies listeners and setDek does not", () => {
    let n = 0;
    const stop = onDekClear(() => {
      n += 1;
    });
    try {
      setDek(DEK);
      expect(n).toBe(0);
      clearDek();
      expect(n).toBe(1);
    } finally {
      stop();
    }
    clearDek();
    expect(n).toBe(1);
  });

  test("console TTL clears the DEK", () => {
    const start = 1_700_000_000_000;
    let now = start;
    const orig = Date.now;
    Date.now = () => now;
    try {
      setDek(DEK);
      expect(getDek()?.length).toBe(KEY_LEN);
      now = start + CONSOLE_TTL_MS;
      expect(getDek()).toBeUndefined();
    } finally {
      Date.now = orig;
    }
  });

  test("storage APIs are never called with anything DEK-derived", () => {
    const marks = dekMarks(DEK);
    const spies = installStorageSpies(marks);
    try {
      setDek(DEK);
      const held = getDek();
      expect(held?.length).toBe(KEY_LEN);
      const blob = seal(DEK, "kv/gitea/token", utf8("fixture-aead-plaintext"));
      const pt = open(DEK, "kv/gitea/token", blob);
      expect(pt.length).toBe(22);
      wrapPassword(DEK, utf8("twelve chars."));
      wrapPasskey(DEK, new Uint8Array(32).fill(0x5c), "cred-1");
      clearDek();
      expect(spies.sawDek()).toBe(false);
    } finally {
      spies.restore();
    }
  });
});

function utf8(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}
