//! Text entry for the register's forms.
//!
//! An `Input` holds `Vec<char>`, so its caret is an index that cannot land
//! inside a multi-byte character. A `char` is a scalar value and not a
//! grapheme cluster, so a combining accent still takes two Backspaces: the
//! granularity the register has always had. Widths are counted in chars, as
//! the action bar counts them, so a double-width glyph drifts the caret by a
//! cell. Growing the buffer can leave a stale copy behind it, which is why the
//! sealed payload is built once and moved into a `Secret`.

use crate::policy::Schema;

/// One line of typed text with a caret.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Input {
    chars: Vec<char>,
    caret: usize,
}

impl Drop for Input {
    fn drop(&mut self) {
        for c in &mut self.chars {
            *c = '\0';
        }
        self.chars.clear();
    }
}

impl Input {
    pub fn from_text(s: &str) -> Self {
        let chars: Vec<char> = s.chars().collect();
        let caret = chars.len();
        Self { chars, caret }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.caret, c);
        self.caret += 1;
    }

    pub fn backspace(&mut self) {
        if self.caret == 0 {
            return;
        }
        self.caret -= 1;
        self.chars.remove(self.caret);
    }

    pub fn delete(&mut self) {
        if self.caret < self.chars.len() {
            self.chars.remove(self.caret);
        }
    }

    pub fn left(&mut self) {
        self.caret = self.caret.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.caret = self.caret.saturating_add(1).min(self.chars.len());
    }

    pub fn home(&mut self) {
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.caret = self.chars.len();
    }

    /// The slice that fits `width` cells with the caret inside it, and the
    /// caret's column within it. `shown` false draws one bullet per character,
    /// as the console's `type="password"` field does.
    pub fn window(&self, width: usize, shown: bool) -> (String, usize) {
        if width == 0 {
            return (String::new(), 0);
        }
        let start = self.caret.saturating_add(1).saturating_sub(width);
        let end = self.chars.len().min(start.saturating_add(width));
        let text = if shown {
            self.chars[start..end].iter().collect()
        } else {
            "\u{2022}".repeat(end.saturating_sub(start))
        };
        (text, self.caret.saturating_sub(start))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldTag {
    Required,
    Optional,
    Plain,
}

impl FieldTag {
    pub fn label(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Plain => "",
        }
    }
}

/// One row of a form: what the console draws as label, tag, env and input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormField {
    pub key: String,
    /// Shown in place of an empty input: what to type, not what is typed.
    pub hint: String,
    pub tag: FieldTag,
    pub env: String,
    pub secret: bool,
    pub input: Input,
    /// Reveal is per field and does not outlive the form.
    pub shown: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Form {
    /// Schema name for a provider form; `None` for the ad-hoc form.
    pub provider: Option<String>,
    /// The modal's title.
    pub title: String,
    /// Index 0 is always the name row; 1.. are the schema fields, in order.
    pub fields: Vec<FormField>,
    pub focus: usize,
    /// Why the last commit did not save. Cleared by the next edit.
    pub error: Option<String>,
    /// The entry this form edits. A changed name is a move, not a copy.
    pub editing: Option<String>,
}

impl Form {
    /// `[a] add`: one name, one opaque value.
    pub fn single(name: &str, value: &str) -> Self {
        Self {
            provider: None,
            title: "add".to_string(),
            fields: vec![
                name_field(name, "path/name"),
                FormField {
                    key: "value".to_string(),
                    hint: String::new(),
                    tag: FieldTag::Plain,
                    env: String::new(),
                    secret: true,
                    input: Input::from_text(value),
                    shown: false,
                },
            ],
            focus: 0,
            error: None,
            editing: None,
        }
    }

