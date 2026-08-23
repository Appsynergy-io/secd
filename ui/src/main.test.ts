import { describe, expect, test } from "bun:test";
import {
  dekFactors,
  hrefFor,
  initialPath,
  lastFactor,
  resolveGate,
  screenFromPath,
  SCREENS,
} from "./main.ts";

describe("router", () => {
  test("five screens with stable hrefs", () => {
    expect(SCREENS).toEqual(["gate", "device", "register", "activity", "account"]);
    expect(hrefFor("gate")).toBe("/");
    expect(hrefFor("device")).toBe("/device");
    expect(hrefFor("register")).toBe("/register");
    expect(hrefFor("activity")).toBe("/activity");
    expect(hrefFor("account")).toBe("/account");
  });

  test("screenFromPath maps the served routes", () => {
    expect(screenFromPath("/")).toBe("gate");
    expect(screenFromPath("/device")).toBe("device");
    expect(screenFromPath("/register")).toBe("register");
    expect(screenFromPath("/activity")).toBe("activity");
    expect(screenFromPath("/account")).toBe("account");
    expect(screenFromPath("/nope")).toBe("gate");
  });

  test("a user code forces the device screen", () => {
    expect(initialPath("/account", "ABCD-EFGH")).toBe("/device");
    expect(initialPath("/account", "")).toBe("/account");
  });
});

describe("resolveGate", () => {
  test("live session is approve-only", () => {
    const v = resolveGate({
      session: {
        email: "a@b.c",
        has_passkey: true,
        has_password: false,
        session_id: "s1",
      },
    });
    expect(v.kind).toBe("approve-only");
    expect(v.showApprove).toBe(true);
    expect(v.showEmail).toBe(false);
  });

  test("cold start shows email with webauthn autocomplete", () => {
    const v = resolveGate({});
    expect(v.kind).toBe("cold");
    expect(v.showEmail).toBe(true);
    expect(v.emailAutocomplete).toBe("username webauthn");
  });
});

describe("dek chain", () => {
  test("the DEK is the factors that unwrap it", () => {
    expect(dekFactors({ has_passkey: true, has_password: true })).toEqual([
      "passkey",
      "password",
    ]);
    expect(dekFactors({ has_passkey: true, has_password: false })).toEqual([
      "passkey",
    ]);
    expect(dekFactors({ has_passkey: false, has_password: true })).toEqual([
      "password",
    ]);
  });

  test("one remaining factor is last", () => {
    expect(lastFactor(["passkey"])).toBe(true);
    expect(lastFactor(["passkey", "password"])).toBe(false);
    expect(lastFactor([])).toBe(false);
  });
});
