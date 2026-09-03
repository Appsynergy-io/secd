/** The tab's handle on the vault key, which it does not hold itself.
 *
 *  Where the browser has `SharedWorker`, the key lives in one worker shared by
 *  every tab: unlocking in one unlocks the rest, a reload reconnects to a
 *  worker that still holds it, and the bytes never enter page scope. Where it
 *  does not -- Samsung Internet, older Chrome for Android -- the same
 *  operations run in this page, which is exactly the behaviour that shipped
 *  before: one unlock per tab, lost on reload. Neither path writes the key
 *  anywhere.
 */

import type { Reply, Request, SealedEntry } from "./keyops.ts";

export type { SealedEntry };

type Listener = () => void;

const listeners = new Set<Listener>();
let unlocked = false;
/** Epoch ms the key expires, so a tab can count down without asking. */
let deadline = 0;
let started: Promise<void> | undefined;

/** Null until `start`; set only when this browser has a shared worker. */
let port: MessagePort | undefined;
/** Held while the port is live so the worker wrapper is not garbage-collected. */
let worker: SharedWorker | undefined;
/** Loaded only where there is no worker: a browser that has one never
 *  downloads the key machinery at all. */
let local: typeof import("./keyops.ts") | undefined;
let crypto: typeof import("./crypto.ts") | undefined;

/** How long a shared worker gets to answer before the page gives up on it. */
const HANDSHAKE_MS = 3_000;
const WORKER_URL = "/keyholder.worker.js";
const WORKER_NAME = "secd-keyholder";
/** Chrome 148+; the TypeScript DOM lib we compile against does not list it. */
type SharedWorkerInit = WorkerOptions & { extendedLifetime?: boolean };

let nextId = 1;
const waiting = new Map<number, (r: Reply) => void>();

function announce(): void {
  for (const fn of [...listeners]) {
    fn();
  }
}

/** Note the key state after an operation or a worker event. Listeners hear
 *  only about a change: an operation that decrypts does not repaint the tabs,
 *  and a listener that decrypts does not announce itself back into a loop. */
function adopt(s: { unlocked: boolean; remainingMs: number }): void {
  const now = port === undefined ? crypto?.getDek() !== undefined : s.unlocked;
  if (port !== undefined) {
    deadline = s.unlocked ? Date.now() + s.remainingMs : 0;
  }
  if (now !== unlocked) {
    unlocked = now;
    announce();
  }
}

/** Whether the key is held. Synchronous, because render paths ask on every
 *  paint: with a worker that is the last state it sent, without one it is the
 *  page's own key, which cannot be stale. */
export function isUnlocked(): boolean {
  if (port === undefined) {
    return crypto?.getDek() !== undefined;
  }
  if (unlocked && Date.now() >= deadline) {
    unlocked = false;
    deadline = 0;
  }
  return unlocked;
}

/** Milliseconds until the key expires; 0 when none is held. */
export function remainingMs(): number {
  if (port === undefined) {
    return crypto?.dekRemainingMs() ?? 0;
  }
  return isUnlocked() ? Math.max(0, deadline - Date.now()) : 0;
}

/** Fired when the key appears or goes away, in this tab or another one. */
export function subscribe(fn: Listener): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

function listen(w: SharedWorker): MessagePort {
  worker = w;
  w.port.onmessage = (ev: MessageEvent) => {
    const data = ev.data as
      | { evt: "state"; unlocked: boolean; remainingMs: number }
      | { id: number; reply: Reply }
      | undefined;
    if (data === undefined) {
      return;
    }
    if ("evt" in data) {
      adopt(data);
      return;
    }
    const resolve = waiting.get(data.id);
    if (resolve !== undefined) {
      waiting.delete(data.id);
      resolve(data.reply);
    }
  };
  w.port.start();
  return w.port;
}

function tryWorker(
  Ctor: typeof SharedWorker,
  opts: SharedWorkerInit,
): MessagePort | undefined {
  try {
    return listen(new Ctor(WORKER_URL, opts as WorkerOptions));
  } catch {
    return undefined;
  }
}

