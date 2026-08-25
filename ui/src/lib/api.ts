/** JSON API paths used by the console. Cookie credentials, no CORS. */

export const FAIL_SENTENCE = "That email and credential do not match.";
export const RATE_SENTENCE = "Too many attempts. Wait a minute.";
export const EMAIL_AUTOCOMPLETE = "username webauthn";
export const LAST_KEY = "secd.last";
export const BREAKPOINT_PX = 900;
export const REMEMBER_DAYS = 30;
export const NO_DEK_SENTENCE =
  "This browser holds no vault key. Sign out and sign in again, then retry.";
export const NO_EPH_SENTENCE = "Open the approval link printed by the secd CLI.";
export const LAST_FACTOR_SENTENCE = "At least one factor must remain.";

export type LayoutMode = "list-only" | "list-inspector";

export function layoutMode(widthPx: number): LayoutMode {
  return widthPx >= BREAKPOINT_PX ? "list-inspector" : "list-only";
}

export function removePasskeyEnabled(
  passkeyCount: number,
  hasPassword: boolean,
): boolean {
  return !(passkeyCount <= 1 && !hasPassword);
}

export function startUrl(): string {
  return "/api/auth/start";
}

export function sessionUrl(): string {
  return "/api/session";
}

export function logoutUrl(): string {
  return "/api/auth/logout";
}

export function passwordLoginUrl(): string {
  return "/api/auth/password/login";
}

export function passwordRegisterUrl(): string {
  return "/api/auth/password/register";
}

export function passkeyRegisterStartUrl(): string {
  return "/api/auth/passkey/register/start";
}

export function passkeyRegisterFinishUrl(): string {
  return "/api/auth/passkey/register/finish";
}

export function passkeyLoginStartUrl(): string {
  return "/api/auth/passkey/login/start";
}

export function passkeyLoginFinishUrl(): string {
  return "/api/auth/passkey/login/finish";
}

export function passkeysUrl(): string {
  return "/api/auth/passkeys";
}

export function sessionsUrl(): string {
  return "/api/v1/sessions";
}

export function vaultUrl(): string {
  return "/api/v1/vault";
}

export function vaultVersionsUrl(): string {
  return "/api/v1/vault/versions";
}

export function vaultRollbackUrl(): string {
  return "/api/v1/vault/rollback";
}

export function providersUrl(): string {
  return "/api/v1/providers";
}

export function auditUrl(): string {
  return "/api/v1/audit";
}

export function deviceApproveUrl(): string {
  return "/api/v1/device/approve";
}

export function utf8PercentEncode(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let out = "";
  for (const b of bytes) {
    if (
      (b >= 0x41 && b <= 0x5a) ||
      (b >= 0x61 && b <= 0x7a) ||
      (b >= 0x30 && b <= 0x39) ||
      b === 0x2d ||
      b === 0x5f ||
      b === 0x2e ||
      b === 0x7e
    ) {
      out += String.fromCharCode(b);
    } else {
      out += `%${b.toString(16).toUpperCase().padStart(2, "0")}`;
    }
  }
  return out;
}

export function sessionRevokePath(id: string): string {
  return `/api/v1/sessions/${id}`;
}

export function passkeyDeletePath(id: string): string {
  return `/api/auth/passkeys/${id}`;
}

export function sessionRevokeDelete(id: string): string {
  return `DELETE /api/v1/sessions/${utf8PercentEncode(id)}`;
}

export function errorMessage(v: unknown): string | undefined {
  if (typeof v !== "object" || v === null || !("error" in v)) {
    return undefined;
  }
  const err = (v as { error: unknown }).error;
  return typeof err === "string" ? err : undefined;
}

export function queryParam(search: string, key: string): string {
  const q = search.startsWith("?") ? search.slice(1) : search;
  for (const pair of q.split("&")) {
    const eq = pair.indexOf("=");
    if (eq < 0) {
      continue;
    }
    if (pair.slice(0, eq) === key) {
      return pair.slice(eq + 1).replaceAll("+", " ");
    }
  }
  return "";
}

/** The CLI opens `/device?code=…&eph=…`; older links used `user_code`/`eph_pub`. */
export function deviceQuery(search: string): { code: string; eph: string } {
  let code = queryParam(search, "user_code");
  if (code === "") {
    code = queryParam(search, "code");
  }
  let eph = queryParam(search, "eph");
  if (eph === "") {
    eph = queryParam(search, "eph_pub");
  }
  return { code, eph };
}

export type Http = {
  status: number;
  data: unknown;
};

export async function req(
  method: string,
  url: string,
  body?: unknown,
  name?: string,
): Promise<Http> {
  const headers = new Headers();
  if (body !== undefined) {
    headers.set("Content-Type", "application/json");
  }
  if (name !== undefined) {
    headers.set("x-secd-name", name);
  }
  const init: RequestInit = {
    method,
    mode: "same-origin",
    credentials: "same-origin",
    headers,
  };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
  }
  const res = await fetch(url, init);
  const text = await res.text();
  let data: unknown = {};
  try {
    data = JSON.parse(text) as unknown;
  } catch {
    data = {};
  }
  return { status: res.status, data };
}
