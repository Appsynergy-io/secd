import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const css = await Bun.file(resolve(root, "src/app.css")).text();

describe("token layer", () => {
  test("Keyring ground, brass accent, and semantic tones are oklch tokens", () => {
    expect(css).toContain("--color-bg: oklch(16% 0.012 250)");
    expect(css).toContain("--color-surface: oklch(19% 0.012 250)");
    expect(css).toContain("--color-accent: oklch(76% 0.14 78)");
    expect(css).toContain("--color-success: oklch(70% 0.15 145)");
    expect(css).toContain("--color-warning: oklch(78% 0.13 80)");
    expect(css).toContain("--color-danger: oklch(64% 0.18 25)");
    expect(css).toContain(
      "--color-border: color-mix(in oklch, var(--color-bg), var(--color-text) 12%)",
    );
  });

  test("light theme is defined under data-theme and prefers-color-scheme", () => {
    expect(css).toContain(':root[data-theme="light"]');
    expect(css).toContain("@media (prefers-color-scheme: light)");
    expect(css).toContain("--space: 8px");
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
