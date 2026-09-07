//! Defaults for config and notes directories.
//!
//! A `config.toml` is not loaded yet (see DECISIONS.md). `$THREAD_HOME` overrides
//! both the notes directory and the future config directory.

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
        Self::from_dirs(
            env::var_os("THREAD_HOME").map(PathBuf::from),
            dirs::home_dir(),
            dirs::config_dir(),
        )
    }

    fn from_dirs(
        thread_home: Option<PathBuf>,
        home: Option<PathBuf>,
        xdg_config: Option<PathBuf>,
    ) -> Self {
        if let Some(thread_home) = thread_home {
            return Self {
                notes_dir: thread_home.clone(),
                config_dir: thread_home,
            };
        }

        let notes_dir = home
            .map(|home| home.join("Documents").join("thread"))
            .unwrap_or_else(|| PathBuf::from("thread"));
        let config_dir = xdg_config
            .unwrap_or_else(|| PathBuf::from("."))
            .join("thread");

        Self {
            notes_dir,
            config_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_notes_dir_is_documents_thread() {
        let cfg = Config::from_dirs(
            None,
            Some(PathBuf::from("/home/ada")),
            Some(PathBuf::from("/home/ada/.config")),
        );
        assert_eq!(cfg.notes_dir, Path::new("/home/ada/Documents/thread"));
        assert_eq!(cfg.config_dir, Path::new("/home/ada/.config/thread"));
    }

    #[test]
    fn thread_home_overrides_notes_dir() {
        let cfg = Config::from_dirs(Some(PathBuf::from("/tmp/thread-home")), None, None);
        assert_eq!(cfg.notes_dir, Path::new("/tmp/thread-home"));
        assert_eq!(cfg.config_dir, Path::new("/tmp/thread-home"));
    }
}
