import { describe, expect, test } from "bun:test";
import {
  deviceQuery,
  errorMessage,
  passkeyDeletePath,
  queryParam,
  sessionRevokeDelete,
  sessionRevokePath,
  startUrl,
  utf8PercentEncode,
  vaultUrl,
} from "./api.ts";

describe("api", () => {
  test("paths match the contract", () => {
    expect(startUrl()).toBe("/api/auth/start");
    expect(vaultUrl()).toBe("/api/v1/vault");
    expect(sessionRevokePath("ab")).toBe("/api/v1/sessions/ab");
    expect(passkeyDeletePath("pk")).toBe("/api/auth/passkeys/pk");
  });

  test("utf8 percent-encode uses uppercase hex", () => {
    expect(utf8PercentEncode("ab")).toBe("ab");
    expect(utf8PercentEncode("a b")).toBe("a%20b");
    expect(sessionRevokeDelete("a/b")).toBe("DELETE /api/v1/sessions/a%2Fb");
  });

  test("queryParam and deviceQuery read both key spellings", () => {
    expect(queryParam("?x=1&y=2", "y")).toBe("2");
    expect(queryParam("x=a+b", "x")).toBe("a b");
    expect(deviceQuery("?code=ABCD-EFGH&eph=aa")).toEqual({
      code: "ABCD-EFGH",
      eph: "aa",
    });
    expect(deviceQuery("user_code=OLD&eph_pub=bb")).toEqual({
      code: "OLD",
      eph: "bb",
    });
    expect(deviceQuery("")).toEqual({ code: "", eph: "" });
  });

  test("errorMessage reads only the error field", () => {
    expect(errorMessage({ error: "prf" })).toBe("prf");
    expect(errorMessage({ ok: true })).toBeUndefined();
    expect(errorMessage(null)).toBeUndefined();
  });
});
