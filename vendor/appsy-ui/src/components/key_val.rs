//! KeyVal — port of `components/dashboard/key-val.tsx`: label/value row for
//! the tunnel-detail Configuration panel. `mono` swaps the value typography;
//! `copy`/`editable` render the reference's icon buttons exactly as it does —
//! visual affordances with `aria-label` and hover tint, no click wiring in
//! the component (none upstream either).

use crate::icons::{Icon, RI_FILE_COPY_LINE, RI_PENCIL_LINE};
use leptos::prelude::*;

pub const KEY_VAL: &str = "asy-key-val";
pub const KEY_VAL_LABEL: &str = "asy-key-val__label";
pub const KEY_VAL_RIGHT: &str = "asy-key-val__right";
pub const KEY_VAL_VALUE: &str = "asy-key-val__value";
pub const KEY_VAL_VALUE_MONO: &str = "asy-key-val__value--mono";
pub const KEY_VAL_BTN: &str = "asy-key-val__btn";
pub const KEY_VAL_GLYPH: &str = "asy-key-val__glyph";

#[component]
pub fn KeyVal(
    #[prop(into)] label: String,
    #[prop(into)] value: ViewFnOnce,
    #[prop(optional)] mono: bool,
    #[prop(optional)] copy: bool,
    #[prop(optional)] editable: bool,
) -> impl IntoView {
    let value_class = if mono {
        format!("mono {KEY_VAL_VALUE_MONO}")
    } else {
        KEY_VAL_VALUE.to_owned()
    };
    view! {
        <div class=KEY_VAL>
            <span class=KEY_VAL_LABEL>{label}</span>
            <div class=KEY_VAL_RIGHT>
                <span class=value_class>{value.run()}</span>
                {copy.then(|| {
                    view! {
                        <button type="button" aria-label="copy" class=KEY_VAL_BTN>
                            <Icon d=RI_FILE_COPY_LINE class=KEY_VAL_GLYPH />
                        </button>
                    }
                })}
                {editable.then(|| {
                    view! {
                        <button type="button" aria-label="edit" class=KEY_VAL_BTN>
                            <Icon d=RI_PENCIL_LINE class=KEY_VAL_GLYPH />
                        </button>
                    }
                })}
            </div>
        </div>
    }
}

/// Row `flex items-center justify-between gap-3`; label `w-[130px] shrink-0
/// text-[12px] text-muted`; right `flex items-center justify-end gap-2
/// flex-1`; value `text-[13px]` or `mono text-[12.5px]`; buttons `inline-flex
/// size-5.5 items-center justify-center rounded text-dim
/// hover:bg-surface-2` (hover gated like Tailwind's variant) with `size-3`
/// glyphs.
pub fn css() -> String {
    format!(
        ".{KEY_VAL}{{display:flex;align-items:center;justify-content:space-between;gap:.75rem}}\
.{KEY_VAL_LABEL}{{width:130px;flex-shrink:0;font-size:12px;color:var(--color-text-muted)}}\
.{KEY_VAL_RIGHT}{{display:flex;min-width:0;align-items:center;justify-content:flex-end;gap:.5rem;\
flex:1 1 0%}}\
.{KEY_VAL_VALUE}{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;font-size:13px}}\
.{KEY_VAL_VALUE_MONO}{{min-width:0;overflow:hidden;text-overflow:ellipsis;\
white-space:nowrap;font-size:12.5px}}\
.{KEY_VAL_BTN}{{display:inline-flex;width:1.375rem;height:1.375rem;align-items:center;\
justify-content:center;border-radius:.25rem;color:var(--color-text-dim)}}\
@media (hover: hover){{.{KEY_VAL_BTN}:hover{{background-color:var(--color-surface-2)}}}}\
.{KEY_VAL_GLYPH}{{width:.75rem;height:.75rem}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_const_has_a_rule() {
        let css = css();
        for class in [
            KEY_VAL,
            KEY_VAL_LABEL,
            KEY_VAL_RIGHT,
            KEY_VAL_VALUE,
            KEY_VAL_VALUE_MONO,
            KEY_VAL_BTN,
            KEY_VAL_GLYPH,
        ] {
            assert!(css.contains(&format!(".{class}{{")), "no rule for .{class}");
        }
    }

    #[test]
    fn button_hover_is_gated_like_tailwind() {
        assert!(css().contains(&format!(
            "@media (hover: hover){{.{KEY_VAL_BTN}:hover{{background-color:var(--color-surface-2)}}}}"
        )));
    }
}
