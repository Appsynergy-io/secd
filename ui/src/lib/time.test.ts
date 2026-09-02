import { describe, expect, test } from "bun:test";
import {
  ago,
  countdown,
  day,
  dayLabel,
  keyFingerprint,
  remainingLabel,
  shortHash,
  stamp,
} from "./time.ts";

const local = (y: number, mo: number, d: number, h = 0, mi = 0, s = 0): Date =>
  new Date(y, mo - 1, d, h, mi, s);

describe("time", () => {
  test("stamp and day are local YYYY-MM-DD HH:MM", () => {
    const at = local(2026, 8, 28, 9, 12);
    expect(stamp(at.toISOString())).toBe("2026-08-28 09:12");
    expect(day(at.toISOString())).toBe("2026-08-28");
    expect(stamp("nope")).toBe("");
  });

  test("ago steps from seconds to days", () => {
    const now = local(2026, 9, 2, 12, 0, 0).getTime();
    const at = (secs: number): string => new Date(now - secs * 1000).toISOString();
    expect(ago(at(3), now)).toBe("just now");
    expect(ago(at(40), now)).toBe("40s ago");
    expect(ago(at(12 * 60), now)).toBe("12 min ago");
    expect(ago(at(60 * 60), now)).toBe("1 hour ago");
    expect(ago(at(5 * 3600), now)).toBe("5 hours ago");
    expect(ago(at(2 * 86400), now)).toBe("2 days ago");
    expect(ago("nope", now)).toBe("");
  });

  test("dayLabel says today, a month-day, or a dated month-day", () => {
    const now = local(2026, 9, 2, 12, 0).getTime();
    expect(dayLabel(local(2026, 9, 2, 8, 14).toISOString(), now)).toBe("today 08:14");
    expect(dayLabel(local(2026, 8, 30, 8, 14).toISOString(), now)).toBe("Aug 30");
    expect(dayLabel(local(2025, 8, 30, 8, 14).toISOString(), now)).toBe("Aug 30, 2025");
  });

  test("countdown and remaining labels", () => {
    expect(countdown(260)).toBe("4m 20s");
    expect(countdown(45)).toBe("45s");
    expect(countdown(-3)).toBe("0s");
    expect(remainingLabel(11 * 3600_000 + 42 * 60_000)).toBe("11h 42m left");
    expect(remainingLabel(42 * 60_000)).toBe("42m left");
    expect(remainingLabel(30_000)).toBe("under a minute left");
    expect(remainingLabel(0)).toBe("expired");
  });

  test("hash and key fingerprints", () => {
    expect(shortHash("a91f" + "0".repeat(56) + "7c4e")).toBe("a91f…7c4e");
    expect(shortHash("abc")).toBe("abc");
    expect(keyFingerprint("4f0c91abd7e23b58" + "f".repeat(48))).toBe("x25519 · 4f0c 91ab d7e2 3b58");
  });
});