function openWorker(): MessagePort | undefined {
  const Ctor = (globalThis as typeof globalThis & { SharedWorker?: typeof SharedWorker })
    .SharedWorker;
  if (typeof Ctor !== "function") {
    return undefined;
  }
  // Built as its own unhashed entry point by scripts/build-ui.sh.
  //
  // extendedLifetime keeps the worker alive across the moment a reload
  // closes the last port. Without it a single open tab loses the key on
  // every refresh, because the browser reclaims a worker with no clients.
  // Chrome 148+ honours it for ~30s; a browser that rejects the unknown
  // option is tried again without it, still sharing the key across tabs.
  // Options are pinned on a running worker: a mismatch throws, which is
  // why the retry exists.
  const named: SharedWorkerInit = { type: "module", name: WORKER_NAME };
  return tryWorker(Ctor, { ...named, extendedLifetime: true }) ?? tryWorker(Ctor, named);
}

function abandonWorker(): void {
  waiting.clear();
  const p = port;
  port = undefined;
  worker = undefined;
  if (p !== undefined) {
    try {
      p.close();
    } catch {
      // Already disentangled.
    }
  }
}

/** Connect once. Safe to call from anywhere; later calls await the first. */
export function start(): Promise<void> {
  if (started !== undefined) {
    return started;
  }
  started = (async () => {
    port = openWorker();
    if (port !== undefined) {
      // A worker that never answers must not leave the console unable to
      // unlock, so fall back rather than wait on it for ever.
      const first = await Promise.race([
        send({ op: "state" }),
        new Promise<undefined>((r) => {
          setTimeout(() => {
            r(undefined);
          }, HANDSHAKE_MS);
        }),
      ]);
      if (first?.ok) {
        adopt(first);
        return;
      }
      abandonWorker();
    }
    local = await import("./keyops.ts");
    crypto = await import("./crypto.ts");
    // No worker: the key is this page's, and its expiry is announced here.
    crypto.onDekClear(() => {
      adopt({ unlocked: false, remainingMs: 0 });
    });
    const r = await send({ op: "state" });
    if (r.ok) {
      adopt(r);
    }
  })();
  return started;
}

function send(req: Request): Promise<Reply> {
  if (port !== undefined) {
    const id = nextId++;
    const p = port;
    return new Promise<Reply>((resolve) => {
      waiting.set(id, resolve);
      p.postMessage({ id, req });
    });
  }
  if (local !== undefined) {
    return Promise.resolve(local.handle(req));
  }
  return Promise.resolve({ ok: false, error: "start", unlocked: false, remainingMs: 0 });
}

async function ask(req: Request): Promise<Reply> {
  await start();
  const reply = await send(req);
  adopt(reply);
  return reply;
}

/** Mint a vault key and hold it. For a vault that does not exist yet. */
export async function create(): Promise<boolean> {
  return (await ask({ op: "create" })).ok;
}

/** Unwrap and hold. `prf` is hex; either factor may be given, not both. */
export async function unlock(
  wraps: unknown,
  factor: { prf?: string; password?: string },
): Promise<boolean> {
  return (await ask({ op: "unlock", wraps, ...factor })).ok;
}

/** Drop the key here and in every other tab. */
export async function lock(): Promise<void> {
  await ask({ op: "lock" });
}

/** Plaintext per entry name; `null` for a blob that would not open. */
export async function openEntries(
  entries: SealedEntry[],
): Promise<Record<string, string | null> | undefined> {
  const r = await ask({ op: "openEntries", entries });
  return r.ok && r.op === "openEntries" ? r.opened : undefined;
}

/** The sealed blob for one entry, as hex. */
export async function sealEntry(name: string, plaintext: string): Promise<string | undefined> {
  const r = await ask({ op: "sealEntry", name, plaintext });
  return r.ok && r.op === "sealEntry" ? r.blob : undefined;
}

/** A new passkey wrap of the held key, for the access screen. */
export async function wrapPasskey(
  prf: string,
  credId: string,
): Promise<Record<string, string> | undefined> {
  const r = await ask({ op: "wrapPasskey", prf, credId });
  return r.ok && r.op === "wrapPasskey" ? r.wrap : undefined;
}

/** A new password wrap of the held key. */
export async function wrapPassword(
  password: string,
): Promise<Record<string, string> | undefined> {
  const r = await ask({ op: "wrapPassword", password });
  return r.ok && r.op === "wrapPassword" ? r.wrap : undefined;
}

/** Seal the held key to a device's ephemeral public key, for approval. */
export async function sealToEph(
  eph: string,
): Promise<{ alg: string; eph_pub: string; blob: string } | undefined> {
  const r = await ask({ op: "sealToEph", eph });
  return r.ok && r.op === "sealToEph" ? r.sealed : undefined;
}
