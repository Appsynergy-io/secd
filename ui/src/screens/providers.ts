/** Providers screen. Placeholder until the screen lands; the shell contract is lib/host.ts. */

import type { AppState, Host } from "../lib/host.ts";

export function renderProviders(_state: AppState, root: HTMLElement, _host: Host): void {
  root.replaceChildren();
}

export function leaveProviders(_state: object): void {}
