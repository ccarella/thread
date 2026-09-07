//! Application model, update, and terminal lifecycle (alt screen + raw mode).

use crate::config::Config;
use crate::editor;
use crate::error::Result;
use crate::keys::{self, InputMode, Message};
use crate::note::{Note, Status};
use crate::store::{self, Store};
use crate::ui;
use chrono::Utc;
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// Body of a note created with `n` (cursor lands after this in Insert).
pub const NEW_NOTE_STARTER: &str = "What am I trying to decide?\n\n";

/// Autosave a dirty buffer this often (BUILD_SPEC §7).
pub const AUTOSAVE_AFTER: Duration = Duration::from_secs(30);

/// Last N body lines included by `p` (pull-quote).
pub const PULL_QUOTE_LINES: usize = 20;

/// RAII guard: `ratatui::init` enables raw mode + alt screen and installs a
/// panic hook; `Drop` always calls `ratatui::restore`.
pub struct TerminalGuard {
    terminal: DefaultTerminal,
}

impl TerminalGuard {
    pub fn enter() -> Self {
        Self {
            terminal: ratatui::init(),
        }
    }

    pub fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleStep {
    Topic,
    Title,
}

#[derive(Debug, Clone)]
pub struct TitleState {
    pub step: TitleStep,
    pub topic: String,
    pub title: String,
    pub cursor: usize,
    pub error: Option<String>,
}

impl TitleState {
    pub fn input(&self) -> &str {
        match self.step {
            TitleStep::Topic => &self.topic,
            TitleStep::Title => &self.title,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftPane {
    Topics,
    Notes,
}

#[derive(Debug, Clone)]
enum Mode {
    Normal,
    Insert,
    Title(TitleState),
    Search,
}

/// Elm-style model for the two-pane TUI with Insert / Title editing.
#[derive(Debug)]
pub struct App {
    store: Store,
    topics: Vec<String>,
    selected: usize,
    notes: Vec<Note>,
    note_selected: usize,
    left_pane: LeftPane,
    show_scratch_in_thread: bool,
    search_query: String,
    search_hits: Vec<Note>,
    search_selected: usize,
    opened: Option<Note>,
    mode: Mode,
    cursor: usize,
    dirty: bool,
    dirty_since: Option<Instant>,
    pub should_quit: bool,
}

impl App {
    pub fn new(store: Store) -> Result<Self> {
        Self::with_config(store, Config::default())
    }

    pub fn with_config(store: Store, config: Config) -> Result<Self> {
        let topics = store.list_topics()?;
        let mut app = Self {
            store,
            topics,
            selected: 0,
            notes: Vec::new(),
            note_selected: 0,
            left_pane: LeftPane::Topics,
            show_scratch_in_thread: config.show_scratch_in_thread,
            search_query: String::new(),
            search_hits: Vec::new(),
            search_selected: 0,
            opened: None,
            mode: Mode::Normal,
            cursor: 0,
            dirty: false,
            dirty_since: None,
            should_quit: false,
        };
        app.open_latest()?;
        Ok(app)
    }

    pub fn topics(&self) -> &[String] {
        &self.topics
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_topic(&self) -> Option<&str> {
        self.topics.get(self.selected).map(String::as_str)
    }

    pub fn left_pane(&self) -> LeftPane {
        self.left_pane
    }

    pub fn left_title(&self) -> &'static str {
        if matches!(self.mode, Mode::Search) {
            return "search";
        }
        match self.left_pane {
            LeftPane::Topics => "topics",
            LeftPane::Notes => "notes",
        }
    }

    pub fn left_labels(&self) -> Vec<String> {
        if matches!(self.mode, Mode::Search) {
            return self
                .search_hits
                .iter()
                .map(|note| format!("{}  {}", note.topic, note.title))
                .collect();
        }
        match self.left_pane {
            LeftPane::Topics => self.topics.clone(),
            LeftPane::Notes => self
                .notes
                .iter()
                .map(|note| format!("{}  {}", note.created.format("%Y-%m-%d"), note.title))
                .collect(),
        }
    }

    pub fn left_selected(&self) -> usize {
        if matches!(self.mode, Mode::Search) {
            return self.search_selected;
        }
        match self.left_pane {
            LeftPane::Topics => self.selected,
            LeftPane::Notes => self.note_selected,
        }
    }

    pub fn thread_notes(&self) -> &[Note] {
        &self.notes
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    pub fn search_hits(&self) -> &[Note] {
        &self.search_hits
    }

    pub fn opened(&self) -> Option<&Note> {
        self.opened.as_ref()
    }

    pub fn input_mode(&self) -> InputMode {
        match self.mode {
            Mode::Normal => InputMode::Normal,
            Mode::Insert => InputMode::Insert,
            Mode::Title(_) => InputMode::Title,
            Mode::Search => InputMode::Search,
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn title_state(&self) -> Option<&TitleState> {
        match &self.mode {
            Mode::Title(state) => Some(state),
            _ => None,
        }
    }

    pub fn run(mut self) -> Result<()> {
        let mut terminal = TerminalGuard::enter();
        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, &self))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if let Some(message) = keys::message_from_key(key, self.input_mode()) {
                    self.update(message)?;
                }
            }
        }
        self.maybe_autosave()?;
        Ok(())
    }

    pub fn update(&mut self, message: Message) -> Result<()> {
        match self.input_mode() {
            InputMode::Title => self.update_title(message)?,
            InputMode::Insert => self.update_insert(message)?,
            InputMode::Search => self.update_search(message)?,
            InputMode::Normal => self.update_normal(message)?,
        }
        Ok(())
    }

    fn update_normal(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Quit => {
                self.save_if_needed()?;
                self.should_quit = true;
            }
            Message::SelectDown => self.select_delta(1)?,
            Message::SelectUp => self.select_delta(-1)?,
            Message::OpenSelected => self.open_selected()?,
            Message::EnterInsert => {
                if self.opened.is_some() {
                    self.mode = Mode::Insert;
                }
            }
            Message::EnterInsertAppend => {
                if let Some(note) = &self.opened {
                    self.cursor = editor::char_len(&note.body);
                    self.mode = Mode::Insert;
                }
            }
            Message::StartNewNote => self.start_new_note()?,
            Message::ToggleThread => self.toggle_thread()?,
            Message::ToggleStatus => self.toggle_status()?,
            Message::PullQuote => self.pull_quote()?,
            Message::StartSearch => self.start_search()?,
            Message::Save => self.save_if_needed()?,
            Message::Escape => {}
            _ => {}
        }
        Ok(())
    }

    fn update_insert(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Escape => {
                self.leave_insert()?;
            }
            Message::Save => self.save_if_needed()?,
            Message::Quit => {
                // `q` is a character in Insert; Quit is Normal-only.
            }
            Message::InsertChar(c) => self.edit_body(|body, cursor| {
                editor::insert_char(body, cursor, c);
            }),
            Message::Backspace => self.edit_body(editor::backspace),
            Message::Newline => self.edit_body(|body, cursor| {
                editor::insert_char(body, cursor, '\n');
            }),
            Message::MoveLeft => {
                if let Some(note) = &self.opened {
                    editor::move_left(&note.body, &mut self.cursor);
                }
            }
            Message::MoveRight => {
                if let Some(note) = &self.opened {
                    editor::move_right(&note.body, &mut self.cursor);
                }
            }
            Message::MoveUp => {
                if let Some(note) = &self.opened {
                    editor::move_up(&note.body, &mut self.cursor);
                }
            }
            Message::MoveDown => {
                if let Some(note) = &self.opened {
                    editor::move_down(&note.body, &mut self.cursor);
                }
            }
            Message::Home => {
                if let Some(note) = &self.opened {
                    editor::home(&note.body, &mut self.cursor);
                }
            }
            Message::End => {
                if let Some(note) = &self.opened {
                    editor::end(&note.body, &mut self.cursor);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn update_title(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Escape => {
                self.mode = Mode::Normal;
            }
            Message::Submit => self.submit_title()?,
            Message::Save => {}
            Message::InsertChar(c) => self.edit_title(|input, cursor| {
                editor::insert_char(input, cursor, c);
            }),
            Message::Backspace => self.edit_title(editor::backspace),
            Message::MoveLeft => {
                if let Mode::Title(state) = &mut self.mode {
                    let mut cursor = state.cursor;
                    editor::move_left(state.input(), &mut cursor);
                    state.cursor = cursor;
                }
            }
            Message::MoveRight => {
                if let Mode::Title(state) = &mut self.mode {
                    let mut cursor = state.cursor;
                    editor::move_right(state.input(), &mut cursor);
                    state.cursor = cursor;
                }
            }
            Message::Home => {
                if let Mode::Title(state) = &mut self.mode {
                    state.cursor = 0;
                }
            }
            Message::End => {
                if let Mode::Title(state) = &mut self.mode {
                    state.cursor = editor::char_len(state.input());
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn update_search(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Escape => {
                self.mode = Mode::Normal;
                self.search_query.clear();
                self.search_hits.clear();
                self.search_selected = 0;
            }
            Message::Submit => self.open_search_hit()?,
            Message::SelectDown => {
                Self::move_index(&mut self.search_selected, self.search_hits.len(), 1)
            }
            Message::SelectUp => {
                Self::move_index(&mut self.search_selected, self.search_hits.len(), -1)
            }
            Message::InsertChar(c) => {
                self.search_query.push(c);
                self.search_selected = 0;
                self.refresh_search()?;
            }
            Message::Backspace => {
                if let Some((idx, _)) = self.search_query.char_indices().next_back() {
                    self.search_query.truncate(idx);
                }
                self.search_selected = 0;
                self.refresh_search()?;
            }
            Message::Save => self.save_if_needed()?,
            _ => {}
        }
        Ok(())
    }

    fn edit_body(&mut self, f: impl FnOnce(&mut String, &mut usize)) {
        let Some(note) = self.opened.as_mut() else {
            return;
        };
        f(&mut note.body, &mut self.cursor);
        self.mark_dirty();
    }

    fn edit_title(&mut self, f: impl FnOnce(&mut String, &mut usize)) {
        let Mode::Title(state) = &mut self.mode else {
            return;
        };
        state.error = None;
        match state.step {
            TitleStep::Topic => f(&mut state.topic, &mut state.cursor),
            TitleStep::Title => f(&mut state.title, &mut state.cursor),
        }
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        if self.dirty_since.is_none() {
            self.dirty_since = Some(Instant::now());
        }
    }

    fn leave_insert(&mut self) -> Result<()> {
        self.save_if_needed()?;
        self.mode = Mode::Normal;
        Ok(())
    }

    fn start_new_note(&mut self) -> Result<()> {
        self.save_if_needed()?;
        let topic = self.selected_topic().unwrap_or("").to_string();
        let cursor = editor::char_len(&topic);
        self.mode = Mode::Title(TitleState {
            step: TitleStep::Topic,
            topic,
            title: String::new(),
            cursor,
            error: None,
        });
        Ok(())
    }

    fn submit_title(&mut self) -> Result<()> {
        let (topic, title) = {
            let Mode::Title(state) = &mut self.mode else {
                return Ok(());
            };
            match state.step {
                TitleStep::Topic => {
                    let topic = state.topic.trim().to_string();
                    if topic.is_empty() {
                        state.error = Some("topic required".into());
                        return Ok(());
                    }
                    if let Err(err) = store::validate_topic(&topic) {
                        state.error = Some(err.to_string());
                        return Ok(());
                    }
                    state.topic = topic;
                    state.step = TitleStep::Title;
                    state.cursor = editor::char_len(&state.title);
                    state.error = None;
                    return Ok(());
                }
                TitleStep::Title => (
                    state.topic.trim().to_string(),
                    state.title.trim().to_string(),
                ),
            }
        };
        self.create_note(topic, title)
    }

    fn create_note(&mut self, topic: String, title: String) -> Result<()> {
        let mut note = Note::new(topic.clone(), title, NEW_NOTE_STARTER);
        self.store.save(&mut note)?;
        self.dirty = false;
        self.dirty_since = None;
        self.opened = Some(note);
        self.cursor = editor::char_len(NEW_NOTE_STARTER);
        self.mode = Mode::Insert;
        self.refresh_topics()?;
        if let Some(idx) = self.topics.iter().position(|t| t == &topic) {
            self.selected = idx;
        }
        if self.left_pane == LeftPane::Notes {
            self.refresh_notes()?;
            let path = self.opened.as_ref().and_then(|n| n.path.clone());
            self.select_note_by_path(path.as_deref());
        }
        Ok(())
    }

    fn move_selection(&mut self, delta: i32) {
        if self.topics.is_empty() {
            self.selected = 0;
            return;
        }
        let last = (self.topics.len() - 1) as i32;
        self.selected = (self.selected as i32 + delta).clamp(0, last) as usize;
    }

    fn move_index(selected: &mut usize, len: usize, delta: i32) {
        if len == 0 {
            *selected = 0;
            return;
        }
        let last = (len - 1) as i32;
        *selected = (*selected as i32 + delta).clamp(0, last) as usize;
    }

    fn select_delta(&mut self, delta: i32) -> Result<()> {
        match self.left_pane {
            LeftPane::Topics => {
                self.move_selection(delta);
                self.open_latest()?;
            }
            LeftPane::Notes => {
                let len = self.notes.len();
                Self::move_index(&mut self.note_selected, len, delta);
            }
        }
        Ok(())
    }

    fn open_selected(&mut self) -> Result<()> {
        match self.left_pane {
            LeftPane::Topics => self.open_latest(),
            LeftPane::Notes => self.open_selected_note(),
        }
    }

    fn open_selected_note(&mut self) -> Result<()> {
        self.save_if_needed()?;
        let path = self
            .notes
            .get(self.note_selected)
            .and_then(|note| note.path.clone());
        self.opened = match path {
            Some(path) => Some(self.store.load(&path)?),
            None => None,
        };
        self.cursor = 0;
        Ok(())
    }

    fn open_latest(&mut self) -> Result<()> {
        self.save_if_needed()?;
        self.opened = match self.selected_topic() {
            Some(topic) => self.store.latest(topic)?,
            None => None,
        };
        self.cursor = 0;
        Ok(())
    }

    fn toggle_thread(&mut self) -> Result<()> {
        match self.left_pane {
            LeftPane::Topics => self.enter_notes_pane()?,
            LeftPane::Notes => {
                self.left_pane = LeftPane::Topics;
            }
        }
        Ok(())
    }

    fn enter_notes_pane(&mut self) -> Result<()> {
        self.refresh_notes()?;
        let path = self.opened.as_ref().and_then(|n| n.path.clone());
        self.select_note_by_path(path.as_deref());
        if path.is_none() && !self.notes.is_empty() {
            self.note_selected = self.notes.len() - 1;
        }
        self.left_pane = LeftPane::Notes;
        Ok(())
    }

    fn refresh_notes(&mut self) -> Result<()> {
        self.notes = match self.selected_topic() {
            Some(topic) => {
                let mut notes = self.store.list_notes(topic)?;
                if !self.show_scratch_in_thread {
                    notes.retain(|note| note.status == Status::Keep);
                }
                notes.sort_by(|a, b| {
                    a.created
                        .cmp(&b.created)
                        .then_with(|| a.title.cmp(&b.title))
                });
                notes
            }
            None => Vec::new(),
        };
        if self.notes.is_empty() {
            self.note_selected = 0;
        } else {
            self.note_selected = self.note_selected.min(self.notes.len() - 1);
        }
        Ok(())
    }

    fn select_note_by_path(&mut self, path: Option<&Path>) {
        let Some(path) = path else {
            if !self.notes.is_empty() {
                self.note_selected = self.notes.len() - 1;
            }
            return;
        };
        if let Some(idx) = self
            .notes
            .iter()
            .position(|n| n.path.as_deref() == Some(path))
        {
            self.note_selected = idx;
        } else if !self.notes.is_empty() {
            self.note_selected = self.notes.len() - 1;
        }
    }

    fn toggle_status(&mut self) -> Result<()> {
        let Some(note) = self.opened.as_mut() else {
            return Ok(());
        };
        note.status = match note.status {
            Status::Scratch => Status::Keep,
            Status::Keep => Status::Scratch,
        };
        note.updated = Utc::now();
        self.store.save(note)?;
        self.dirty = false;
        self.dirty_since = None;
        self.refresh_topics()?;
        if self.left_pane == LeftPane::Notes {
            let path = self.opened.as_ref().and_then(|n| n.path.clone());
            self.refresh_notes()?;
            self.select_note_by_path(path.as_deref());
        }
        Ok(())
    }

    fn last_keep_or_latest(&self, topic: &str) -> Result<Option<Note>> {
        let notes = self.store.list_notes(topic)?;
        let keep = notes
            .iter()
            .filter(|note| note.status == Status::Keep)
            .max_by(|a, b| {
                a.updated
                    .cmp(&b.updated)
                    .then_with(|| a.created.cmp(&b.created))
                    .then_with(|| a.title.cmp(&b.title))
            });
        if let Some(keep) = keep {
            return Ok(Some(keep.clone()));
        }
        self.store.latest(topic)
    }

    fn pull_quote(&mut self) -> Result<()> {
        if self.opened.is_none() {
            return Ok(());
        }
        let topic = self.opened.as_ref().unwrap().topic.clone();
        let Some(source) = self.last_keep_or_latest(&topic)? else {
            return Ok(());
        };
        let quote = source.quoted_excerpt(PULL_QUOTE_LINES);
        {
            let note = self.opened.as_mut().unwrap();
            if !note.body.is_empty() && !note.body.ends_with('\n') {
                note.body.push('\n');
            }
            if !note.body.is_empty() {
                note.body.push('\n');
            }
            note.body.push_str(&quote);
            self.cursor = editor::char_len(&note.body);
        }
        self.mark_dirty();
        self.mode = Mode::Insert;
        Ok(())
    }

    fn start_search(&mut self) -> Result<()> {
        self.search_query.clear();
        self.search_selected = 0;
        self.refresh_search()?;
        self.mode = Mode::Search;
        Ok(())
    }

    fn refresh_search(&mut self) -> Result<()> {
        self.search_hits = self.store.search(&self.search_query)?;
        if self.search_hits.is_empty() {
            self.search_selected = 0;
        } else {
            self.search_selected = self.search_selected.min(self.search_hits.len() - 1);
        }
        Ok(())
    }

    fn open_search_hit(&mut self) -> Result<()> {
        self.save_if_needed()?;
        let Some(hit) = self.search_hits.get(self.search_selected).cloned() else {
            self.mode = Mode::Normal;
            return Ok(());
        };
        let topic = hit.topic.clone();
        let path = hit.path.clone();
        self.mode = Mode::Normal;
        self.left_pane = LeftPane::Topics;
        self.search_query.clear();
        self.search_hits.clear();
        self.search_selected = 0;
        self.refresh_topics()?;
        if let Some(idx) = self.topics.iter().position(|t| t == &topic) {
            self.selected = idx;
        }
        self.opened = match path {
            Some(path) => Some(self.store.load(&path)?),
            None => Some(hit),
        };
        self.cursor = 0;
        Ok(())
    }

    fn refresh_topics(&mut self) -> Result<()> {
        let current = self.selected_topic().map(str::to_string);
        self.topics = self.store.list_topics()?;
        self.selected = current
            .and_then(|topic| self.topics.iter().position(|t| t == &topic))
            .unwrap_or(0);
        Ok(())
    }

    /// Persist a dirty buffer if one exists. Also used on quit and autosave.
    fn save_if_needed(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let Some(note) = self.opened.as_mut() else {
            self.dirty = false;
            self.dirty_since = None;
            return Ok(());
        };
        note.updated = Utc::now();
        self.store.save(note)?;
        self.dirty = false;
        self.dirty_since = None;
        self.refresh_topics()?;
        Ok(())
    }

    fn maybe_autosave(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let Some(since) = self.dirty_since else {
            return Ok(());
        };
        if since.elapsed() >= AUTOSAVE_AFTER {
            self.save_if_needed()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{Note, Status};
    use chrono::{TimeZone, Utc};
    use std::fs;

    fn seeded_store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut older = Note::new("philosophy", "Forms", "old body");
        older.created = Utc.with_ymd_and_hms(2026, 8, 1, 10, 0, 0).unwrap();
        older.updated = older.created;
        let mut rust_old = Note::new("rust", "Old", "rust old");
        rust_old.created = Utc.with_ymd_and_hms(2026, 9, 1, 10, 0, 0).unwrap();
        rust_old.updated = rust_old.created;
        let mut rust_new = Note::new("rust", "Ownership", "borrow checker");
        rust_new.created = Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap();
        rust_new.updated = rust_new.created;
        store.save(&mut older).unwrap();
        store.save(&mut rust_old).unwrap();
        store.save(&mut rust_new).unwrap();
        (store, dir)
    }

    #[test]
    fn starts_on_most_recent_topic_with_its_latest_note() {
        let (store, _dir) = seeded_store();
        let app = App::new(store).unwrap();
        assert_eq!(app.topics(), &["rust", "philosophy"]);
        assert_eq!(app.selected_topic(), Some("rust"));
        assert_eq!(app.opened().unwrap().title, "Ownership");
        assert_eq!(app.opened().unwrap().body, "borrow checker");
        assert_eq!(app.input_mode(), InputMode::Normal);
    }

    #[test]
    fn j_k_move_selection_and_load_latest_clamped() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();

        app.update(Message::SelectUp).unwrap();
        assert_eq!(app.selected_topic(), Some("rust"));

        app.update(Message::SelectDown).unwrap();
        assert_eq!(app.selected_topic(), Some("philosophy"));
        assert_eq!(app.opened().unwrap().title, "Forms");

        app.update(Message::SelectDown).unwrap();
        assert_eq!(app.selected_topic(), Some("philosophy"));

        app.update(Message::SelectUp).unwrap();
        assert_eq!(app.selected_topic(), Some("rust"));
        assert_eq!(app.opened().unwrap().title, "Ownership");
    }

    #[test]
    fn enter_reloads_latest_for_selected_topic() {
        let (store, dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        assert_eq!(app.opened().unwrap().title, "Ownership");

        let mut newer = Note::new("rust", "Even newer", "fresh");
        newer.created = Utc.with_ymd_and_hms(2026, 9, 8, 0, 0, 0).unwrap();
        newer.updated = newer.created;
        Store::open(dir.path()).unwrap().save(&mut newer).unwrap();

        app.update(Message::OpenSelected).unwrap();
        assert_eq!(app.opened().unwrap().title, "Even newer");
        assert_eq!(app.opened().unwrap().body, "fresh");
    }

    #[test]
    fn empty_store_has_empty_panes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut app = App::new(store).unwrap();
        assert!(app.topics().is_empty());
        assert!(app.opened().is_none());
        assert_eq!(app.selected_topic(), None);

        app.update(Message::SelectDown).unwrap();
        app.update(Message::SelectUp).unwrap();
        app.update(Message::OpenSelected).unwrap();
        app.update(Message::EnterInsert).unwrap();
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.opened().is_none());
    }

    #[test]
    fn q_quits_without_prompt() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::Quit).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn topic_with_no_notes_opens_empty_right_pane() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        std::fs::create_dir_all(store.notes_dir().join("empty")).unwrap();
        let mut app = App::new(store).unwrap();
        assert_eq!(app.topics(), &["empty"]);
        assert!(app.opened().is_none());
        app.update(Message::OpenSelected).unwrap();
        assert!(app.opened().is_none());
    }

    #[test]
    fn i_and_a_enter_insert_when_a_note_is_open() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::EnterInsert).unwrap();
        assert_eq!(app.input_mode(), InputMode::Insert);
        assert_eq!(app.cursor(), 0);

        app.update(Message::Escape).unwrap();
        assert_eq!(app.input_mode(), InputMode::Normal);

        app.update(Message::EnterInsertAppend).unwrap();
        assert_eq!(app.input_mode(), InputMode::Insert);
        assert_eq!(app.cursor(), editor::char_len("borrow checker"));
    }

    #[test]
    fn insert_edits_body_word_count_and_esc_saves() {
        let (store, dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        let words_before = app.opened().unwrap().word_count();
        app.update(Message::EnterInsertAppend).unwrap();
        app.update(Message::InsertChar(' ')).unwrap();
        app.update(Message::InsertChar('x')).unwrap();
        assert!(app.is_dirty());
        assert_eq!(app.opened().unwrap().word_count(), words_before + 1);
        assert!(app.opened().unwrap().body.ends_with(" x"));

        app.update(Message::Escape).unwrap();
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(!app.is_dirty());

        let path = dir.path().join("rust").join("2026-09-07-ownership.md");
        let loaded = Store::open(dir.path()).unwrap().load(&path).unwrap();
        assert!(loaded.body.ends_with(" x"));
    }

    #[test]
    fn ctrl_s_saves_without_quit_and_q_saves_then_quits() {
        let (store, dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::EnterInsert).unwrap();
        app.update(Message::InsertChar('z')).unwrap();
        app.update(Message::Save).unwrap();
        assert!(!app.is_dirty());
        assert!(!app.should_quit);
        assert_eq!(app.input_mode(), InputMode::Insert);

        let path = dir.path().join("rust").join("2026-09-07-ownership.md");
        let loaded = Store::open(dir.path()).unwrap().load(&path).unwrap();
        assert!(loaded.body.starts_with('z'));

        app.update(Message::InsertChar('y')).unwrap();
        assert!(app.is_dirty());
        app.update(Message::Escape).unwrap();
        app.update(Message::Quit).unwrap();
        assert!(app.should_quit);
        assert!(!app.is_dirty());
        let loaded = Store::open(dir.path()).unwrap().load(&path).unwrap();
        assert!(loaded.body.starts_with("zy"));
    }

    #[test]
    fn utf8_edit_does_not_panic() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::EnterInsert).unwrap();
        app.update(Message::InsertChar('é')).unwrap();
        app.update(Message::InsertChar('本')).unwrap();
        app.update(Message::Backspace).unwrap();
        app.update(Message::Newline).unwrap();
        assert_eq!(app.opened().unwrap().body.chars().next(), Some('é'));
    }

