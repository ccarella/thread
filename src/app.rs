//! Application loop and terminal lifecycle (alt screen + raw mode).

use crate::error::Result;
use crate::keys::{self, Command};
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

pub struct App {
    pub should_quit: bool,
    pub store: Store,
}

impl App {
    pub fn new(store: Store) -> Self {
        Self {
            should_quit: false,
            store,
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
        if let Event::Key(key) = event::read()? {
            if let Some(command) = keys::command_from_key(key) {
                self.apply(command);
            }
        }
        Ok(())
    }

    pub fn apply(&mut self, command: Command) {
        match command {
            Command::Quit => self.should_quit = true,
        }
    }
}
