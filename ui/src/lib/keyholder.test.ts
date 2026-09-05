import { afterEach, describe, expect, test } from "bun:test";

import { clearDek } from "./crypto.ts";
import * as keyholder from "./keyholder.ts";

afterEach(async () => {
  await keyholder.lock();
  clearDek();
});

describe("keyholder", () => {
  test("create holds a key and lock drops it", async () => {
    await keyholder.start();
    expect(keyholder.isUnlocked()).toBe(false);
    expect(await keyholder.create()).toBe(true);
    expect(keyholder.isUnlocked()).toBe(true);
    expect(keyholder.remainingMs()).toBeGreaterThan(0);
    await keyholder.lock();
    expect(keyholder.isUnlocked()).toBe(false);
    expect(keyholder.remainingMs()).toBe(0);
  });
});
