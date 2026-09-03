import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import * as keyholder from "./lib/keyholder.ts";
import { clearDek, mintDek, setDek } from "./lib/crypto.ts";
import type { AppState } from "./lib/host.ts";
import { leaveAccess } from "./screens/access.ts";
import { leaveActivity } from "./screens/activity.ts";
import { leaveApprove } from "./screens/approve.ts";
import { leaveDevices } from "./screens/devices.ts";
import { leaveVault } from "./screens/vault.ts";
import {
  HINTS,
  NAV,
  NO_KEY_LABEL,
  SCREENS,
  afterLoginPath,
  asSession,
  flash,
  freshState,
  hrefFor,
  initialPath,
  initials,
  keyLabel,
  layoutFlags,
  render,
  screenFromPath,
  titleFor,
} from "./main.ts";

function mount(): HTMLElement {
  const root = document.createElement("div");
  root.id = "app";
  document.body.replaceChildren(root);
  return root;
}

const session = { email: "ops@imabee.com", session_id: "s1", has_passkey: true, has_password: false };

const origFetch = globalThis.fetch;
const painted: object[] = [];

/** Paint the shell for real, but let no screen's load reach the network. */
function paint(state: AppState): void {
  painted.push(state);
  render(state);
}

beforeEach(async () => {

  await keyholder.start();
  globalThis.fetch = (async (_input: RequestInfo | URL, _init?: RequestInit) =>
    new Response("{}", {
      status: 200,
      headers: { "Content-Type": "application/json" },
    })) as typeof fetch;
});

afterEach(() => {
  for (const state of painted.splice(0)) {
    leaveVault(state);
    leaveActivity(state);
    leaveAccess(state);
    leaveApprove(state);
    leaveDevices(state);
  }
  globalThis.fetch = origFetch;
  clearDek();
  document.body.replaceChildren();
});

describe("router", () => {
  test("seven screens with stable hrefs", () => {
    expect(SCREENS).toEqual(["gate", "approve", "vault", "providers", "devices", "activity", "access"]);
    expect(hrefFor("gate")).toBe("/");
    expect(hrefFor("approve")).toBe("/device");
    expect(hrefFor("vault")).toBe("/vault");
    expect(hrefFor("access")).toBe("/access");
    for (const s of SCREENS) {
      expect(screenFromPath(hrefFor(s))).toBe(s);
    }
    expect(screenFromPath("/register")).toBe("gate");
  });

  test("a user code forces the approval page, before and after unlock", () => {
    expect(initialPath("/access", "ABCD-EFGH")).toBe("/device");
    expect(initialPath("/access", "")).toBe("/access");
    expect(afterLoginPath("ABCD-EFGH")).toBe("/device");
    expect(afterLoginPath("")).toBe("/vault");
  });

  test("nav labels, titles and hints come from one table", () => {
    expect(NAV.map(([id]) => id)).toEqual(["vault", "providers", "devices", "activity", "access"]);
    expect(titleFor("devices")).toBe("Devices");
    expect(HINTS.activity).toBe("append-only, hash-chained, value-free");
  });

  test("layout flags follow the three widths", () => {
    expect(layoutFlags(800)).toEqual({ split: false, wide: false, hint: false });
    expect(layoutFlags(1000)).toEqual({ split: true, wide: false, hint: false });
    expect(layoutFlags(1280)).toEqual({ split: true, wide: true, hint: true });
  });
});

describe("shell", () => {
  test("session JSON is parsed strictly", () => {
    expect(asSession({ email: "a@b.c", session_id: "s", has_passkey: true })).toEqual({
      email: "a@b.c",
      session_id: "s",
      has_passkey: true,
      has_password: false,
    });
    expect(asSession({ email: "a@b.c" })).toBeUndefined();
    expect(asSession("x")).toBeUndefined();
  });

  test("initials and the vault-key label", () => {
    expect(initials("ops@imabee.com")).toBe("OP");
    expect(keyLabel(0)).toBe(NO_KEY_LABEL);
    expect(keyLabel(11 * 3600_000 + 42 * 60_000)).toBe("vault key · 11h 42m left");
  });

  test("a signed-in shell shows the rail, counts, identity and sign-out", () => {
    const root = mount();
    const state = freshState("/activity");
    state.session.set(session);
    state.counts.set({ vault: 12, activity: 8 });
    paint(state);
    const items = [...root.querySelectorAll(".nav-item")];
    expect(items.map((a) => a.textContent)).toEqual(["Vault12", "Providers", "Devices", "Activity8", "Access"]);
    expect(root.querySelector('.nav-item[aria-current="page"]')?.getAttribute("href")).toBe("/activity");
    expect(root.querySelector(".top-title")?.textContent).toBe("Activity");
    expect(root.querySelector(".avatar")?.textContent).toBe("OP");
    expect(root.querySelector("[data-email]")?.textContent).toBe("ops@imabee.com");
    expect(root.querySelector("[data-key]")?.textContent).toBe(NO_KEY_LABEL);
    expect(root.querySelector('[data-action="logout"]')).not.toBeNull();
    expect(root.querySelector('.content[data-screen="activity"]')).not.toBeNull();
    state.counts.set({ vault: 12, activity: 9, devices: 3 });
    expect(root.querySelector('[data-count="activity"]')?.textContent).toBe("9");
    expect(root.querySelector('[data-count="devices"]')?.textContent).toBe("3");
  });

  test("the vault needs the tab DEK, other screens only a session", () => {
    const root = mount();
    const state = freshState("/vault");
    state.session.set(session);
    paint(state);
    expect(root.querySelector('.shell[data-screen="gate"]')).not.toBeNull();
    setDek(mintDek());
    paint(state);
    expect(root.querySelector('.content[data-screen="vault"]')).not.toBeNull();
    expect(root.querySelector(".content")?.getAttribute("data-scroll")).toBe("hidden");
    expect(root.querySelector("[data-key]")?.textContent).toStartWith("vault key · ");
  });

  test("/device stays on the approval page with or without a code", () => {
    const root = mount();
    const bare = freshState("/device");
    bare.session.set(session);
    setDek(mintDek());
    paint(bare);
    expect(bare.path.get()).toBe("/device");
    expect(root.querySelector('.shell[data-screen="approve"]')).not.toBeNull();

    const withCode = freshState("/device", "ABCD-EFGH");
    withCode.session.set(session);
    paint(withCode);
    expect(withCode.path.get()).toBe("/device");
    expect(root.querySelector('.shell[data-screen="approve"]')).not.toBeNull();
  });

  test("a signed-out tab lands on the gate for every path", () => {
    const root = mount();
    for (const path of ["/vault", "/providers", "/devices", "/activity", "/access", "/device", "/"]) {
      const state = freshState(path);
      paint(state);
      expect(root.querySelector('.shell[data-screen="gate"]')).not.toBeNull();
      expect(root.querySelector(".side")).toBeNull();
    }
  });

  test("flash shows the toast and clears it", async () => {
    const root = mount();
    const state = freshState("/access");
    state.session.set(session);
    paint(state);
    const toast = root.querySelector(".toast") as HTMLElement;
    expect(toast.hidden).toBe(true);
    flash(state, "Copied");
    expect(toast.hidden).toBe(false);
    expect(toast.textContent).toBe("Copied");
    state.toast.set("");
    expect(toast.hidden).toBe(true);
  });
});
