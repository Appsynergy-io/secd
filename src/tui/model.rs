use std::collections::{BTreeMap, HashMap, VecDeque};

use secd_core::{build_payload, check_name, Secret};
use serde_json::Value;
use zeroize::Zeroize;

use crate::login::{self, Unlocked};
use crate::policy::{self, Schema};

use super::form::Form;
use super::tree::{Index, Row};
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

/// Every key, in one place. The action bar draws the subset that fits and the
/// help overlay draws all of it, so a key cannot be bound and undocumented, or
/// documented and unbound.
pub const KEYS: &[(&str, &str)] = &[
    ("a", "add a single value"),
    ("p", "add a provider bundle"),
    ("e", "edit what is selected"),
    ("r", "reveal the selected value"),
    ("c", "copy: a value, or a bundle as an env block"),
    ("d", "delete what is selected"),
    ("/", "filter by path or provider"),
    ("Enter", "open a bundle, or reveal a value"),
    ("Esc", "close the filter, the bundle, then secd"),
    ("j k ↑ ↓", "move"),
    ("PgUp PgDn", "move ten"),
    ("Ctrl-R", "reveal, inside a form"),
    ("?", "this list"),
    ("q", "quit"),
];

/// How far a page key moves a list.
const PAGE: usize = 10;

/// How long a copied value stays on the clipboard. The same 30 seconds the web
/// console holds it: a value that outlives the reason it was copied is a value
/// on the clipboard of whatever runs next.
const CLIP_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// `[p] provider` step 1. The schema list lives on the `Model`, so cloning a
/// `Mode` stays cheap.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Picker {
    pub selected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Modal {
    /// `[a] add`, and `[e] edit` on an entry that is not a bundle.
    Add { form: Form },
    /// `[p] provider` step 1: which schema.
    Pick { pick: Picker },
    /// `[p] provider` step 2, and `[e] edit` on a bundle.
    Provider { form: Form },
    /// `label` is the row as drawn; `names` is every entry it stands for, so
    /// deleting a fused bundle does not leave five of its six fields behind.
    Delete { label: String, names: Vec<String> },
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
    /// The credentials the names make, one row each. Rebuilt from `names`
    /// whenever they, the open bundle or the filter change.
    rows: Vec<Row>,
    index: Index,
    /// The bundle being looked into, empty for the register itself.
    open: String,
    filter: String,
    /// `/` is live: a letter is filter text rather than a command.
    filtering: bool,
    /// `?` overlay.
    helping: bool,
    /// Where the last paint put each scrolling pane, so the next one continues
    /// from it instead of snapping the selection to an edge.
    list_window: usize,
    modal_window: usize,
    /// When the clipboard was last written, so it can be cleared on time.
    copied_at: Option<std::time::Instant>,
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
    /// Which entries stand for one credential, as the runner groups them.
    shapes: Vec<policy::BundleShape>,
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
            rows: Vec::new(),
            index: Index::build(&[], &[]),
            open: String::new(),
            filter: String::new(),
            filtering: false,
            helping: false,
            list_window: 0,
            modal_window: 0,
            copied_at: None,
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
            shapes: Vec::new(),
            token: None,
            dek: None,
            before: BTreeMap::new(),
            save_blocked: None,
        }
    }

    /// The register before it has loaded anything. No I/O: the caller paints
    /// this frame first, then calls `load`.
    pub fn from_unlocked(unlocked: Unlocked) -> Self {
        let Unlocked { token, dek } = unlocked;
        let mut model = Self::new();
        model.token = Some(token);
        model.dek = Some(dek);
        model.note("loading…");
        model
    }

    /// Two round trips, each with a 30 second timeout. They happen after the
    /// first paint, because a blank uninterruptible screen and a hung one look
    /// exactly the same from the outside.
    ///
    /// A save replaces the whole vault, so a register built from a load that
    /// dropped entries -- or from no load at all -- would delete them.
    pub fn load(&mut self) {
        self.load_register();
        self.note(format!("loaded {}", self.names.len()));
        if let Some(why) = self.save_blocked.clone() {
            self.note(format!("saves refused: {why}"));
        }
        // Fetched here, where the register already blocks. Doing it on the
        // keypress would freeze a 250ms poll loop mid-interaction.
        self.load_schemas();
        self.sync_detail();
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

    /// One row per credential.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The bundle being looked into, empty for the register itself.
    pub fn open(&self) -> &str {
        &self.open
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn filtering(&self) -> bool {
        self.filtering
    }

    pub fn helping(&self) -> bool {
        self.helping
    }

    pub fn list_window(&self) -> usize {
        self.list_window
    }

    pub fn modal_window(&self) -> usize {
        self.modal_window
    }

    /// What the last paint decided. The draw knows the pane height; the model
    /// is where it has to survive until the next one.
    pub fn set_painted(&mut self, painted: &super::view::Painted) {
        self.list_window = painted.list;
        self.modal_window = painted.modal;
        self.spots.clone_from(&painted.spots);
    }

    /// Credentials in the register, before the filter.
    pub fn total(&self) -> usize {
        self.index.total()
    }

    fn selected_row(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }

    /// Recompute the rows. `keep` is the entry the selection should land on,
    /// so a save, a filter or an open does not silently move it elsewhere.
    fn rebuild(&mut self, keep: Option<&str>) {
        let want = keep
            .and_then(|n| self.index.owner_of(n))
            .map(ToString::to_string)
            .or_else(|| self.selected_row().map(|r| r.label.clone()));
        self.index = Index::build(&self.names, &self.shapes);
        // An open bundle that the vault no longer holds would show an empty
        // list with no way back but Esc.
        if !self.open.is_empty() && !self.index.is_bundle(&self.open) {
            self.open.clear();
        }
        self.rows = self.index.rows(&self.open, &self.filter);
        self.selected = want
            .and_then(|w| self.rows.iter().position(|r| r.label == w))
            .unwrap_or(0)
            .min(self.rows.len().saturating_sub(1));
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
        self.selected_row()
            .map(|r| r.label.clone())
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
            Event::Backspace => self.on_backspace(),
            Event::Delete => self.on_form(Form::delete),
            Event::Tab => self.on_form(Form::focus_next),
            Event::BackTab => self.on_form(Form::focus_prev),
            Event::PageUp => self.on_page(false),
            Event::PageDown => self.on_page(true),
            Event::Reveal => self.on_reveal(),
            Event::Key(c) => self.on_char(c),
            Event::Tick | Event::Resize => self.expire_clipboard(),
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

    fn begin_filter(&mut self) {
        self.filtering = true;
        self.helping = false;
    }

    fn on_backspace(&mut self) {
        if self.is_idle() && self.filtering {
            self.filter.pop();
            let keep = self.selected_name().map(ToString::to_string);
            self.rebuild(keep.as_deref());
            self.sync_detail();
            return;
        }
        self.on_form(Form::backspace);
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
                if self.is_idle() && i < self.rows.len() {
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

    /// One layer at a time. Esc that quits from under a filter, an open
    /// bundle or the help overlay is Esc throwing away context the human is
    /// still using; only the bare register has nothing left to close.
    fn on_esc(&mut self) {
        if let Mode::Modal(_) = self.mode {
            self.mode = Mode::Idle;
            return;
        }
        if self.helping {
            self.helping = false;
        } else if self.filtering || !self.filter.is_empty() {
            self.clear_filter();
        } else if !self.open.is_empty() {
            self.close_bundle();
        } else {
            self.quit = true;
        }
    }

    fn clear_filter(&mut self) {
        let keep = self.selected_name().map(ToString::to_string);
        self.filtering = false;
        self.filter.clear();
        self.rebuild(keep.as_deref());
        self.sync_detail();
    }

    /// Back out to the register, landing on the bundle that was open.
    fn close_bundle(&mut self) {
        let was = std::mem::take(&mut self.open);
        self.rebuild(None);
        if let Some(i) = self.rows.iter().position(|r| r.label == was) {
            self.selected = i;
        }
        self.sync_detail();
    }

    /// Enter on a bundle opens it onto its fields. Enter on a single value
    /// has nothing to open, so it does the other thing a closer look means.
    fn open_selected(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if !row.descends() {
            self.reveal_all = !self.reveal_all;
            return;
        }
        self.open = row.label.clone();
        self.filtering = false;
        self.filter.clear();
        self.rebuild(None);
        self.sync_detail();
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
            self.selected = self.rows.len().saturating_sub(1);
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
            let last = self.rows.len().saturating_sub(1);
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
        if self.is_idle() && self.filtering {
            self.filter.push(c);
            let keep = self.selected_name().map(ToString::to_string);
            self.rebuild(keep.as_deref());
            self.sync_detail();
            return;
        }
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
                '/' => self.begin_filter(),
                '?' => self.helping = !self.helping,
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
        if self.filtering {
            // Enter commits the filter and hands the letters back to the
            // commands, which is the only way to act on what you found.
            self.filtering = false;
            return;
        }
        let mode = std::mem::replace(&mut self.mode, Mode::Idle);
        match mode {
            Mode::Idle => self.open_selected(),
            Mode::Modal(Modal::Add { form }) => self.commit_single(form),
            Mode::Modal(Modal::Provider { form }) => self.commit_provider(form),
            Mode::Modal(Modal::Pick { pick }) => self.open_schema_form(pick.selected),
            Mode::Modal(Modal::Delete { names, .. }) => self.commit_delete(&names),
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
        let Some(row) = self.selected_row().cloned() else {
            self.note("nothing selected");
            return;
        };
        if row.descends() {
            return self.edit_fused(&row);
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

    /// Edit a bundle stored as siblings: one form over the credential, saved
    /// back as the entries it came from.
    fn edit_fused(&mut self, row: &Row) {
        let found = row
            .provider
            .as_deref()
            .and_then(|p| self.schemas.iter().find(|s| s.name == p))
            .cloned();
        let Some(schema) = found else {
            self.note("no schema for this bundle: edit a field on its own");
            return;
        };
        let mut pairs = Vec::with_capacity(row.members.len());
        for m in &row.members {
            let key = m.rsplit_once('/').map_or(m.as_str(), |(_, k)| k);
            let Some(text) = self.plaintext(m) else {
                self.note("binary value: edit is unavailable");
                return;
            };
            pairs.push((key.to_string(), text));
        }
        let mut form = Form::schema(&schema, &row.label, &pairs);
        form.title = "edit".to_string();
        form.editing = Some(row.label.clone());
        self.mode = Mode::Modal(Modal::Provider { form });
    }

    fn begin_delete(&mut self) {
        if self.refuse_if_blocked() {
            return;
        }
        let Some(row) = self.selected_row() else {
            self.note("nothing selected");
            return;
        };
        self.mode = Mode::Modal(Modal::Delete {
            label: row.label.clone(),
            names: row.members.clone(),
        });
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
        if let Err(why) = self.commit(
            vec![(name, value, Value::Object(Default::default()))],
            drop_old.into_iter().collect(),
        ) {
            // Keep the modal, keep the text. `Input` zeroes itself on drop, so
            // closing the form on a failed save destroys what was typed and
            // leaves one grey line to explain it.
            self.form_refuse(Modal::Add { form }, why);
        }
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
        let old = form.editing.clone();
        // A bundle stored as siblings is written back as siblings. Folding it
        // into one JSON entry would silently change how it is stored, and
        // `secd run` reads the two shapes by different paths.
        let fused = old
            .as_deref()
            .is_some_and(|o| !self.names.iter().any(|n| n == o) && self.index.is_bundle(o));
        let gone: Vec<String> = match &old {
            Some(o) if fused => self
                .members_of(o)
                .into_iter()
                .filter(|m| !pairs.iter().any(|(k, _)| *m == format!("{name}/{k}")))
                .collect(),
            Some(o) if *o != name => vec![o.clone()],
            _ => Vec::new(),
        };
        let writes = if fused {
            pairs
                .iter()
                .map(|(k, v)| {
                    (
                        format!("{name}/{k}"),
                        Secret::new(v.clone().into_bytes()),
                        Value::Object(Default::default()),
                    )
                })
                .collect()
        } else {
            let keys: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
            let meta = policy::provider_meta(&schema.name, &keys);
            vec![(
                name,
                Secret::new(policy::payload_json(&pairs).into_bytes()),
                meta,
            )]
        };
        if let Err(why) = self.commit(writes, gone) {
            self.form_refuse(Modal::Provider { form }, why);
        }
    }

    /// The entries a credential stands for, by its row label.
    fn members_of(&self, prefix: &str) -> Vec<String> {
        self.names
            .iter()
            .filter(|n| self.index.owner_of(n) == Some(prefix))
            .cloned()
            .collect()
    }

    /// An edit may rename, but not onto a name that is already taken: that
    /// would silently replace a second entry.
    fn rename_collision(&self, form: &Form, name: &str) -> Option<String> {
        let old = form.editing.as_deref()?;
        if old == name {
            return None;
        }
        // A fused bundle's own members are not a collision with itself.
        let mine = self.members_of(old);
        let taken = self
            .names
            .iter()
            .any(|n| n == name || (n.starts_with(&format!("{name}/")) && !mine.contains(n)));
        taken.then(|| format!("{name} already exists"))
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

    /// Save over the whole register. `writes` is one entry for a single value
    /// or a JSON bundle, and several for a bundle stored as siblings; `gone` is
    /// what a rename left behind. Both halves are one PUT, so an entry is never
    /// duplicated and never briefly absent.
    fn commit(
        &mut self,
        writes: Vec<(String, Secret, Value)>,
        gone: Vec<String>,
    ) -> Result<(), String> {
        if let Some(why) = self.save_blocked.clone() {
            return Err(format!("save refused: {why}"));
        }
        if writes.is_empty() {
            return Err("nothing to save".to_string());
        }
        let empty = Value::Object(Default::default());
        let mut names: Vec<String> = self
            .names
            .iter()
            .filter(|n| !gone.iter().any(|g| g == *n))
            .cloned()
            .collect();
        for (name, _, _) in &writes {
            if !names.iter().any(|n| n == name) {
                names.push(name.clone());
            }
        }
        names.sort();
        let saved = {
            let rows: Vec<policy::Row<'_>> = names
                .iter()
                .filter_map(|n| {
                    if let Some((name, v, m)) = writes.iter().find(|(w, _, _)| w == n) {
                        return Some((name.as_str(), v, m));
                    }
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
                for old in &gone {
                    self.values.remove(old);
                    self.meta.remove(old);
                }
                let first = writes[0].0.clone();
                let n = writes.len();
                for (name, value, meta) in writes {
                    self.values.insert(name.clone(), value);
                    // The caller's meta, always: an overwrite that kept the old
                    // one would leave `secd info` describing an entry that is
                    // gone.
                    self.meta.insert(name, meta);
                }
                self.reshape();
                self.rebuild(Some(&first));
                self.sync_detail();
                self.note(format!("saved {n} {}", entry_word(n)));
                Ok(())
            }
            Some(Err(e)) => {
                self.reload_from_vault();
                Err(format!("save failed: {e}"))
            }
            None => Err("not signed in".to_string()),
        }
    }

    fn commit_delete(&mut self, gone: &[String]) {
        self.mode = Mode::Idle;
        if self.refuse_if_blocked() || gone.is_empty() {
            return;
        }
        let empty = Value::Object(Default::default());
        let names: Vec<String> = self
            .names
            .iter()
            .filter(|n| !gone.iter().any(|g| g == *n))
            .cloned()
            .collect();
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
                for n in gone {
                    self.values.remove(n);
                    self.meta.remove(n);
                }
                self.reshape();
                self.rebuild(None);
                self.sync_detail();
                let n = gone.len();
                self.note(format!("deleted {n} {}", entry_word(n)));
            }
            Some(Err(e)) => {
                self.note(format!("save failed: {e}"));
                self.reload_from_vault();
            }
            None => {}
        }
    }

    /// A single value copies as itself. A bundle copies as the `.env` block
    /// the console writes, because a raw JSON object is not what anyone is
    /// about to paste. Both are cleared on exit.
    fn copy_selected(&mut self) {
        if self.detail.is_empty() {
            self.note("nothing to copy");
            return;
        }
        let one_value = self.detail.len() == 1 && self.detail[0].key.is_empty();
        let mut text = if one_value {
            self.detail[0].value.clone()
        } else {
            let mut out = String::new();
            for r in &self.detail {
                let key = if r.env.is_empty() {
                    r.key.to_ascii_uppercase()
                } else {
                    r.env.clone()
                };
                out.push_str(&key);
                out.push('=');
                out.push_str(&r.value);
                out.push('\n');
            }
            out
        };
        login::clipboard_set(text.as_bytes());
        text.zeroize();
        self.copied_at = Some(std::time::Instant::now());
        let what = if one_value { "value" } else { "env block" };
        let secs = CLIP_TTL.as_secs();
        self.note(format!("copied {what}; clipboard cleared in {secs}s"));
    }

    /// Called on every idle tick. The register polls at 250ms, so the clear
    /// lands within a quarter second of the deadline without a thread.
    fn expire_clipboard(&mut self) {
        let Some(at) = self.copied_at else {
            return;
        };
        if at.elapsed() < CLIP_TTL {
            return;
        }
        self.copied_at = None;
        login::clipboard_clear();
        self.note("clipboard cleared");
    }

    fn select_next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.rows.len() - 1);
        self.sync_detail();
    }

    fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.sync_detail();
    }

    /// The selected row's entry, when the row is exactly one. A fused bundle
    /// has no single name, so anything that needs one asks for it and gets
    /// `None` rather than the first member by accident.
    fn selected_name(&self) -> Option<&str> {
        self.selected_row()?.only()
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
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        match row.only() {
            Some(name) => self.detail_of_entry(name),
            None => self.detail_of_fused(&row),
        }
    }

    /// One entry: a JSON bundle if it reads as one, else a single value.
    fn detail_of_entry(&mut self, name: &str) {
        if !self.values.contains_key(name) {
            return;
        }
        let Some(text) = self.plaintext(name) else {
            self.detail.push(DetailRow {
                key: String::new(),
                env: String::new(),
                secret: false,
                value: "(binary)".to_string(),
            });
            return;
        };
        if let Some((schema, pairs)) = self.bundle_of(name, &text) {
            self.push_bundle(&schema, &pairs);
            return;
        }
        self.detail.push(DetailRow {
            key: String::new(),
            env: String::new(),
            secret: true,
            value: text,
        });
    }

    /// Siblings under a shared parent, read as the one credential they are.
    /// The key is the leaf segment, which is what the fusion matched on.
    fn detail_of_fused(&mut self, row: &Row) {
        let pairs: Vec<(String, String)> = row
            .members
            .iter()
            .map(|m| {
                let key = m.rsplit_once('/').map_or(m.as_str(), |(_, k)| k);
                let text = self.plaintext(m).unwrap_or_else(|| "(binary)".to_string());
                (key.to_string(), text)
            })
            .collect();
        let schema = row
            .provider
            .as_deref()
            .and_then(|p| self.schemas.iter().find(|s| s.name == p))
            .cloned();
        match schema {
            Some(schema) => self.push_bundle(&schema, &pairs),
            None => {
                for (k, v) in pairs {
                    self.detail.push(DetailRow {
                        key: k,
                        env: String::new(),
                        secret: true,
                        value: v,
                    });
                }
            }
        }
    }

    /// Schema order first, then whatever the schema does not name. A key the
    /// schema does not know is masked, as the console does.
    fn push_bundle(&mut self, schema: &Schema, pairs: &[(String, String)]) {
        let n = pairs.len();
        let unit = if n == 1 { "field" } else { "fields" };
        self.detail_title = format!("{} \u{b7} {n} {unit}", schema.title);
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
        for (k, v) in pairs {
            if schema.fields.iter().any(|f| f.key == *k) {
                continue;
            }
            self.detail.push(DetailRow {
                key: k.clone(),
                env: String::new(),
                secret: true,
                value: v.clone(),
            });
        }
    }

    /// Regroup from what the register holds now. A save changes which entries
    /// are siblings, so the rows have to be recomputed from the values rather
    /// than from the load that first produced them.
    fn reshape(&mut self) {
        let empty = Value::Object(Default::default());
        let shapes = {
            let rows: Vec<policy::Row<'_>> = self
                .names
                .iter()
                .filter_map(|n| {
                    Some((
                        n.as_str(),
                        self.values.get(n)?,
                        self.meta.get(n).unwrap_or(&empty),
                    ))
                })
                .collect();
            policy::shapes_of(&rows)
        };
        self.shapes = shapes;
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
        // Before the entries are taken apart: the fusion needs the values.
        self.shapes = policy::bundle_shapes(&loaded.entries);
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
        self.rebuild(None);
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

fn entry_word(n: usize) -> &'static str {
    if n == 1 {
        "entry"
    } else {
        "entries"
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
        let form = m.form().expect("a refused save keeps the modal");
        assert_eq!(form.value_pairs()[0].1, "x", "and keeps what was typed");
        assert!(
            form.error.as_deref().is_some_and(|e| e.contains("refused")),
            "and says why"
        );
        m.handle(Event::Esc);
        assert_eq!(m.names(), ["kv/alpha"]);

        m.mode = Mode::Modal(Modal::Delete {
            label: "kv/alpha".into(),
            names: vec!["kv/alpha".into()],
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
        assert!(
            m.names().is_empty(),
            "no session: add must not keep the row"
        );
        assert!(!claimed_save(&m.activity_lines()));
        // A 64-character token pasted into a form is not something to lose to
        // a failed round trip.
        let form = m.form().expect("a failed save keeps the modal");
        assert_eq!(form.value_pairs()[0].1, "x", "and keeps what was typed");
        m.handle(Event::Esc);

        m.names.push("kv/alpha".into());
        m.values
            .insert("kv/alpha".into(), Secret::new(b"x".to_vec()));
        m.mode = Mode::Modal(Modal::Delete {
            label: "kv/alpha".into(),
            names: vec!["kv/alpha".into()],
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
        assert!(
            m.names().is_empty(),
            "no session: the provider row must not survive"
        );
        assert!(!claimed_save(&m.activity_lines()));
        let form = m.form().expect("a failed save keeps the modal");
        assert_eq!(form.value_pairs()[0].1, "t", "and keeps what was typed");
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
