use std::collections::{BTreeMap, HashMap, VecDeque};

use secd_core::{check_name, Secret};
use serde_json::Value;

use crate::login::{self, Unlocked};
use crate::policy;

use super::view::{hit_key, Action, ActionHit};

const IDLE_ACTIONS: &[Action] = &[
    Action {
        key: 'a',
        label: "add",
    },
    Action {
        key: 'd',
        label: "delete",
    },
    Action {
        key: 'c',
        label: "copy",
    },
    Action {
        key: 'q',
        label: "quit",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddField {
    Name,
    Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Modal {
    Add {
        name: String,
        value: String,
        focus: AddField,
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
    Backspace,
    Tab,
    Click { column: u16, row: u16 },
    Quit,
    Tick,
    Resize,
}

pub struct Model {
    mode: Mode,
    quit: bool,
    names: Vec<String>,
    selected: usize,
    values: HashMap<String, Secret>,
    meta: HashMap<String, Value>,
    detail: String,
    activity: VecDeque<String>,
    hits: Vec<ActionHit>,
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
            detail: String::new(),
            activity: VecDeque::new(),
            hits: Vec::new(),
            token: None,
            dek: None,
            before: BTreeMap::new(),
            save_blocked: None,
        }
    }

    pub fn from_unlocked(unlocked: Unlocked) -> Self {
        let Unlocked { token, dek } = unlocked;
        let mut model = Self::new();
        // A save replaces the whole vault, so a register built from a load that
        // dropped entries -- or from no load at all -- would delete them.
        match policy::load_vault(&token, &dek) {
            Ok(loaded) => {
                model.save_blocked = loaded.drop_refusal();
                model.before = loaded.before;
                for policy::Entry { name, value, meta } in loaded.entries {
                    model.names.push(name.clone());
                    model.values.insert(name.clone(), value);
                    model.meta.insert(name, meta);
                }
            }
            Err(e) => model.save_blocked = Some(format!("vault: {e}")),
        }
        model.token = Some(token);
        model.names.sort();
        model.dek = Some(dek);
        model.note(format!("loaded {}", model.names.len()));
        if let Some(why) = model.save_blocked.clone() {
            model.note(format!("saves refused: {why}"));
        }
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

    pub fn detail_text(&self) -> &str {
        &self.detail
    }

    pub fn activity_lines(&self) -> Vec<String> {
        self.activity.iter().cloned().collect()
    }

    pub fn title(&self) -> String {
        self.selected_name()
            .map(|n| n.to_string())
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

    pub fn handle(&mut self, ev: Event) {
        match ev {
            Event::Click { column, row } => {
                if let Some(k) = hit_key(&self.hits, column, row) {
                    self.handle(Event::Key(k));
                }
            }
            Event::Quit => self.quit = true,
            Event::Esc => self.on_esc(),
            Event::Enter => self.on_enter(),
            Event::Up => {
                if self.is_idle() {
                    self.select_prev();
                }
            }
            Event::Down => {
                if self.is_idle() {
                    self.select_next();
                }
            }
            Event::Backspace => self.on_backspace(),
            Event::Tab => self.on_tab(),
            Event::Key(c) => self.on_char(c),
            Event::Tick | Event::Resize => {}
        }
    }

    fn on_esc(&mut self) {
        match self.mode {
            Mode::Idle => self.quit = true,
            Mode::Modal(_) => self.mode = Mode::Idle,
        }
    }

    fn on_char(&mut self, c: char) {
        match &mut self.mode {
            Mode::Idle => match c {
                'a' | 'A' => self.begin_add(),
                'd' | 'D' => self.begin_delete(),
                'c' | 'C' => self.copy_selected(),
                'q' | 'Q' => self.quit = true,
                'j' => self.select_next(),
                'k' => self.select_prev(),
                _ => {}
            },
            Mode::Modal(Modal::Add { name, value, focus }) => match *focus {
                AddField::Name => name.push(c),
                AddField::Value => value.push(c),
            },
            Mode::Modal(Modal::Delete { .. }) => {}
        }
    }

    fn on_backspace(&mut self) {
        if let Mode::Modal(Modal::Add { name, value, focus }) = &mut self.mode {
            match *focus {
                AddField::Name => {
                    name.pop();
                }
                AddField::Value => {
                    value.pop();
                }
            }
        }
    }

    fn on_tab(&mut self) {
        if let Mode::Modal(Modal::Add { focus, .. }) = &mut self.mode {
            *focus = match *focus {
                AddField::Name => AddField::Value,
                AddField::Value => AddField::Name,
            };
        }
    }

    fn on_enter(&mut self) {
        match self.mode.clone() {
            Mode::Idle => {}
            Mode::Modal(Modal::Add { name, value, .. }) => self.commit_add(name, value),
            Mode::Modal(Modal::Delete { name }) => self.commit_delete(&name),
        }
    }

    fn begin_add(&mut self) {
        if let Some(why) = self.save_blocked.clone() {
            self.note(format!("save refused: {why}"));
            return;
        }
        self.mode = Mode::Modal(Modal::Add {
            name: String::new(),
            value: String::new(),
            focus: AddField::Name,
        });
    }

    fn begin_delete(&mut self) {
        if let Some(why) = self.save_blocked.clone() {
            self.note(format!("save refused: {why}"));
            return;
        }
        let Some(name) = self.selected_name().map(str::to_string) else {
            return;
        };
        self.mode = Mode::Modal(Modal::Delete { name });
    }

    fn commit_add(&mut self, name: String, value: String) {
        if check_name(&name).is_err() {
            self.note("bad name");
            return;
        }
        if !self.names.iter().any(|n| n == &name) {
            self.names.push(name.clone());
            self.names.sort();
        }
        if let Some(i) = self.names.iter().position(|n| n == &name) {
            self.selected = i;
        }
        self.values
            .insert(name.clone(), Secret::new(value.into_bytes()));
        self.meta
            .entry(name)
            .or_insert_with(|| Value::Object(Default::default()));
        self.mode = Mode::Idle;
        self.sync_detail();
        if self.push_snapshot() {
            self.note("saved");
        }
    }

    fn commit_delete(&mut self, name: &str) {
        self.names.retain(|n| n != name);
        self.values.remove(name);
        self.meta.remove(name);
        if self.selected >= self.names.len() {
            self.selected = self.names.len().saturating_sub(1);
        }
        self.mode = Mode::Idle;
        self.sync_detail();
        if self.push_snapshot() {
            self.note("deleted");
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

    fn sync_detail(&mut self) {
        self.detail.clear();
        let Some(name) = self.selected_name() else {
            return;
        };
        let Some(secret) = self.values.get(name) else {
            return;
        };
        match std::str::from_utf8(secret.as_bytes()) {
            Ok(s) => self.detail.push_str(s),
            Err(_) => self.detail.push_str("(binary)"),
        }
    }

    fn push_snapshot(&mut self) -> bool {
        if let Some(why) = self.save_blocked.clone() {
            self.note(format!("save refused: {why}"));
            return false;
        }
        let (Some(token), Some(dek)) = (self.token.as_deref(), self.dek.as_ref()) else {
            return false;
        };
        let saved = login::save_snapshot(
            token,
            dek,
            &self.names,
            &self.values,
            &self.meta,
            &self.before,
        );
        match saved {
            Ok(written) => {
                self.before = written;
                true
            }
            Err(e) => {
                self.note(format!("save failed: {e}"));
                false
            }
        }
    }

    fn note(&mut self, msg: impl Into<String>) {
        self.activity.push_back(msg.into());
        while self.activity.len() > 32 {
            self.activity.pop_front();
        }
    }
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

        m.handle(Event::Key('d'));
        assert!(m.is_idle(), "must not open delete");
        assert!(!claimed_save(&m.activity_lines()));

        m.mode = Mode::Modal(Modal::Add {
            name: "kv/new".into(),
            value: "x".into(),
            focus: AddField::Value,
        });
        m.handle(Event::Enter);
        assert!(!claimed_save(&m.activity_lines()));
        assert!(m.is_idle());

        m.mode = Mode::Modal(Modal::Delete {
            name: "kv/alpha".into(),
        });
        m.handle(Event::Enter);
        assert!(!claimed_save(&m.activity_lines()));
    }
}
