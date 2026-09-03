/** The operations that need the vault key, and nothing else.
 *
 *  Both the shared worker and the in-page fallback run this module, so the key
 *  is held in exactly one place either way. Requests carry what an operation
 *  needs; replies carry results. The key itself is never a reply, which is the
 *  point: a page that can ask for a value cannot walk off with the vault.
 */

import {
  type Wrap,
  clearDek,
  dekRemainingMs,
  getDek,
  mintDek,
  open,
  seal,
  sealDekToEph,
  setDek,
  unwrapAny,
  wrapPasskey,
  wrapPassword,
  wrapToJson,
  wrapsFromJson,
  zeroizeBytes,
} from "./crypto.ts";
import { fromHex, toHex } from "./crypto.ts";

/** An entry as the vault route carries it: a name and its sealed blob. */
export type SealedEntry = { name: string; blob: string };

export type Request =
  | { op: "state" }
  | { op: "create" }
  | { op: "unlock"; wraps: unknown; prf?: string; password?: string }
  | { op: "lock" }
  | { op: "openEntries"; entries: SealedEntry[] }
  | { op: "sealEntry"; name: string; plaintext: string }
  | { op: "wrapPasskey"; prf: string; credId: string }
  | { op: "wrapPassword"; password: string }
  | { op: "sealToEph"; eph: string };

/** Every reply carries the key state, so a caller never has to ask twice. */
export type State = { unlocked: boolean; remainingMs: number };

export type Reply =
  | ({ ok: true; op: "state" | "create" | "unlock" | "lock" } & State)
  | ({ ok: true; op: "openEntries"; opened: Record<string, string | null> } & State)
  | ({ ok: true; op: "sealEntry"; blob: string } & State)
  | ({ ok: true; op: "wrapPasskey" | "wrapPassword"; wrap: Record<string, string> } & State)
  | ({ ok: true; op: "sealToEph"; sealed: { alg: string; eph_pub: string; blob: string } } & State)
  | ({ ok: false; error: string } & State);

function state(): State {
  return { unlocked: getDek() !== undefined, remainingMs: dekRemainingMs() };
}

function fail(error: string): Reply {
  return { ok: false, error, ...state() };
}

/** Run one request against the held key. Never throws: a failure is a reply. */
export function handle(req: Request): Reply {
  try {
    switch (req.op) {
      case "state":
        return { ok: true, op: "state", ...state() };

      case "create": {
        const fresh = mintDek();
        setDek(fresh);
        zeroizeBytes(fresh);
        return { ok: true, op: "create", ...state() };
      }

      case "unlock": {
        const wraps = wrapsFromJson(req.wraps);
        if (wraps.length === 0) {
          return fail("factor");
        }
        const prf = req.prf === undefined ? undefined : fromHex(req.prf);
        const password = req.password === undefined ? undefined : utf8(req.password);
        const opened = unwrapAny(wraps, password, prf);
        if (prf !== undefined) {
          zeroizeBytes(prf);
        }
        if (password !== undefined) {
          zeroizeBytes(password);
        }
        if (opened === undefined) {
          return fail("factor");
        }
        setDek(opened);
        zeroizeBytes(opened);
        return { ok: true, op: "unlock", ...state() };
      }

      case "lock":
        clearDek();
        return { ok: true, op: "lock", ...state() };

      case "openEntries": {
        const key = getDek();
        if (key === undefined) {
          return fail("locked");
        }
        // A blob that will not open is `null` rather than a thrown request:
        // one damaged entry must not hide the rest of the vault.
        const opened: Record<string, string | null> = {};
        for (const e of req.entries) {
          try {
            const plain = open(key, e.name, fromHex(e.blob));
            opened[e.name] = new TextDecoder().decode(plain);
            zeroizeBytes(plain);
          } catch {
            opened[e.name] = null;
          }
        }
        return { ok: true, op: "openEntries", opened, ...state() };
      }

      case "sealEntry": {
        const key = getDek();
        if (key === undefined) {
          return fail("locked");
        }
        const plain = utf8(req.plaintext);
        const blob = seal(key, req.name, plain);
        zeroizeBytes(plain);
        return { ok: true, op: "sealEntry", blob: toHex(blob), ...state() };
      }

      case "wrapPasskey": {
        const key = getDek();
        if (key === undefined) {
          return fail("locked");
        }
        const prf = fromHex(req.prf);
        const wrap = wrapPasskey(key, prf, req.credId);
        zeroizeBytes(prf);
        return { ok: true, op: "wrapPasskey", wrap: wrapToJson(wrap), ...state() };
      }

      case "wrapPassword": {
        const key = getDek();
        if (key === undefined) {
          return fail("locked");
        }
        const password = utf8(req.password);
        const wrap = wrapPassword(key, password);
        zeroizeBytes(password);
        return { ok: true, op: "wrapPassword", wrap: wrapToJson(wrap), ...state() };
      }

      case "sealToEph": {
        const key = getDek();
        if (key === undefined) {
          return fail("locked");
        }
        const sealed = sealDekToEph(key, fromHex(req.eph));
        return { ok: true, op: "sealToEph", sealed, ...state() };
      }

      default:
        return fail("op");
    }
  } catch (e) {
    return fail(e instanceof Error ? e.message : "fail");
  }
}

function utf8(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}
