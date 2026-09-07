//! Application model, update, and terminal lifecycle (alt screen + raw mode).

use crate::error::Result;
use crate::keys::{self, Message};
use crate::note::Note;
use crate::store::Store;
use crate::ui;
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};
use std::io;

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

/// Elm-style model for the M2 Normal-mode TUI.
#[derive(Debug)]
pub struct App {
    store: Store,
    topics: Vec<String>,
    selected: usize,
    opened: Option<Note>,
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

    pub fn run(mut self) -> Result<()> {
        let mut terminal = TerminalGuard::enter();
        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, &self))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if let Event::Key(key) = event::read()? {
            if let Some(message) = keys::message_from_key(key) {
                self.update(message)?;
            }
        }
        Ok(())
    }

    pub fn update(&mut self, message: Message) -> Result<()> {
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
        self.opened = match self.selected_topic() {
            Some(topic) => self.store.latest(topic)?,
            None => None,
        };
        Ok(())
    }

    /// Persist a dirty buffer if one exists. M2 is read-only, so this is a no-op.
    fn save_if_needed(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;
    use chrono::{TimeZone, Utc};

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
}
