//! Thinking Log TUI library: note store plus a two-pane Ratatui app.

pub mod app;
pub mod config;
pub mod editor;
pub mod error;
pub mod keys;
pub mod note;
pub mod store;
pub mod ui;

use config::Config;
use error::Result;
use store::Store;

/// Create the notes directory if needed and run the TUI.
pub fn run() -> Result<()> {
    let config = Config::load();
    let store = Store::open(config.notes_dir.clone())?;
    app::App::with_config(store, config)?.run()
}
