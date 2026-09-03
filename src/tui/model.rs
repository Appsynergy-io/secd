use std::collections::{BTreeMap, HashMap, VecDeque};

use secd_core::{build_payload, check_name, Secret};
use serde_json::Value;

use crate::login::{self, Unlocked};
use crate::policy::{self, Schema};

use super::form::Form;
use super::view::{hit_key, spot_at, Action, ActionHit, Spot, SpotHit};

/// Seven buttons, 75 of 80 cells. `[q] quit` is last so a narrow terminal
/// drops the button whose keyboard equivalent is most redundant.
const IDLE_ACTIONS: &[Action] = &[
    Action {
        key: 'a',
        label: "add",
    },
    Action {
        key: 'p',
        label: "provider",
    },
    Action {
        key: 'e',
        label: "edit",
    },
    Action {
        key: 'r',
        label: "reveal",
    },
    Action {
        key: 'c',
        label: "copy",
    },
    Action {
        key: 'd',
        label: "delete",
    },
    Action {
        key: 'q',
        label: "quit",
    },
];

/// How far a page key moves a list.
const PAGE: usize = 10;

/// `[p] provider` step 1. The schema list lives on the `Model`, so cloning a
/// `Mode` stays cheap.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Picker {
    pub selected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Modal {
    /// `[a] add`, and `[e] edit` on an entry that is not a bundle.
    Add {
        form: Form,
    },
    /// `[p] provider` step 1: which schema.
    Pick {
        pick: Picker,
    },
    /// `[p] provider` step 2, and `[e] edit` on a bundle.
    Provider {
        form: Form,
    },
    Delete {
        name: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Mode {
    Idle,
    Modal(Modal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    Key(char),
    Esc,
    Enter,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Backspace,
    Delete,
    Tab,
    BackTab,
    PageUp,
    PageDown,
    /// Ctrl-R, and the `[r] reveal` button.
    Reveal,
    Click {
        column: u16,
        row: u16,
    },
    Quit,
    Tick,
    Resize,
}

/// One line of the detail pane. `key` is empty for a plain single value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailRow {
    pub key: String,
    pub env: String,
    pub secret: bool,
    pub value: String,
}

pub struct Model {
    mode: Mode,
    quit: bool,
    names: Vec<String>,
    selected: usize,
    values: HashMap<String, Secret>,
    meta: HashMap<String, Value>,
    detail: Vec<DetailRow>,
    /// Provider line above the detail rows; empty when the entry is not a bundle.
    detail_title: String,
    /// Reveal is for the selected entry only, and does not survive a move.
    reveal_all: bool,
    activity: VecDeque<String>,
    hits: Vec<ActionHit>,
    spots: Vec<SpotHit>,
    schemas: Vec<Schema>,
    token: Option<String>,
    dek: Option<Secret>,
    /// The vault as this register last saw it, name -> ciphertext.
    before: BTreeMap<String, String>,
    /// Why a save would lose data. Set means every save is refused.
    save_blocked: Option<String>,
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Model {
    pub fn new() -> Self {
        Self {
            mode: Mode::Idle,
            quit: false,
            names: Vec::new(),
            selected: 0,
            values: HashMap::new(),
            meta: HashMap::new(),
            detail: Vec::new(),
            detail_title: String::new(),
            reveal_all: false,
            activity: VecDeque::new(),
            hits: Vec::new(),
            spots: Vec::new(),
            schemas: policy::builtin_schemas(),
            token: None,
            dek: None,
            before: BTreeMap::new(),
            save_blocked: None,
        }
    }

    pub fn from_unlocked(unlocked: Unlocked) -> Self {
        let Unlocked { token, dek } = unlocked;
        let mut model = Self::new();
        model.token = Some(token);
        model.dek = Some(dek);
        // A save replaces the whole vault, so a register built from a load that
        // dropped entries -- or from no load at all -- would delete them.
        model.load_register();
        model.note(format!("loaded {}", model.names.len()));
        if let Some(why) = model.save_blocked.clone() {
            model.note(format!("saves refused: {why}"));
        }
        // One round trip at start, where the register already blocks. Doing it
        // on the keypress would freeze a 250ms poll loop mid-interaction.
        model.load_schemas();
        model.sync_detail();
        model
    }

    pub fn quit(&self) -> bool {
        self.quit
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.mode, Mode::Idle)
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn schemas(&self) -> &[Schema] {
        &self.schemas
    }

    pub fn detail_rows(&self) -> &[DetailRow] {
        &self.detail
    }

    pub fn detail_title(&self) -> &str {
        &self.detail_title
    }

    pub fn revealed(&self) -> bool {
        self.reveal_all
    }

    pub fn activity_lines(&self) -> Vec<String> {
        self.activity.iter().cloned().collect()
    }

    pub fn title(&self) -> String {
        self.selected_name()
            .map(ToString::to_string)
            .unwrap_or_else(|| "register".to_string())
    }

    pub fn actions(&self) -> &[Action] {
        match self.mode {
            Mode::Idle => IDLE_ACTIONS,
            Mode::Modal(_) => &[],
        }
    }

    pub fn set_hits(&mut self, hits: Vec<ActionHit>) {
        self.hits = hits;
    }

    pub fn set_spots(&mut self, spots: Vec<SpotHit>) {
        self.spots = spots;
    }

    pub fn handle(&mut self, ev: Event) {
        match ev {
            Event::Click { column, row } => self.on_click(column, row),
            Event::Quit => self.quit = true,
            Event::Esc => self.on_esc(),
            Event::Enter => self.on_enter(),
            Event::Up => self.on_up(),
            Event::Down => self.on_down(),
            Event::Left => self.on_form(Form::caret_left),
            Event::Right => self.on_form(Form::caret_right),
            Event::Home => self.on_home(),
            Event::End => self.on_end(),
            Event::Backspace => self.on_form(Form::backspace),
            Event::Delete => self.on_form(Form::delete),
            Event::Tab => self.on_form(Form::focus_next),
            Event::BackTab => self.on_form(Form::focus_prev),
            Event::PageUp => self.on_page(false),
            Event::PageDown => self.on_page(true),
            Event::Reveal => self.on_reveal(),
            Event::Key(c) => self.on_char(c),
            Event::Tick | Event::Resize => {}
        }
    }

    fn form_mut(&mut self) -> Option<&mut Form> {
        match &mut self.mode {
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => Some(form),
            _ => None,
        }
    }

    pub fn form(&self) -> Option<&Form> {
        match &self.mode {
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => Some(form),
            _ => None,
        }
    }

    pub fn picker(&self) -> Option<&Picker> {
        match &self.mode {
            Mode::Modal(Modal::Pick { pick }) => Some(pick),
            _ => None,
        }
    }

    fn on_form(&mut self, f: fn(&mut Form)) {
        if let Some(form) = self.form_mut() {
            f(form);
        }
    }

    fn on_click(&mut self, column: u16, row: u16) {
        if let Some(spot) = spot_at(&self.spots, column, row) {
            self.on_spot(spot);
            return;
        }
        // The bar is the only char-keyed surface, and it is live only when a
        // letter is a command rather than text.
        if self.is_idle() {
            if let Some(k) = hit_key(&self.hits, column, row) {
                self.handle(Event::Key(k));
            }
        }
    }

    fn on_spot(&mut self, spot: Spot) {
        match spot {
            Spot::Row(i) => {
                if self.is_idle() && i < self.names.len() {
                    self.selected = i;
                    self.sync_detail();
                }
            }
            Spot::Field(i) => {
                if let Some(form) = self.form_mut() {
                    form.focus_at(i);
                }
            }
            Spot::Reveal(i) => {
                if let Some(form) = self.form_mut() {
                    form.focus_at(i);
                    form.toggle_shown_at(i);
                }
            }
            Spot::Choice(i) => {
                if let Mode::Modal(Modal::Pick { pick }) = &mut self.mode {
                    pick.selected = i;
                }
                self.open_schema_form(i);
            }
            Spot::Save => self.on_enter(),
            Spot::Cancel => self.mode = Mode::Idle,
        }
    }

    fn on_esc(&mut self) {
        match self.mode {
            Mode::Idle => self.quit = true,
            Mode::Modal(_) => self.mode = Mode::Idle,
        }
    }

    fn on_up(&mut self) {
        if self.is_idle() {
            self.select_prev();
            return;
        }
        match &mut self.mode {
            Mode::Modal(Modal::Pick { pick }) => {
                pick.selected = pick.selected.saturating_sub(1);
            }
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => form.focus_prev(),
            _ => {}
        }
    }

    fn on_down(&mut self) {
        if self.is_idle() {
            self.select_next();
            return;
        }
        let last = self.schemas.len().saturating_sub(1);
        match &mut self.mode {
            Mode::Modal(Modal::Pick { pick }) => {
                pick.selected = pick.selected.saturating_add(1).min(last);
            }
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => form.focus_next(),
            _ => {}
        }
    }

    fn on_home(&mut self) {
        if self.is_idle() {
            self.selected = 0;
            self.sync_detail();
            return;
        }
        match &mut self.mode {
            Mode::Modal(Modal::Pick { pick }) => pick.selected = 0,
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => form.caret_home(),
            _ => {}
        }
    }

    fn on_end(&mut self) {
        if self.is_idle() {
            self.selected = self.names.len().saturating_sub(1);
            self.sync_detail();
            return;
        }
        let last = self.schemas.len().saturating_sub(1);
        match &mut self.mode {
            Mode::Modal(Modal::Pick { pick }) => pick.selected = last,
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => form.caret_end(),
            _ => {}
        }
    }

    fn on_page(&mut self, down: bool) {
        if self.is_idle() {
            let last = self.names.len().saturating_sub(1);
            self.selected = if down {
                self.selected.saturating_add(PAGE).min(last)
            } else {
                self.selected.saturating_sub(PAGE)
            };
            self.sync_detail();
            return;
        }
        let last = self.schemas.len().saturating_sub(1);
        match &mut self.mode {
            Mode::Modal(Modal::Pick { pick }) => {
                pick.selected = if down {
                    pick.selected.saturating_add(PAGE).min(last)
                } else {
                    pick.selected.saturating_sub(PAGE)
                };
            }
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => {
                if down {
                    form.focus_last();
                } else {
                    form.focus_first();
                }
            }
            _ => {}
        }
    }

    fn on_reveal(&mut self) {
        if self.is_idle() {
            self.reveal_all = !self.reveal_all;
            return;
        }
        if let Some(form) = self.form_mut() {
            form.toggle_shown();
        }
    }

    fn on_char(&mut self, c: char) {
        match &mut self.mode {
            Mode::Idle => match c {
                'a' | 'A' => self.begin_add(),
                'p' | 'P' => self.begin_pick(),
                'e' | 'E' => self.begin_edit(),
                'r' | 'R' => self.reveal_all = !self.reveal_all,
                'c' | 'C' => self.copy_selected(),
                'd' | 'D' => self.begin_delete(),
                'q' | 'Q' => self.quit = true,
                'j' => self.select_next(),
                'k' => self.select_prev(),
                _ => {}
            },
            Mode::Modal(Modal::Pick { .. }) => self.pick_type_ahead(c),
            Mode::Modal(Modal::Add { form } | Modal::Provider { form }) => form.insert(c),
            Mode::Modal(Modal::Delete { .. }) => {}
        }
    }

    fn pick_type_ahead(&mut self, c: char) {
        let want = c.to_ascii_lowercase();
        let found = self.schemas.iter().position(|s| {
            s.name
                .chars()
                .next()
                .is_some_and(|f| f.to_ascii_lowercase() == want)
        });
        let Some(i) = found else {
            return;
        };
        if let Mode::Modal(Modal::Pick { pick }) = &mut self.mode {
            pick.selected = i;
        }
    }

    fn on_enter(&mut self) {
        // Take the form rather than clone it: a commit must not leave a second
        // copy of a value behind.
        let mode = std::mem::replace(&mut self.mode, Mode::Idle);
        match mode {
            Mode::Idle => {}
            Mode::Modal(Modal::Add { form }) => self.commit_single(form),
            Mode::Modal(Modal::Provider { form }) => self.commit_provider(form),
            Mode::Modal(Modal::Pick { pick }) => self.open_schema_form(pick.selected),
            Mode::Modal(Modal::Delete { name }) => self.commit_delete(&name),
        }
    }

    fn begin_add(&mut self) {
        if self.refuse_if_blocked() {
            return;
        }
        self.mode = Mode::Modal(Modal::Add {
            form: Form::single("", ""),
        });
    }

    fn begin_pick(&mut self) {
        if self.refuse_if_blocked() {
            return;
        }
        if self.schemas.is_empty() {
            self.note("no provider schemas");
            return;
        }
        self.mode = Mode::Modal(Modal::Pick {
            pick: Picker::default(),
        });
    }

    fn begin_edit(&mut self) {
        if self.refuse_if_blocked() {
            return;
        }
        let Some(name) = self.selected_name().map(str::to_string) else {
            return;
        };
        let Some(text) = self.plaintext(&name) else {
            self.note("binary value: edit is unavailable");
            return;
        };
        let mut form = match self.bundle_of(&name, &text) {
            Some((schema, pairs)) => Form::schema(&schema, &name, &pairs),
            None => Form::single(&name, &text),
        };
        form.title = "edit".to_string();
        form.editing = Some(name);
        self.mode = Mode::Modal(if form.provider.is_some() {
            Modal::Provider { form }
        } else {
            Modal::Add { form }
        });
    }

    fn begin_delete(&mut self) {
        if self.refuse_if_blocked() {
            return;
        }
        let Some(name) = self.selected_name().map(str::to_string) else {
            return;
        };
        self.mode = Mode::Modal(Modal::Delete { name });
    }

    fn open_schema_form(&mut self, i: usize) {
        let Some(schema) = self.schemas.get(i).cloned() else {
            return;
        };
        self.mode = Mode::Modal(Modal::Provider {
            form: Form::schema(&schema, "", &[]),
        });
    }

    fn commit_single(&mut self, form: Form) {
        let name = form.name_text().trim().to_string();
        if let Err(e) = check_name(&name) {
            return self.form_refuse(Modal::Add { form }, format!("bad name: {e}"));
        }
        if let Some(why) = self.rename_collision(&form, &name) {
            return self.form_refuse(Modal::Add { form }, why);
        }
        let text = form
            .value_pairs()
            .into_iter()
            .next()
            .map(|(_, v)| v)
            .unwrap_or_default();
        let drop_old = form.editing.clone().filter(|o| *o != name);
        let value = Secret::new(text.into_bytes());
        self.commit_entry(name, value, Value::Object(Default::default()), drop_old);
    }

    fn commit_provider(&mut self, form: Form) {
        let name = form.name_text().trim().to_string();
        if let Err(e) = check_name(&name) {
            return self.form_refuse(Modal::Provider { form }, format!("bad name: {e}"));
        }
        let found = form
            .provider
            .as_deref()
            .and_then(|p| self.schemas.iter().find(|s| s.name == p))
            .cloned();
        let Some(schema) = found else {
            return self.form_refuse(Modal::Provider { form }, "unknown provider".to_string());
        };
        let Some(pairs) = build_payload(&schema.fields, &form.value_pairs()) else {
            return self.form_refuse(
                Modal::Provider { form },
                "a required field is empty".to_string(),
            );
        };
        if let Some(why) = self.rename_collision(&form, &name) {
            return self.form_refuse(Modal::Provider { form }, why);
        }
        let keys: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
        let meta = policy::provider_meta(&schema.name, &keys);
        let drop_old = form.editing.clone().filter(|o| *o != name);
        let value = Secret::new(policy::payload_json(&pairs).into_bytes());
        self.commit_entry(name, value, meta, drop_old);
    }

    /// An edit may rename, but not onto a name that is already taken: that
    /// would silently replace a second entry.
    fn rename_collision(&self, form: &Form, name: &str) -> Option<String> {
        let old = form.editing.as_deref()?;
        (old != name && self.names.iter().any(|n| n == name))
            .then(|| format!("{name} already exists"))
    }

    /// Say why next to the field, and leave the modal open.
    fn form_refuse(&mut self, modal: Modal, why: String) {
        self.note(why.clone());
        let mut modal = modal;
        if let Modal::Add { form } | Modal::Provider { form } = &mut modal {
            form.error = Some(why);
        }
        self.mode = Mode::Modal(modal);
    }

    /// Save one entry over the whole register. `drop_old` is set only by an
    /// edit that renamed: the move and the write are one PUT, so the entry is
    /// never duplicated and never briefly absent.
    fn commit_entry(&mut self, name: String, value: Secret, meta: Value, drop_old: Option<String>) {
        if self.refuse_if_blocked() {
            return;
        }
        let empty = Value::Object(Default::default());
        let mut names: Vec<String> = self
            .names
            .iter()
            .filter(|n| drop_old.as_deref() != Some(n.as_str()))
            .cloned()
            .collect();
        if !names.iter().any(|n| n == &name) {
            names.push(name.clone());
            names.sort();
        }
        let saved = {
            let rows: Vec<policy::Row<'_>> = names
                .iter()
                .filter_map(|n| {
                    if n == &name {
                        Some((n.as_str(), &value, &meta))
                    } else {
                        let v = self.values.get(n)?;
                        Some((n.as_str(), v, self.meta.get(n).unwrap_or(&empty)))
                    }
                })
                .collect();
            self.save_rows(&rows)
        };
        match saved {
            Some(Ok(written)) => {
                self.before = written;
                self.names = names;
                if let Some(old) = &drop_old {
                    self.values.remove(old);
                    self.meta.remove(old);
                }
                if let Some(i) = self.names.iter().position(|n| n == &name) {
                    self.selected = i;
                }
                self.values.insert(name.clone(), value);
                // The caller's meta, always: an overwrite that kept the old one
                // would leave `secd info` describing an entry that is gone.
                self.meta.insert(name, meta);
                self.sync_detail();
                self.note("saved");
            }
            Some(Err(e)) => {
                self.note(format!("save failed: {e}"));
                self.reload_from_vault();
            }
            None => {}
        }
    }

    fn commit_delete(&mut self, name: &str) {
        self.mode = Mode::Idle;
        if self.refuse_if_blocked() {
            return;
        }
        let empty = Value::Object(Default::default());
        let names: Vec<String> = self.names.iter().filter(|n| *n != name).cloned().collect();
        let saved = {
            let rows: Vec<policy::Row<'_>> = names
                .iter()
                .filter_map(|n| {
                    let v = self.values.get(n)?;
                    Some((n.as_str(), v, self.meta.get(n).unwrap_or(&empty)))
                })
                .collect();
            self.save_rows(&rows)
        };
        match saved {
            Some(Ok(written)) => {
                self.before = written;
                self.names = names;
                self.values.remove(name);
                self.meta.remove(name);
                if self.selected >= self.names.len() {
                    self.selected = self.names.len().saturating_sub(1);
                }
                self.sync_detail();
                self.note("deleted");
            }
            Some(Err(e)) => {
                self.note(format!("save failed: {e}"));
                self.reload_from_vault();
            }
            None => {}
        }
    }

    fn copy_selected(&mut self) {
        let Some(name) = self.selected_name().map(str::to_string) else {
            return;
        };
        let Some(secret) = self.values.get(&name) else {
            return;
        };
        login::clipboard_set(secret.as_bytes());
        self.note("copied");
    }

    fn select_next(&mut self) {
        if self.names.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.names.len() - 1);
        self.sync_detail();
    }

    fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.sync_detail();
    }

    fn selected_name(&self) -> Option<&str> {
        self.names.get(self.selected).map(String::as_str)
    }

    /// The selected entry's plaintext, when it is text.
    fn plaintext(&self, name: &str) -> Option<String> {
        let secret = self.values.get(name)?;
        std::str::from_utf8(secret.as_bytes())
            .ok()
            .map(ToString::to_string)
    }

    /// The schema and field values for an entry whose plaintext is a JSON
    /// object of strings. `meta.provider` wins, then `infer` over the keys --
    /// the ladder `policy::resolve_provider` walks, widened to the custom
    /// schemas the register fetched, which that ladder cannot name.
    fn bundle_of(&self, name: &str, text: &str) -> Option<(Schema, Vec<(String, String)>)> {
        let pairs = bundle_pairs(text)?;
        let keys: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
        let want = self
            .meta
            .get(name)
            .and_then(|m| m.get("provider"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| secd_core::infer(&keys).map(ToString::to_string))?;
        let schema = self.schemas.iter().find(|s| s.name == want)?.clone();
        Some((schema, pairs))
    }

    fn sync_detail(&mut self) {
        self.detail.clear();
        self.detail_title.clear();
        // A move must not carry the last entry's reveal onto this one.
        self.reveal_all = false;
        let Some(name) = self.selected_name().map(str::to_string) else {
            return;
        };
        if !self.values.contains_key(&name) {
            return;
        }
        let Some(text) = self.plaintext(&name) else {
            self.detail.push(DetailRow {
                key: String::new(),
                env: String::new(),
                secret: false,
                value: "(binary)".to_string(),
            });
            return;
        };
        if let Some((schema, pairs)) = self.bundle_of(&name, &text) {
            let n = pairs.len();
            let unit = if n == 1 { "field" } else { "fields" };
            self.detail_title = format!("{} · {n} {unit}", schema.title);
            for f in &schema.fields {
                if let Some((_, v)) = pairs.iter().find(|(k, _)| *k == f.key) {
                    self.detail.push(DetailRow {
                        key: f.key.clone(),
                        env: f.env.clone(),
                        secret: f.secret,
                        value: v.clone(),
                    });
                }
            }
            for (k, v) in &pairs {
                if schema.fields.iter().any(|f| f.key == *k) {
                    continue;
                }
                // A key the schema does not know is masked, as the console does.
                self.detail.push(DetailRow {
                    key: k.clone(),
                    env: String::new(),
                    secret: true,
                    value: v.clone(),
                });
            }
            return;
        }
        self.detail.push(DetailRow {
            key: String::new(),
            env: String::new(),
            secret: true,
            value: text,
        });
    }

    fn refuse_if_blocked(&mut self) -> bool {
        let Some(why) = self.save_blocked.clone() else {
            return false;
        };
        self.note(format!("save refused: {why}"));
        true
    }

    /// Prospective rows, not the register: a miss must not leave a ghost name.
    fn save_rows(
        &self,
        rows: &[policy::Row<'_>],
    ) -> Option<anyhow::Result<BTreeMap<String, String>>> {
        let (Some(token), Some(dek)) = (self.token.as_deref(), self.dek.as_ref()) else {
            return None;
        };
        Some(login::save_snapshot(token, dek, rows, &self.before))
    }

    /// Replace the register from a load. The detail pane follows, so it can
    /// never describe an entry the register no longer holds.
    pub fn apply_loaded(&mut self, loaded: policy::VaultLoad) {
        self.save_blocked = loaded.drop_refusal();
        self.before = loaded.before;
        self.names.clear();
        self.values.clear();
        self.meta.clear();
        self.reveal_all = false;
        for policy::Entry { name, value, meta } in loaded.entries {
            self.names.push(name.clone());
            self.values.insert(name.clone(), value);
            self.meta.insert(name, meta);
        }
        self.names.sort();
        if self.selected >= self.names.len() {
            self.selected = self.names.len().saturating_sub(1);
        }
        self.sync_detail();
    }

    fn load_register(&mut self) {
        let (Some(token), Some(dek)) = (self.token.as_deref(), self.dek.as_ref()) else {
            return;
        };
        match policy::load_vault(token, dek) {
            Ok(loaded) => self.apply_loaded(loaded),
            Err(e) => self.save_blocked = Some(format!("vault: {e}")),
        }
    }

    /// Built-ins and custom. A failure is not fatal: the built-ins seeded in
    /// `new` stand, and the reason goes to the log rather than a modal.
    fn load_schemas(&mut self) {
        let Some(token) = self.token.as_deref() else {
            return;
        };
        match policy::fetch_schemas(token) {
            Ok((schemas, dropped)) if !schemas.is_empty() => {
                if dropped > 0 {
                    let total = schemas.len() + dropped;
                    self.note(format!("providers: {dropped} of {total} unreadable"));
                }
                self.schemas = schemas;
            }
            Ok(_) => self.note("providers: empty list, built-ins only"),
            Err(e) => self.note(format!("providers: {e}, built-ins only")),
        }
    }

    fn reload_from_vault(&mut self) {
        self.load_register();
        if let Some(why) = self.save_blocked.clone() {
            self.note(format!("saves refused: {why}"));
        }
        self.load_schemas();
        self.sync_detail();
    }

    fn note(&mut self, msg: impl Into<String>) {
        self.activity.push_back(msg.into());
        while self.activity.len() > 32 {
            self.activity.pop_front();
        }
    }
}

/// An entry's plaintext as key/value pairs, when it is a JSON object whose
/// values are all strings.
fn bundle_pairs(text: &str) -> Option<Vec<(String, String)>> {
    let v: Value = serde_json::from_str(text).ok()?;
    let obj = v.as_object()?;
    if obj.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(obj.len());
    for (k, val) in obj {
        out.push((k.clone(), val.as_str()?.to_string()));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked_register() -> Model {
        let mut m = Model::new();
        m.save_blocked =
            Some("vault: 1 of 2 entries did not decode; a save would delete them".into());
        m.names.push("kv/alpha".into());
        m
    }

    fn claimed_save(lines: &[String]) -> bool {
        lines.iter().any(|l| l == "saved" || l == "deleted")
    }

    fn add_modal(name: &str, value: &str) -> Mode {
        Mode::Modal(Modal::Add {
            form: Form::single(name, value),
        })
    }

    #[test]
    fn blocked_register_does_not_claim_a_save() {
        let mut m = blocked_register();
        m.handle(Event::Key('a'));
        assert!(m.is_idle(), "must not open add");
        assert!(
            m.activity_lines()
                .iter()
                .any(|l| l.starts_with("save refused:")),
            "must note the refusal"
        );
        assert!(!claimed_save(&m.activity_lines()));

        m.handle(Event::Key('p'));
        assert!(m.is_idle(), "must not open the provider picker");

        m.handle(Event::Key('e'));
        assert!(m.is_idle(), "must not open edit");

        m.handle(Event::Key('d'));
        assert!(m.is_idle(), "must not open delete");
        assert!(!claimed_save(&m.activity_lines()));

        m.mode = add_modal("kv/new", "x");
        m.handle(Event::Enter);
        assert!(!claimed_save(&m.activity_lines()));
        assert!(m.is_idle());
        assert_eq!(m.names(), ["kv/alpha"]);

        m.mode = Mode::Modal(Modal::Delete {
            name: "kv/alpha".into(),
        });
        m.handle(Event::Enter);
        assert!(!claimed_save(&m.activity_lines()));
        assert_eq!(m.names(), ["kv/alpha"]);
    }

    #[test]
    fn failed_save_does_not_leave_a_ghost_row() {
        let mut m = Model::new();
        m.mode = add_modal("kv/new", "x");
        m.handle(Event::Enter);
        assert!(m.is_idle());
        assert!(
            m.names().is_empty(),
            "no session: add must not keep the row"
        );
        assert!(!claimed_save(&m.activity_lines()));

        m.names.push("kv/alpha".into());
        m.values
            .insert("kv/alpha".into(), Secret::new(b"x".to_vec()));
        m.mode = Mode::Modal(Modal::Delete {
            name: "kv/alpha".into(),
        });
        m.handle(Event::Enter);
        assert_eq!(
            m.names(),
            ["kv/alpha"],
            "no session: delete must not drop the row"
        );
        assert!(!claimed_save(&m.activity_lines()));
    }

    #[test]
    fn failed_provider_save_does_not_leave_a_ghost_row() {
        let mut m = Model::new();
        let schema = m
            .schemas
            .iter()
            .find(|s| s.name == "github")
            .cloned()
            .expect("github schema");
        let form = Form::schema(&schema, "kv/gh", &[("token".to_string(), "t".to_string())]);
        m.mode = Mode::Modal(Modal::Provider { form });
        m.handle(Event::Enter);
        assert!(m.is_idle(), "no session: the modal still closes");
        assert!(
            m.names().is_empty(),
            "no session: the provider row must not survive"
        );
        assert!(!claimed_save(&m.activity_lines()));
    }

    #[test]
    fn a_required_field_holds_the_modal_open() {
        let mut m = Model::new();
        let schema = m
            .schemas
            .iter()
            .find(|s| s.name == "github")
            .cloned()
            .expect("github schema");
        m.mode = Mode::Modal(Modal::Provider {
            form: Form::schema(&schema, "kv/gh", &[]),
        });
        m.handle(Event::Enter);
        let form = m.form().expect("modal stays open");
        assert_eq!(form.error.as_deref(), Some("a required field is empty"));
        assert!(m.names().is_empty());
    }

    #[test]
    fn a_bad_name_reports_what_is_wrong() {
        let mut m = Model::new();
        m.mode = add_modal("kv/../escape", "x");
        m.handle(Event::Enter);
        let form = m.form().expect("modal stays open");
        let err = form.error.clone().expect("an error");
        assert!(err.starts_with("bad name: "), "{err}");
        assert!(
            err.len() > "bad name: ".len(),
            "the reason is carried: {err}"
        );
    }

    #[test]
    fn apply_loaded_replaces_the_register_and_pre_image() {
        let mut m = Model::new();
        m.names.push("kv/ghost".into());
        m.values
            .insert("kv/ghost".into(), Secret::new(b"x".to_vec()));
        m.before.insert("stale".into(), "ct-stale".into());
        m.selected = 3;
        m.reveal_all = true;

        let mut before = BTreeMap::new();
        before.insert("kv/alpha".into(), "ct-alpha".into());
        before.insert("kv/dropped".into(), "ct-dropped".into());
        m.apply_loaded(policy::VaultLoad {
            entries: vec![policy::Entry {
                name: "kv/alpha".into(),
                value: Secret::new(b"x".to_vec()),
                meta: Value::Object(Default::default()),
            }],
            raw: 2,
            body: String::new(),
            before,
        });
        assert_eq!(m.names(), ["kv/alpha"]);
        assert_eq!(m.selected, 0);
        assert!(!m.revealed(), "a reload must not leave a value on screen");
        assert_eq!(
            m.before.get("kv/alpha").map(String::as_str),
            Some("ct-alpha")
        );
        assert_eq!(
            m.before.get("kv/dropped").map(String::as_str),
            Some("ct-dropped")
        );
        assert!(m.save_blocked.is_some());
    }
}
