//! Reset + base typography. Two layers, emitted in order:
//!
//! 1. A reset reproducing the computed effects of Tailwind v4 preflight for
//!    the elements the site renders — the reference's baseline is
//!    utilities-over-preflight, so base parity is a hard prerequisite for
//!    every component comparison.
//! 2. The reference's own `@layer base` from `app.css`, verbatim semantics:
//!    theme background/text wiring, body typography, heading scale, and the
//!    `.mono`/`.num` typographic helpers.
//!
//! The motion layer (`live-pulse`, `skel-shimmer`, `fade-slide-in`,
//! `chart-draw`, `bar-grow`, `ring-anim`, `.tbl` row hover) ports with the
//! first component that references each class — the stylesheet gate fails
//! unreferenced rules, so they cannot land speculatively.

/// Preflight-equivalent reset.
const RESET: &str = concat!(
    "*,::before,::after{box-sizing:border-box;border:0 solid;margin:0;padding:0}",
    "::placeholder{opacity:1}",
    "::-webkit-inner-spin-button,::-webkit-outer-spin-button{height:auto}",
    "[type=search]::-webkit-search-decoration{display:none}",
    "::-webkit-date-and-time-value{min-height:1lh}",
    "[hidden]:where(:not([hidden=\"until-found\"])){display:none!important}",
    "html{line-height:1.5;-webkit-text-size-adjust:100%;tab-size:4;",
    "font-family:var(--font-sans);font-feature-settings:normal;",
    "font-variation-settings:normal;-webkit-tap-highlight-color:transparent}",
    "body{line-height:inherit}",
    "h1,h2,h3,h4,h5,h6{font-size:inherit;font-weight:inherit}",
    "a{color:inherit;-webkit-text-decoration:inherit;text-decoration:inherit}",
    "b,strong{font-weight:bolder}",
    "ol,ul,menu{list-style:none}",
    "code,kbd,samp,pre{font-family:var(--font-mono);font-feature-settings:normal;",
    "font-variation-settings:normal;font-size:1em}",
    "small{font-size:80%}",
    "button,input,optgroup,select,textarea{font:inherit;",
    "font-feature-settings:inherit;font-variation-settings:inherit;",
    "letter-spacing:inherit;color:inherit;border-radius:0;",
    "background-color:transparent;opacity:1}",
    "table{text-indent:0;border-color:inherit;border-collapse:collapse}",
    "button,[type=button],[type=reset],[type=submit]{appearance:button}",
    "img,svg,video,canvas{display:block;vertical-align:middle}",
    "img,video{max-width:100%;height:auto}",
);

/// The reference's base layer (`app.css` `@layer base`), same rules.
const SITE_BASE: &str = concat!(
    "html,body{background:var(--color-bg);color:var(--color-text)}",
    "body{font-family:var(--font-sans);font-feature-settings:\"ss01\",\"cv11\";",
    "letter-spacing:-0.005em;line-height:1.55;font-weight:400;",
    "-webkit-font-smoothing:antialiased}",
    "h1,h2,h3,h4{font-weight:600;letter-spacing:-0.02em;line-height:1.15;margin:0}",
    ".mono{font-family:var(--font-mono);font-feature-settings:\"ss01\"}",
    ".num{font-variant-numeric:tabular-nums;font-feature-settings:\"tnum\",\"ss01\"}",
);

pub fn css() -> String {
    format!("{RESET}{SITE_BASE}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_precedes_site_base() {
        let css = css();
        let reset = css.find("box-sizing:border-box").expect("reset");
        let site = css.find("line-height:1.55").expect("site base");
        assert!(reset < site, "site base must be able to override the reset");
    }

    #[test]
    fn base_references_tokens_not_literals() {
        let css = css();
        assert!(css.contains("var(--color-bg)"));
        assert!(css.contains("var(--font-sans)"));
        assert!(!css.contains("oklch("), "base layer must not restate token values");
    }
}
