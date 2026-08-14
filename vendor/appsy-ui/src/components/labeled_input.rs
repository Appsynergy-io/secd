//! LabeledInput — port of `auth/labeled-input.tsx`: the stacked Label +
//! Input pair every auth form uses, with the optional `after` slot for the
//! password-eye and similar trailing affordances. Input attributes forward
//! through exactly like the reference's prop spread.

use crate::components::input::Input;
use crate::components::label::Label;
use leptos::prelude::*;

pub const LINPUT: &str = "asy-linput";
pub const LINPUT_FIELD: &str = "asy-linput__field";
pub const LINPUT_BOX: &str = "asy-linput__box";
pub const LINPUT_BOX_AFTER: &str = "asy-linput__box--after";
pub const LINPUT_AFTER: &str = "asy-linput__after";

#[component]
pub fn LabeledInput(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    /// Mono value text (server names, tokens).
    #[prop(optional)]
    mono: bool,
    /// Trailing affordance rendered over the input's right edge.
    #[prop(optional)]
    after: Option<ViewFn>,
    #[prop(optional, into, default = "text".into())] r#type: String,
    #[prop(optional, into)] placeholder: Option<String>,
    /// Static or reactive; see [`Input`]'s `value` (approved API change,
    /// 2026-08-07; frozen again).
    #[prop(optional, into)]
    value: MaybeProp<String>,
    #[prop(optional)] disabled: bool,
    /// `autocomplete` attribute — the reference forwards it via prop spread.
    #[prop(optional, into)]
    autocomplete: Option<String>,
    /// `required` attribute — the reference forwards it via prop spread.
    #[prop(optional)]
    required: bool,
    #[prop(optional, into)] class: Option<String>,
    /// Handle to the underlying `<input>` for focus management.
    #[prop(optional)]
    node_ref: NodeRef<leptos::html::Input>,
) -> impl IntoView {
    let mut input_cls = LINPUT_FIELD.to_owned();
    if mono {
        input_cls.push_str(" mono");
    }
    if after.is_some() {
        input_cls.push(' ');
        input_cls.push_str(LINPUT_BOX_AFTER);
    }
    if let Some(extra) = class {
        input_cls.push(' ');
        input_cls.push_str(&extra);
    }
    view! {
        <div class=LINPUT>
            <Label r#for=id.clone()>{label}</Label>
            <div class=LINPUT_BOX>
                // attr: spread onto Input's root <input> is the port of the
                // reference's {...props} forwarding — Input's own API stays
                // frozen.
                <Input
                    id=id
                    r#type=r#type
                    value=value
                    disabled=disabled
                    class=input_cls
                    node_ref=node_ref
                    attr:placeholder=placeholder
                    attr:autocomplete=autocomplete
                    attr:required=required.then_some("")
                />
                {after.map(|a| view! { <span class=LINPUT_AFTER>{a.run()}</span> })}
            </div>
        </div>
    }
}

pub fn css() -> String {
    format!(
        concat!(
            ".{root}{{display:flex;flex-direction:column;gap:.375rem}}",
            ".{field}{{height:38px}}",
            ".{after_pad}{{padding-right:2.25rem}}",
            ".{boxc}{{position:relative}}",
            ".{after}{{position:absolute;right:.625rem;top:50%;translate:0 -50%;",
            "color:var(--color-text-dim)}}",
        ),
        root = LINPUT,
        field = LINPUT_FIELD,
        after_pad = LINPUT_BOX_AFTER,
        boxc = LINPUT_BOX,
        after = LINPUT_AFTER,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_covers_every_class() {
        let css = css();
        for class in [LINPUT, LINPUT_FIELD, LINPUT_BOX, LINPUT_BOX_AFTER, LINPUT_AFTER] {
            assert!(css.contains(&format!(".{class}{{")), "missing rule for {class}");
        }
    }
}
