//! Label — port of `components/ui/label.tsx` (Radix Label root = a styled
//! `<label>`; Radix adds no behavior beyond the element itself). The
//! reference's `peer-disabled:opacity-50` dims the label when a sibling
//! `peer` control (checkbox/switch) is disabled — ported as the
//! `.asy-peer:disabled ~` combinator; checkbox/switch emit `asy-peer`.

use leptos::prelude::*;

pub const LABEL: &str = "asy-label";
/// Marker class the reference calls `peer` — emitted by checkbox/switch
/// roots so sibling labels can react to their disabled state.
pub const PEER: &str = "asy-peer";

#[component]
pub fn Label(
    /// `for` attribute (the reference's `htmlFor`).
    #[prop(optional, into)] r#for: Option<String>,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let mut cls = LABEL.to_owned();
    if let Some(extra) = class {
        cls.push(' ');
        cls.push_str(&extra);
    }
    view! {
        <label class=cls for=r#for>{children()}</label>
    }
}

/// `text-[11.5px] font-medium uppercase tracking-[0.04em]
/// text-[var(--color-text-dim)] peer-disabled:opacity-50`.
pub fn css() -> String {
    format!(
        ".{LABEL}{{font-size:11.5px;font-weight:500;text-transform:uppercase;\
letter-spacing:0.04em;color:var(--color-text-dim)}}\
.{PEER}:disabled~.{LABEL}{{opacity:.5}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        assert!(css.contains(&format!(".{LABEL}{{")));
        assert!(css.contains(&format!(".{PEER}:disabled~.{LABEL}")));
    }
}
