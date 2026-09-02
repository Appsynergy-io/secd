import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const css = await Bun.file(resolve(root, "src/app.css")).text();

describe("token layer", () => {
  test("Keyring ground, brass accent, and semantic tones are oklch tokens", () => {
    expect(css).toContain("--color-bg: oklch(13% 0.008 240)");
    expect(css).toContain("--color-surface: oklch(16% 0.008 240)");
    expect(css).toContain("--color-accent: oklch(62% 0.07 48)");
    expect(css).toContain("--color-success: oklch(70% 0.15 145)");
    expect(css).toContain("--color-warning: oklch(78% 0.13 80)");
    expect(css).toContain("--color-danger: oklch(64% 0.18 25)");
    expect(css).toContain(
      "--color-border: color-mix(in oklch, var(--color-bg), var(--color-text) 28%)",
    );
    expect(css).not.toContain("--color-border: oklch(");
    expect(css).not.toContain("--color-accent: oklch(45%");
    expect(css).not.toContain("--color-danger: oklch(52%");
    expect(css.match(/--color-accent: oklch\(62% 0\.07 48\)/g)?.length).toBe(1);
    expect(css.match(/--color-danger: oklch\(64% 0\.18 25\)/g)?.length).toBe(1);
  });

  test("light theme is defined under data-theme and prefers-color-scheme", () => {
    expect(css).toContain(':root[data-theme="light"]');
    expect(css).toContain("@media (prefers-color-scheme: light)");
    expect(css).toContain("--space: 8px");
    expect(css).toContain("--color-bg: oklch(97% 0.008 90)");
    expect(css).toContain("--color-surface: oklch(94% 0.008 90)");
    expect(css).toContain("--color-focus: oklch(48% 0.08 48)");
    expect(css).toContain("--color-success: oklch(52% 0.15 145)");
  });

  test("latin Geist faces are file URLs, not data URIs or Google Fonts", () => {
    expect(css).toContain('font-family: "Geist"');
    expect(css).toContain('font-family: "Geist Mono"');
    expect(css).toContain('url("../fonts/geist-latin-wght-normal.woff2")');
    expect(css).toContain('url("../fonts/geist-mono-latin-wght-normal.woff2")');
    expect(css).not.toContain("data:");
    expect(css.toLowerCase()).not.toContain("fonts.google");
    expect(css).not.toContain("Martian");
  });

  test("motion, focus, and layout constraints hold", () => {
    expect(css).toContain("150ms");
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toContain(":focus-visible");
    expect(css).toContain("outline: 2px solid var(--color-focus)");
    expect(css).not.toContain("outline: 2px solid var(--color-accent)");
    expect(css).toContain("border-color: var(--color-focus)");
    expect(css).not.toContain("box-shadow: 0 0 0 3px var(--color-accent-soft)");
    expect(css).toContain("@media (min-width: 900px)");
    expect(css).toContain('[data-pane="inspector"]');
    expect(css).toContain('[data-pane="sheet"]');
    expect(css).toContain(".chain");
    expect(css).not.toContain("linear-gradient");
    expect(css).not.toContain("radial-gradient");
  });
});

describe("faces on disk", () => {
  test("only latin normal Geist files and OFL ship under ui/fonts", async () => {
    const latin = Bun.file(resolve(root, "fonts/geist-latin-wght-normal.woff2"));
    const mono = Bun.file(resolve(root, "fonts/geist-mono-latin-wght-normal.woff2"));
    const ofl = await Bun.file(resolve(root, "fonts/OFL.txt")).text();
    expect(await latin.exists()).toBe(true);
    expect(await mono.exists()).toBe(true);
    expect(ofl).toContain("SIL Open Font License");
    expect(ofl).toContain("Geist");
    const names = [] as string[];
    for await (const e of new Bun.Glob("*").scan({ cwd: resolve(root, "fonts") })) {
      names.push(e);
    }
    expect(names.sort()).toEqual([
      "OFL.txt",
      "geist-latin-wght-normal.woff2",
      "geist-mono-latin-wght-normal.woff2",
    ]);
  });
});
