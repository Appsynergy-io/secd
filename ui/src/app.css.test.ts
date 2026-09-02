import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const css = await Bun.file(resolve(root, "src/app.css")).text();

describe("token layer", () => {
  test("the console's ground, surface, accent and success tones are oklch tokens", () => {
    expect(css).toContain("--color-bg: oklch(0.245 0.016 245)");
    expect(css).toContain("--color-bg-deep: oklch(0.21 0.016 245)");
    expect(css).toContain("--color-surface: oklch(0.33 0.017 245)");
    expect(css).toContain("--color-accent: oklch(0.52 0.135 248)");
    expect(css).toContain("--color-accent-text: oklch(0.74 0.115 248)");
    expect(css).toContain("--color-success: oklch(0.62 0.14 155)");
    expect(css).toContain("--color-danger-text: oklch(0.75 0.13 25)");
    expect(css.match(/--color-accent: oklch\(0\.52 0\.135 248\)/g)?.length).toBe(1);
    expect(css).toContain("--side-w: 224px");
    expect(css).toContain("--top-h: 52px");
    expect(css).toContain("--space: 8px");
  });

  test("rules use tokens, not literal colours", () => {
    const body = css.slice(css.indexOf("*,\n*::before"));
    const literals = body.match(/oklch\(/g) ?? [];
    expect(literals.length).toBeLessThanOrEqual(2);
    expect(body).not.toContain("#");
  });

  test("latin Geist faces are file URLs, not data URIs or Google Fonts", () => {
    expect(css).toContain('font-family: "Geist"');
    expect(css).toContain('font-family: "Geist Mono"');
    expect(css).toContain('url("../fonts/geist-latin-wght-normal.woff2")');
    expect(css).toContain('url("../fonts/geist-mono-latin-wght-normal.woff2")');
    expect(css).not.toContain("data:");
    expect(css.toLowerCase()).not.toContain("fonts.google");
  });

  test("motion, focus and layout constraints hold", () => {
    expect(css).toContain("--motion: 150ms");
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toContain(":focus-visible");
    expect(css).toContain("outline: 2px solid var(--color-focus)");
    expect(css).toContain("@media (max-width: 760px)");
    expect(css).toContain('.shell[data-split="true"] .vault');
    expect(css).toContain('.shell[data-hint="false"] .top-hint');
    for (const cls of [".side", ".top", ".content", ".toast", ".overlay", ".modal", ".gate-card", ".approve-card", ".pending-card", ".verified-bar"]) {
      expect(css).toContain(`${cls} {`);
    }
  });
});
