/** Gate screen. Placeholder until the screen lands; the shell contract is lib/host.ts. */

import type { AppState, Host } from "../lib/host.ts";

export function renderGate(_state: AppState, root: HTMLElement, _host: Host): void {
  root.replaceChildren();
}

export function leaveGate(_state: object): void {}