    /// `[p] provider`: the name, then one row per schema field, in order.
    pub fn schema(schema: &Schema, name: &str, values: &[(String, String)]) -> Self {
        let mut fields = Vec::with_capacity(schema.fields.len() + 1);
        let hint = format!("prod/{}", schema.name);
        fields.push(name_field(name, &hint));
        for f in &schema.fields {
            let v = values
                .iter()
                .find(|(k, _)| *k == f.key)
                .map(|(_, v)| v.as_str())
                .unwrap_or_default();
            fields.push(FormField {
                key: f.key.clone(),
                hint: String::new(),
                tag: if f.optional {
                    FieldTag::Optional
                } else {
                    FieldTag::Required
                },
                env: f.env.clone(),
                secret: f.secret,
                input: Input::from_text(v),
                shown: false,
            });
        }
        Self {
            provider: Some(schema.name.clone()),
            title: schema.title.clone(),
            fields,
            focus: 0,
            error: None,
            editing: None,
        }
    }

    pub fn name_text(&self) -> String {
        self.fields
            .first()
            .map(|f| f.input.text())
            .unwrap_or_default()
    }

    /// Schema key to typed text, for `build_payload`. The name row is not one.
    pub fn value_pairs(&self) -> Vec<(String, String)> {
        self.fields
            .iter()
            .skip(1)
            .map(|f| (f.key.clone(), f.input.text()))
            .collect()
    }

    pub fn focus_next(&mut self) {
        if self.fields.is_empty() {
            return;
        }
        self.focus = (self.focus + 1) % self.fields.len();
    }

    pub fn focus_prev(&mut self) {
        if self.fields.is_empty() {
            return;
        }
        self.focus = (self.focus + self.fields.len() - 1) % self.fields.len();
    }

    pub fn focus_at(&mut self, i: usize) {
        if i < self.fields.len() {
            self.focus = i;
        }
    }

    pub fn focus_first(&mut self) {
        self.focus = 0;
    }

    pub fn focus_last(&mut self) {
        self.focus = self.fields.len().saturating_sub(1);
    }

    /// Reveal only means something on a secret field.
    pub fn toggle_shown_at(&mut self, i: usize) {
        if let Some(f) = self.fields.get_mut(i) {
            if f.secret {
                f.shown = !f.shown;
            }
        }
    }

    pub fn toggle_shown(&mut self) {
        self.toggle_shown_at(self.focus);
    }

    pub fn insert(&mut self, c: char) {
        self.error = None;
        if let Some(f) = self.fields.get_mut(self.focus) {
            f.input.insert(c);
        }
    }

    pub fn backspace(&mut self) {
        self.error = None;
        if let Some(f) = self.fields.get_mut(self.focus) {
            f.input.backspace();
        }
    }

    pub fn delete(&mut self) {
        self.error = None;
        if let Some(f) = self.fields.get_mut(self.focus) {
            f.input.delete();
        }
    }

    pub fn caret_left(&mut self) {
        if let Some(f) = self.fields.get_mut(self.focus) {
            f.input.left();
        }
    }

    pub fn caret_right(&mut self) {
        if let Some(f) = self.fields.get_mut(self.focus) {
            f.input.right();
        }
    }

    pub fn caret_home(&mut self) {
        if let Some(f) = self.fields.get_mut(self.focus) {
            f.input.home();
        }
    }

    pub fn caret_end(&mut self) {
        if let Some(f) = self.fields.get_mut(self.focus) {
            f.input.end();
        }
    }

    /// The widest key and the widest env, for the column plan.
    pub fn widths(&self) -> (u16, u16) {
        let key = self
            .fields
            .iter()
            .map(|f| f.key.chars().count())
            .max()
            .unwrap_or(0);
        let env = self
            .fields
            .iter()
            .map(|f| f.env.chars().count())
            .max()
            .unwrap_or(0);
        (
            u16::try_from(key).unwrap_or(u16::MAX),
            u16::try_from(env).unwrap_or(u16::MAX),
        )
    }
}

fn name_field(name: &str, hint: &str) -> FormField {
    FormField {
        key: "name".to_string(),
        hint: hint.to_string(),
        tag: FieldTag::Plain,
        env: String::new(),
        secret: false,
        input: Input::from_text(name),
        shown: true,
    }
}
