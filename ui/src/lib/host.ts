/** The shell's contract with a screen. Screens paint into the root the shell
 *  hands them and talk back through the host; nothing else is shared. */

import type { Signal } from "./signal.ts";

export type Screen =
  | "gate"
  | "approve"
  | "vault"
  | "providers"
  | "devices"
  | "activity"
  | "access";

export type AuthMethod = "register" | "passkey" | "password" | "either";

export type SessionInfo = {
  email: string;
  session_id: string;
  has_passkey: boolean;
  has_password: boolean;
};

/** Sidebar counts. A screen sets its own after a load; the shell seeds them once per session. */
export type NavCounts = {
  vault?: number;
  providers?: number;
  devices?: number;
  activity?: number;
};

export type AppState = {
  path: Signal<string>;
  email: Signal<string>;
  password: Signal<string>;
  error: Signal<string | undefined>;
  pending: Signal<boolean>;
  session: Signal<SessionInfo | undefined>;
  method: Signal<AuthMethod | undefined>;
  different: Signal<boolean>;
  revealPassword: Signal<boolean>;
  userCode: Signal<string>;
  eph: Signal<string>;
  counts: Signal<NavCounts>;
  toast: Signal<string>;
};

export type Host = {
  navigate(to: string): void;
  /** Repaint the whole shell and the current screen. */
  redraw(): void;
  /** Bottom-right toast for about two seconds. Never carries a value. */
  flash(message: string): void;
  /** POST /api/auth/logout, drop the tab's DEK, land on the gate. */
  signOut(): Promise<void>;
  /** GET /api/session into state.session; 401 signs the tab out. */
  loadSession(): Promise<void>;
  /** The header's action slot for this screen (the vault's "New secret"). Cleared on every render. */
  actions: HTMLElement;
};

export type ScreenModule = {
  render(state: AppState, root: HTMLElement, host: Host): void;
  leave(state: object): void;
};