    #[test]
    fn n_prompts_topic_then_title_creates_scratch_and_lands_insert() {
        let (store, dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::StartNewNote).unwrap();
        assert_eq!(app.input_mode(), InputMode::Title);
        let state = app.title_state().unwrap();
        assert_eq!(state.step, TitleStep::Topic);
        assert_eq!(state.topic, "rust");

        app.update(Message::Submit).unwrap();
        assert_eq!(app.title_state().unwrap().step, TitleStep::Title);

        for c in "New idea".chars() {
            app.update(Message::InsertChar(c)).unwrap();
        }
        app.update(Message::Submit).unwrap();

        assert_eq!(app.input_mode(), InputMode::Insert);
        assert_eq!(app.opened().unwrap().title, "New idea");
        assert_eq!(app.opened().unwrap().topic, "rust");
        assert_eq!(app.opened().unwrap().body, NEW_NOTE_STARTER);
        assert_eq!(app.cursor(), editor::char_len(NEW_NOTE_STARTER));
        assert_eq!(app.opened().unwrap().status.to_string(), "scratch");
        assert!(app.topics().contains(&"rust".to_string()));

        let expected = dir
            .path()
            .join("rust")
            .join(format!("{}-new-idea.md", Utc::now().format("%Y-%m-%d")));
        assert!(
            expected.exists(),
            "expected note at {}, dir has {:?}",
            expected.display(),
            fs::read_dir(dir.path().join("rust"))
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn n_can_create_a_new_topic_and_esc_cancels() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::StartNewNote).unwrap();
        while app.title_state().unwrap().cursor > 0 {
            app.update(Message::Backspace).unwrap();
        }
        for c in "math".chars() {
            app.update(Message::InsertChar(c)).unwrap();
        }
        app.update(Message::Submit).unwrap();
        for c in "Sets".chars() {
            app.update(Message::InsertChar(c)).unwrap();
        }
        app.update(Message::Submit).unwrap();
        assert_eq!(app.selected_topic(), Some("math"));
        assert_eq!(app.opened().unwrap().title, "Sets");
        assert_eq!(app.topics()[0], "math");

        app.update(Message::Escape).unwrap();
        app.update(Message::StartNewNote).unwrap();
        app.update(Message::Escape).unwrap();
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert_eq!(app.opened().unwrap().title, "Sets");
    }

