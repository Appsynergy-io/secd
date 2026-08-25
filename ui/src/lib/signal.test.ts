import { describe, expect, test } from "bun:test";
import { signal } from "./signal.ts";

describe("signal", () => {
  test("get returns the initial value", () => {
    const s = signal(1);
    expect(s.get()).toBe(1);
  });

  test("set notifies subscribers", () => {
    const s = signal("a");
    const seen: string[] = [];
    const stop = s.subscribe((v) => {
      seen.push(v);
    });
    s.set("b");
    s.set("c");
    stop();
    s.set("d");
    expect(seen).toEqual(["b", "c"]);
    expect(s.get()).toBe("d");
  });
});
