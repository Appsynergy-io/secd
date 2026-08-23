import { describe, expect, test } from "bun:test";
import { copyText } from "./clipboard.ts";

describe("clipboard", () => {
  test("returns false when the clipboard API is missing", async () => {
    const nav = globalThis.navigator;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {},
    });
    try {
      expect(await copyText("note-text")).toBe(false);
    } finally {
      Object.defineProperty(globalThis, "navigator", {
        configurable: true,
        value: nav,
      });
    }
  });

  test("writes through navigator.clipboard", async () => {
    let wrote: string | undefined;
    const nav = globalThis.navigator;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {
        clipboard: {
          writeText: async (text: string) => {
            wrote = text;
          },
        },
      },
    });
    try {
      expect(await copyText("note-text")).toBe(true);
      expect(wrote).toBe("note-text");
    } finally {
      Object.defineProperty(globalThis, "navigator", {
        configurable: true,
        value: nav,
      });
    }
  });
});
