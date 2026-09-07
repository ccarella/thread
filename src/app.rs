//! Application model, update, and terminal lifecycle (alt screen + raw mode).

use crate::editor;
use crate::error::Result;
use crate::keys::{self, InputMode, Message};
use crate::note::Note;
use crate::store::{self, Store};
use crate::ui;
use chrono::Utc;
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};
use std::io;
use std::time::{Duration, Instant};

/// Body of a note created with `n` (cursor lands after this in Insert).
pub const NEW_NOTE_STARTER: &str = "What am I trying to decide?\n\n";

/// Autosave a dirty buffer this often (BUILD_SPEC §7).
pub const AUTOSAVE_AFTER: Duration = Duration::from_secs(30);

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

#[derive(Debug, Clone)]
enum Mode {
    Normal,
    Insert,
    Title(TitleState),
}

/// Elm-style model for the two-pane TUI with Insert / Title editing.
#[derive(Debug)]
pub struct App {
    store: Store,
    topics: Vec<String>,
    selected: usize,
    opened: Option<Note>,
    mode: Mode,
    cursor: usize,
    dirty: bool,
    dirty_since: Option<Instant>,
    pub should_quit: bool,
}

impl App {
    pub fn new(store: Store) -> Result<Self> {
        let topics = store.list_topics()?;
        let mut app = Self {
            store,
            topics,
            selected: 0,
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

    pub fn opened(&self) -> Option<&Note> {
        self.opened.as_ref()
    }

    pub fn input_mode(&self) -> InputMode {
        match self.mode {
            Mode::Normal => InputMode::Normal,
            Mode::Insert => InputMode::Insert,
            Mode::Title(_) => InputMode::Title,
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
            Message::TopicDown => {
                self.move_selection(1);
                self.open_latest()?;
            }
            Message::TopicUp => {
                self.move_selection(-1);
                self.open_latest()?;
            }
            Message::OpenLatest => self.open_latest()?,
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

    fn open_latest(&mut self) -> Result<()> {
        self.save_if_needed()?;
        self.opened = match self.selected_topic() {
            Some(topic) => self.store.latest(topic)?,
            None => None,
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
    use crate::note::Note;
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

        app.update(Message::TopicUp).unwrap();
        assert_eq!(app.selected_topic(), Some("rust"));

        app.update(Message::TopicDown).unwrap();
        assert_eq!(app.selected_topic(), Some("philosophy"));
        assert_eq!(app.opened().unwrap().title, "Forms");

        app.update(Message::TopicDown).unwrap();
        assert_eq!(app.selected_topic(), Some("philosophy"));

        app.update(Message::TopicUp).unwrap();
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

        app.update(Message::OpenLatest).unwrap();
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

        app.update(Message::TopicDown).unwrap();
        app.update(Message::TopicUp).unwrap();
        app.update(Message::OpenLatest).unwrap();
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
        app.update(Message::OpenLatest).unwrap();
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
}