    #[test]
    fn invalid_topic_stays_on_prompt() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::StartNewNote).unwrap();
        while app.title_state().unwrap().cursor > 0 {
            app.update(Message::Backspace).unwrap();
        }
        app.update(Message::InsertChar('.')).unwrap();
        app.update(Message::Submit).unwrap();
        assert_eq!(app.input_mode(), InputMode::Title);
        assert_eq!(app.title_state().unwrap().step, TitleStep::Topic);
        assert!(app.title_state().unwrap().error.is_some());
    }

    #[test]
    fn autosave_writes_after_interval() {
        let (store, dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::EnterInsertAppend).unwrap();
        app.update(Message::InsertChar('!')).unwrap();
        assert!(app.is_dirty());
        app.dirty_since = Some(Instant::now() - AUTOSAVE_AFTER - Duration::from_secs(1));
        app.maybe_autosave().unwrap();
        assert!(!app.is_dirty());
        let path = dir.path().join("rust").join("2026-09-07-ownership.md");
        let body = Store::open(dir.path()).unwrap().load(&path).unwrap().body;
        assert!(body.ends_with('!'));
    }

    #[test]
    fn t_toggles_thread_notes_chronological_newest_at_bottom() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        assert_eq!(app.left_pane(), LeftPane::Topics);

        app.update(Message::ToggleThread).unwrap();
        assert_eq!(app.left_pane(), LeftPane::Notes);
        let titles: Vec<_> = app
            .thread_notes()
            .iter()
            .map(|n| n.title.as_str())
            .collect();
        assert_eq!(titles, ["Old", "Ownership"]);
        assert_eq!(app.thread_notes()[app.left_selected()].title, "Ownership");

        app.update(Message::SelectUp).unwrap();
        assert_eq!(app.thread_notes()[app.left_selected()].title, "Old");
        assert_eq!(app.opened().unwrap().title, "Ownership");

        app.update(Message::OpenSelected).unwrap();
        assert_eq!(app.opened().unwrap().title, "Old");

        app.update(Message::ToggleThread).unwrap();
        assert_eq!(app.left_pane(), LeftPane::Topics);
        assert_eq!(app.selected_topic(), Some("rust"));
        assert_eq!(app.opened().unwrap().title, "Old");
    }

    #[test]
    fn enter_on_topic_row_still_opens_latest() {
        let (store, dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        let mut newer = Note::new("rust", "Even newer", "fresh");
        newer.created = Utc.with_ymd_and_hms(2026, 9, 8, 0, 0, 0).unwrap();
        newer.updated = newer.created;
        Store::open(dir.path()).unwrap().save(&mut newer).unwrap();
        app.update(Message::OpenSelected).unwrap();
        assert_eq!(app.opened().unwrap().title, "Even newer");
    }

    #[test]
    fn w_toggles_scratch_keep_and_persists() {
        let (store, dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        assert_eq!(app.opened().unwrap().status, Status::Scratch);

        app.update(Message::ToggleStatus).unwrap();
        assert_eq!(app.opened().unwrap().status, Status::Keep);
        let path = dir.path().join("rust").join("2026-09-07-ownership.md");
        let loaded = Store::open(dir.path()).unwrap().load(&path).unwrap();
        assert_eq!(loaded.status, Status::Keep);

        app.update(Message::ToggleStatus).unwrap();
        assert_eq!(app.opened().unwrap().status, Status::Scratch);
        let loaded = Store::open(dir.path()).unwrap().load(&path).unwrap();
        assert_eq!(loaded.status, Status::Scratch);
    }

    #[test]
    fn p_appends_quote_from_last_keep_and_enters_insert() {
        let (_store, dir) = seeded_store();
        let mut keep = Note::new("rust", "Decision", "line1\nline2\nkeep body");
        keep.status = Status::Keep;
        keep.created = Utc.with_ymd_and_hms(2026, 9, 2, 0, 0, 0).unwrap();
        keep.updated = keep.created;
        Store::open(dir.path()).unwrap().save(&mut keep).unwrap();

        let mut app = App::new(Store::open(dir.path()).unwrap()).unwrap();
        assert_eq!(app.opened().unwrap().title, "Ownership");
        app.update(Message::PullQuote).unwrap();
        assert_eq!(app.input_mode(), InputMode::Insert);
        assert!(app.is_dirty());
        let body = &app.opened().unwrap().body;
        assert!(body.contains("> 2026-09-02"));
        assert!(body.contains("> keep body"));
        assert!(body.contains("> line1"));
        assert!(body.starts_with("borrow checker"));
    }

    #[test]
    fn p_falls_back_to_latest_when_no_keep() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::PullQuote).unwrap();
        let body = &app.opened().unwrap().body;
        assert!(body.contains("> 2026-09-07"));
        assert!(body.contains("> borrow checker"));
        assert_eq!(app.input_mode(), InputMode::Insert);
    }

    #[test]
    fn slash_search_filters_and_enter_opens_esc_cancels() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::StartSearch).unwrap();
        assert_eq!(app.input_mode(), InputMode::Search);
        assert_eq!(app.search_hits().len(), 3);

        for c in "forms".chars() {
            app.update(Message::InsertChar(c)).unwrap();
        }
        assert_eq!(app.search_query(), "forms");
        assert_eq!(app.search_hits().len(), 1);
        assert_eq!(app.search_hits()[0].title, "Forms");

        app.update(Message::Submit).unwrap();
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert_eq!(app.opened().unwrap().title, "Forms");
        assert_eq!(app.selected_topic(), Some("philosophy"));

        app.update(Message::StartSearch).unwrap();
        app.update(Message::InsertChar('x')).unwrap();
        assert!(app.search_hits().is_empty());
        app.update(Message::Escape).unwrap();
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert_eq!(app.opened().unwrap().title, "Forms");
    }

    #[test]
    fn search_substring_is_case_insensitive_on_body() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::StartSearch).unwrap();
        for c in "BORROW".chars() {
            app.update(Message::InsertChar(c)).unwrap();
        }
        assert_eq!(app.search_hits().len(), 1);
        assert_eq!(app.search_hits()[0].title, "Ownership");
    }

    #[test]
    fn search_up_down_picks_among_hits() {
        let (store, _dir) = seeded_store();
        let mut app = App::new(store).unwrap();
        app.update(Message::StartSearch).unwrap();
        assert!(app.search_hits().len() >= 2);
        let first = app.search_hits()[0].title.clone();
        let second = app.search_hits()[1].title.clone();
        app.update(Message::SelectDown).unwrap();
        app.update(Message::Submit).unwrap();
        assert_eq!(app.opened().unwrap().title, second);
        assert_ne!(first, second);
    }

    #[test]
    fn hide_scratch_in_thread_when_config_says_so() {
        let (_store, dir) = seeded_store();
        let mut keep = Note::new("rust", "Kept", "kept body");
        keep.status = Status::Keep;
        keep.created = Utc.with_ymd_and_hms(2026, 9, 3, 0, 0, 0).unwrap();
        keep.updated = keep.created;
        Store::open(dir.path()).unwrap().save(&mut keep).unwrap();

        let mut cfg = Config::default();
        cfg.show_scratch_in_thread = false;
        let mut app = App::with_config(Store::open(dir.path()).unwrap(), cfg).unwrap();
        app.update(Message::ToggleThread).unwrap();
        let titles: Vec<_> = app
            .thread_notes()
            .iter()
            .map(|n| n.title.as_str())
            .collect();
        assert_eq!(titles, ["Kept"]);
        assert!(!titles.contains(&"Ownership"));
        assert!(!titles.contains(&"Old"));
    }
}
