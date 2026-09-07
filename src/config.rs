//! Defaults for config and notes directories.
//!
//! `$THREAD_HOME` overrides both the notes directory and the config directory.
//! If `config.toml` exists there (or in `~/.config/thread`), M4–M5 read
//! `show_scratch_in_thread` and `session_minutes`. Other keys are ignored.
//! The file is never created.

use std::env;
use std::fs;
use std::path::PathBuf;

/// BUILD_SPEC default session length when `session_minutes` is missing.
pub const DEFAULT_SESSION_MINUTES: u64 = 20;

#[derive(Debug, Clone)]
pub struct Config {
    pub notes_dir: PathBuf,
    /// Directory for `config.toml` (`$THREAD_HOME` or `~/.config/thread`).
    pub config_dir: PathBuf,
    /// When false, thread-detail (`t`) hides scratch notes. BUILD_SPEC default true.
    pub show_scratch_in_thread: bool,
    /// Session countdown length for `s`. BUILD_SPEC default 20.
    pub session_minutes: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notes_dir: PathBuf::from("thread"),
            config_dir: PathBuf::from("."),
            show_scratch_in_thread: true,
            session_minutes: DEFAULT_SESSION_MINUTES,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        Self::load_from_dirs(
            env::var_os("THREAD_HOME").map(PathBuf::from),
            dirs::home_dir(),
            dirs::config_dir(),
        )
    }

    fn load_from_dirs(
        thread_home: Option<PathBuf>,
        home: Option<PathBuf>,
        xdg_config: Option<PathBuf>,
    ) -> Self {
        let mut cfg = Self::from_dirs(thread_home, home, xdg_config);
        cfg.apply_config_file();
        cfg
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
                show_scratch_in_thread: true,
                session_minutes: DEFAULT_SESSION_MINUTES,
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
            show_scratch_in_thread: true,
            session_minutes: DEFAULT_SESSION_MINUTES,
        }
    }

    fn apply_config_file(&mut self) {
        let path = self.config_dir.join("config.toml");
        let Ok(text) = fs::read_to_string(&path) else {
            return;
        };
        if let Some(value) = parse_toml_bool(&text, "show_scratch_in_thread") {
            self.show_scratch_in_thread = value;
        }
        if let Some(value) = parse_toml_u64(&text, "session_minutes") {
            if value > 0 {
                self.session_minutes = value;
            }
        }
    }
}

/// Tiny TOML-ish scan for one boolean. Missing key → `None` (caller keeps default).
fn parse_toml_bool(text: &str, key: &str) -> Option<bool> {
    match toml_value(text, key)?.as_str() {
        "true" | "True" | "TRUE" | "1" => Some(true),
        "false" | "False" | "FALSE" | "0" => Some(false),
        _ => None,
    }
}

/// Tiny TOML-ish scan for one integer. Missing / unparsable → `None`.
fn parse_toml_u64(text: &str, key: &str) -> Option<u64> {
    toml_value(text, key)?.parse().ok()
}

fn toml_value(text: &str, key: &str) -> Option<String> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_end_matches(',').trim();
        let rest = rest.trim_matches('"').trim_matches('\'').trim();
        return Some(rest.to_string());
    }
    None
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
        assert!(cfg.show_scratch_in_thread);
        assert_eq!(cfg.session_minutes, DEFAULT_SESSION_MINUTES);
    }

    #[test]
    fn thread_home_overrides_notes_dir() {
        let cfg = Config::from_dirs(Some(PathBuf::from("/tmp/thread-home")), None, None);
        assert_eq!(cfg.notes_dir, Path::new("/tmp/thread-home"));
        assert_eq!(cfg.config_dir, Path::new("/tmp/thread-home"));
        assert!(cfg.show_scratch_in_thread);
        assert_eq!(cfg.session_minutes, DEFAULT_SESSION_MINUTES);
    }

    #[test]
    fn missing_config_file_keeps_scratch_visible() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load_from_dirs(Some(dir.path().to_path_buf()), None, None);
        assert!(cfg.show_scratch_in_thread);
        assert_eq!(cfg.session_minutes, DEFAULT_SESSION_MINUTES);
    }

    #[test]
    fn config_file_can_hide_scratch_in_thread() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.toml"),
            "# comment\nshow_scratch_in_thread = false\n",
        )
        .unwrap();
        let cfg = Config::load_from_dirs(Some(dir.path().to_path_buf()), None, None);
        assert!(!cfg.show_scratch_in_thread);
    }

    #[test]
    fn parse_show_scratch_accepts_bare_and_quoted() {
        assert_eq!(
            parse_toml_bool("show_scratch_in_thread = true", "show_scratch_in_thread"),
            Some(true)
        );
        assert_eq!(
            parse_toml_bool("show_scratch_in_thread=false", "show_scratch_in_thread"),
            Some(false)
        );
        assert_eq!(
            parse_toml_bool(
                "show_scratch_in_thread = \"false\"",
                "show_scratch_in_thread"
            ),
            Some(false)
        );
        assert_eq!(
            parse_toml_bool("unrelated = 1\n", "show_scratch_in_thread"),
            None
        );
    }

    #[test]
    fn config_file_sets_session_minutes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("config.toml"),
            "show_scratch_in_thread = true\nsession_minutes = 5\n",
        )
        .unwrap();
        let cfg = Config::load_from_dirs(Some(dir.path().to_path_buf()), None, None);
        assert_eq!(cfg.session_minutes, 5);
        assert!(cfg.show_scratch_in_thread);
    }

    #[test]
    fn zero_or_invalid_session_minutes_keeps_default() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.toml"), "session_minutes = 0\n").unwrap();
        let cfg = Config::load_from_dirs(Some(dir.path().to_path_buf()), None, None);
        assert_eq!(cfg.session_minutes, DEFAULT_SESSION_MINUTES);

        fs::write(dir.path().join("config.toml"), "session_minutes = nope\n").unwrap();
        let cfg = Config::load_from_dirs(Some(dir.path().to_path_buf()), None, None);
        assert_eq!(cfg.session_minutes, DEFAULT_SESSION_MINUTES);
    }

    #[test]
    fn parse_session_minutes_accepts_bare_and_quoted() {
        assert_eq!(
            parse_toml_u64("session_minutes = 20", "session_minutes"),
            Some(20)
        );
        assert_eq!(
            parse_toml_u64("session_minutes=1", "session_minutes"),
            Some(1)
        );
        assert_eq!(
            parse_toml_u64("session_minutes = \"15\"", "session_minutes"),
            Some(15)
        );
        assert_eq!(parse_toml_u64("unrelated = 1\n", "session_minutes"), None);
    }
}
