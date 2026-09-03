/** The shared worker that holds the vault key for every tab of this origin.
 *
 *  One instance serves all tabs, so unlocking in one unlocks the rest, and a
 *  reload reconnects to a worker that still holds the key. The page asks for
 *  `extendedLifetime` so a single-tab reload does not destroy this scope in
 *  the gap where no port is connected. The key never leaves here: tabs send
 *  operations and receive results.
 */

/// <reference lib="webworker" />

import { dekRemainingMs, getDek, onDekClear } from "./lib/crypto.ts";
import { type Request, handle } from "./lib/keyops.ts";

type Envelope = { id: number; req: Request };

const ports = new Set<MessagePort>();

function stateNow(): { unlocked: boolean; remainingMs: number } {
  return { unlocked: getDek() !== undefined, remainingMs: dekRemainingMs() };
}

/** Tell every tab but `except`. A port whose tab is gone throws; drop it. */
function broadcast(except?: MessagePort): void {
  const evt = { evt: "state" as const, ...stateNow() };
  for (const port of [...ports]) {
    if (port === except) {
      continue;
    }
    try {
      port.postMessage(evt);
    } catch {
      ports.delete(port);
    }
  }
}

// The twelve hours run in here, so every tab is told at the same moment.
onDekClear(() => {
  broadcast();
});

function connect(port: MessagePort): void {
  ports.add(port);
  port.onmessage = (ev: MessageEvent) => {
    const env = ev.data as Envelope | undefined;
    if (env === undefined || typeof env.id !== "number") {
      return;
    }
    const reply = handle(env.req);
    port.postMessage({ id: env.id, reply });
    // Any operation can change whether the key is held, so the other tabs
    // learn about it without asking.
    broadcast(port);
  };
  port.start();
  port.postMessage({ evt: "state", ...stateNow() });
}

(globalThis as unknown as SharedWorkerGlobalScope).onconnect = (ev: MessageEvent) => {
  const port = ev.ports[0];
  if (port !== undefined) {
    connect(port);
  }
};
