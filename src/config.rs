//! Defaults for config and notes directories.
//!
//! A `config.toml` is not loaded yet (see DECISIONS.md). `$THREAD_HOME` still
//! overrides the config directory so later milestones can read it from one place.

use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub notes_dir: PathBuf,
    /// Directory for a future `config.toml` (`$THREAD_HOME` or `~/.config/thread`).
    pub config_dir: PathBuf,
}

impl Config {
    pub fn load() -> Self {
        let config_dir = match env::var_os("THREAD_HOME") {
            Some(path) => PathBuf::from(path),
            None => dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("thread"),
        };

        let notes_dir = dirs::home_dir()
            .map(|home| home.join("Documents").join("thread"))
            .unwrap_or_else(|| PathBuf::from("thread"));

        Self {
            notes_dir,
            config_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_notes_dir_is_documents_thread() {
        let cfg = Config::load();
        if let Some(home) = dirs::home_dir() {
            assert_eq!(cfg.notes_dir, home.join("Documents").join("thread"));
        }
        if std::env::var_os("THREAD_HOME").is_none() {
            if let Some(config) = dirs::config_dir() {
                assert_eq!(cfg.config_dir, config.join("thread"));
            }
        }
    }
}
