//! Thinking Log TUI library: note store plus a thin Ratatui app.

pub mod app;
pub mod config;
pub mod error;
pub mod keys;
pub mod note;
pub mod store;
pub mod ui;

use config::Config;
use error::Result;
use store::Store;

/// Create the notes directory if needed and run the M0 TUI.
pub fn run() -> Result<()> {
    let config = Config::load();
    Store::open(config.notes_dir)?;
    app::App::new().run()
}
