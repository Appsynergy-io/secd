/** The last account that signed in from this browser, so the gate can prefill it. */

import { FAIL_SENTENCE, LAST_KEY, RATE_SENTENCE, REMEMBER_DAYS } from "./api.ts";

export type Remembered = {
  email: string;
  has_passkey: boolean;
  at: string;
};

function rememberIsFresh(atIso: string, nowMs: number): boolean {
  const at = Date.parse(atIso);
  if (Number.isNaN(at)) {
    return false;
  }
  return nowMs - at <= REMEMBER_DAYS * 24 * 60 * 60 * 1000;
}

export function loadRemember(nowMs = Date.now()): Remembered | undefined {
  try {
    const raw = localStorage.getItem(LAST_KEY);
    if (!raw) {
      return undefined;
    }
    const v = JSON.parse(raw) as unknown;
    if (typeof v !== "object" || v === null) {
      return undefined;
    }
    const rec = v as Record<string, unknown>;
    if (typeof rec["email"] !== "string" || typeof rec["has_passkey"] !== "boolean") {
      return undefined;
    }
    if (typeof rec["at"] !== "string" || !rememberIsFresh(rec["at"], nowMs)) {
      return undefined;
    }
    return {
      email: rec["email"],
      has_passkey: rec["has_passkey"],
      at: rec["at"],
    };
  } catch {
    return undefined;
  }
}

export function saveRemember(email: string, hasPasskey: boolean): void {
  try {
    localStorage.setItem(
      LAST_KEY,
      JSON.stringify({
        email,
        has_passkey: hasPasskey,
        at: new Date().toISOString(),
      }),
    );
  } catch {
    /* ignore quota / private mode */
  }
}

export function forgetRemember(): void {
  try {
    localStorage.removeItem(LAST_KEY);
  } catch {
    /* ignore */
  }
}

export function sentenceFor(status: number): string {
  if (status === 429) {
    return RATE_SENTENCE;
  }
  return FAIL_SENTENCE;
}
