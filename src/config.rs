//! Defaults for config and notes directories.
//!
//! `$THREAD_HOME` overrides both the notes directory and the config directory.
//! If `config.toml` exists there (or in `~/.config/thread`), M4 reads
//! `show_scratch_in_thread`. Other keys are ignored. The file is never created.

use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub notes_dir: PathBuf,
    /// Directory for `config.toml` (`$THREAD_HOME` or `~/.config/thread`).
    pub config_dir: PathBuf,
    /// When false, thread-detail (`t`) hides scratch notes. BUILD_SPEC default true.
    pub show_scratch_in_thread: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            notes_dir: PathBuf::from("thread"),
            config_dir: PathBuf::from("."),
            show_scratch_in_thread: true,
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
        }
    }

    fn apply_config_file(&mut self) {
        let path = self.config_dir.join("config.toml");
        let Ok(text) = fs::read_to_string(&path) else {
            return;
        };
        if let Some(value) = parse_show_scratch_in_thread(&text) {
            self.show_scratch_in_thread = value;
        }
    }
}

/// Tiny TOML-ish scan for one boolean. Missing key → `None` (caller keeps default).
fn parse_show_scratch_in_thread(text: &str) -> Option<bool> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let rest = line.strip_prefix("show_scratch_in_thread")?.trim();
        let rest = rest.strip_prefix('=')?.trim();
        let rest = rest.trim_end_matches(',').trim();
        let rest = rest.trim_matches('"').trim_matches('\'').trim();
        return match rest {
            "true" | "True" | "TRUE" | "1" => Some(true),
            "false" | "False" | "FALSE" | "0" => Some(false),
            _ => None,
        };
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
    }

    #[test]
    fn thread_home_overrides_notes_dir() {
        let cfg = Config::from_dirs(Some(PathBuf::from("/tmp/thread-home")), None, None);
        assert_eq!(cfg.notes_dir, Path::new("/tmp/thread-home"));
        assert_eq!(cfg.config_dir, Path::new("/tmp/thread-home"));
        assert!(cfg.show_scratch_in_thread);
    }

    #[test]
    fn missing_config_file_keeps_scratch_visible() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load_from_dirs(Some(dir.path().to_path_buf()), None, None);
        assert!(cfg.show_scratch_in_thread);
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
            parse_show_scratch_in_thread("show_scratch_in_thread = true"),
            Some(true)
        );
        assert_eq!(
            parse_show_scratch_in_thread("show_scratch_in_thread=false"),
            Some(false)
        );
        assert_eq!(
            parse_show_scratch_in_thread("show_scratch_in_thread = \"false\""),
            Some(false)
        );
        assert_eq!(parse_show_scratch_in_thread("unrelated = 1\n"), None);
    }
}
