//! Filesystem store for topic-scoped notes.

use crate::error::{Error, Result};
use crate::note::{slugify, Note};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Store {
    notes_dir: PathBuf,
}

impl Store {
    /// Create `notes_dir` if needed and return a store rooted there.
    pub fn open(notes_dir: impl Into<PathBuf>) -> Result<Self> {
        let notes_dir = notes_dir.into();
        fs::create_dir_all(&notes_dir)?;
        Ok(Self { notes_dir })
    }

    pub fn notes_dir(&self) -> &Path {
        &self.notes_dir
    }

    /// Unique topic directory names, most recently updated first.
    ///
    /// Recency is the latest note's `updated` timestamp in that topic. Directories
    /// with no readable notes sort last, then by name.
    pub fn list_topics(&self) -> Result<Vec<String>> {
        if !self.notes_dir.exists() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in fs::read_dir(&self.notes_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            names.push(name.to_string());
        }

        let mut scored: Vec<(Option<DateTime<Utc>>, String)> = Vec::with_capacity(names.len());
        for name in names {
            let latest = self.latest(&name)?;
            scored.push((latest.map(|note| note.updated), name));
        }
        scored.sort_by(|a, b| match (&a.0, &b.0) {
            (Some(ta), Some(tb)) => tb.cmp(ta).then_with(|| a.1.cmp(&b.1)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.1.cmp(&b.1),
        });
        Ok(scored.into_iter().map(|(_, name)| name).collect())
    }

    pub fn list_notes(&self, topic: &str) -> Result<Vec<Note>> {
        validate_topic(topic)?;
        let dir = self.notes_dir.join(topic);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut notes = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            if let Ok(note) = self.load(&path) {
                notes.push(note);
            }
        }
        notes.sort_by(|a, b| {
            b.created
                .cmp(&a.created)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(notes)
    }

    pub fn load(&self, path: impl AsRef<Path>) -> Result<Note> {
        let path = self.resolve(path.as_ref());
        match fs::read_to_string(&path) {
            Ok(raw) => Note::from_markdown(Some(path), &raw),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(Error::NotFound(path)),
            Err(err) => Err(err.into()),
        }
    }

    /// Write `note` to disk. New notes get
    /// `{notes_dir}/{topic}/{yyyy-mm-dd}-{slug}.md`; existing `path` is kept.
    pub fn save(&self, note: &mut Note) -> Result<PathBuf> {
        validate_topic(&note.topic)?;
        fs::create_dir_all(&self.notes_dir)?;
        if note.path.is_none() {
            note.path = Some(self.allocate_path(note)?);
        }
        let path = note.path.clone().expect("path assigned above");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let markdown = note.to_markdown()?;
        let tmp = path.with_extension("md.tmp");
        fs::write(&tmp, markdown)?;
        fs::rename(&tmp, &path)?;
        Ok(path)
    }

    pub fn search(&self, query: &str) -> Result<Vec<Note>> {
        let mut found = Vec::new();
        for topic in self.list_topics()? {
            for note in self.list_notes(&topic)? {
                if note.matches_query(query) {
                    found.push(note);
                }
            }
        }
        found.sort_by(|a, b| {
            b.updated
                .cmp(&a.updated)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(found)
    }

    pub fn latest(&self, topic: &str) -> Result<Option<Note>> {
        let mut notes = self.list_notes(topic)?;
        notes.sort_by(|a, b| {
            b.updated
                .cmp(&a.updated)
                .then_with(|| b.created.cmp(&a.created))
        });
        Ok(notes.into_iter().next())
    }

    fn allocate_path(&self, note: &Note) -> Result<PathBuf> {
        let dir = self.notes_dir.join(&note.topic);
        fs::create_dir_all(&dir)?;
        let date = note.created.format("%Y-%m-%d");
        let slug = slugify(&note.title);
        let mut candidate = dir.join(format!("{date}-{slug}.md"));
        let mut n = 2u32;
        while candidate.exists() {
            candidate = dir.join(format!("{date}-{slug}-{n}.md"));
            n += 1;
        }
        Ok(candidate)
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.notes_dir.join(path)
        }
    }
}

pub(crate) fn validate_topic(topic: &str) -> Result<()> {
    if topic.is_empty()
        || topic == "."
        || topic == ".."
        || topic.starts_with('.')
        || topic.contains('/')
        || topic.contains('\\')
        || topic.contains('\0')
    {
        return Err(Error::InvalidTopic(topic.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Status;
    use chrono::{TimeZone, Utc};

    fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        (store, dir)
    }

    fn dated(topic: &str, title: &str, body: &str, y: i32, m: u32, d: u32, hour: u32) -> Note {
        let ts = Utc.with_ymd_and_hms(y, m, d, hour, 0, 0).unwrap();
        Note {
            path: None,
            topic: topic.into(),
            title: title.into(),
            status: Status::Scratch,
            created: ts,
            updated: ts,
            parent: None,
            body: body.into(),
        }
    }

    #[test]
    fn open_creates_notes_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes");
        assert!(!path.exists());
        Store::open(&path).unwrap();
        assert!(path.is_dir());
    }

    #[test]
    fn save_load_roundtrip_uses_topic_date_slug_path() {
        let (store, _dir) = store();
        let mut note = dated("rust", "Ownership notes", "hello", 2026, 9, 7, 12);
        let path = store.save(&mut note).unwrap();
        assert_eq!(
            path.strip_prefix(store.notes_dir()).unwrap(),
            Path::new("rust/2026-09-07-ownership-notes.md")
        );
        let loaded = store.load(&path).unwrap();
        assert_eq!(loaded.topic, "rust");
        assert_eq!(loaded.title, "Ownership notes");
        assert_eq!(loaded.body, "hello");
        assert_eq!(loaded.status, Status::Scratch);
    }

    #[test]
    fn list_topics_and_list_notes() {
        let (store, _dir) = store();
        store
            .save(&mut dated("rust", "A", "a", 2026, 9, 1, 10))
            .unwrap();
        store
            .save(&mut dated("rust", "B", "b", 2026, 9, 2, 10))
            .unwrap();
        store
            .save(&mut dated("philosophy", "Forms", "c", 2026, 8, 1, 10))
            .unwrap();

        assert_eq!(store.list_topics().unwrap(), vec!["rust", "philosophy"]);

        let rust_notes = store.list_notes("rust").unwrap();
        assert_eq!(rust_notes.len(), 2);
        assert_eq!(rust_notes[0].title, "B");
        assert_eq!(rust_notes[1].title, "A");

        let philosophy = store.list_notes("philosophy").unwrap();
        assert_eq!(philosophy.len(), 1);
        assert_eq!(philosophy[0].title, "Forms");

        assert!(store.list_notes("missing").unwrap().is_empty());
    }

    #[test]
    fn search_is_case_insensitive_substring() {
        let (store, _dir) = store();
        store
            .save(&mut dated(
                "rust",
                "Borrow checker",
                "See NLL",
                2026,
                9,
                7,
                1,
            ))
            .unwrap();
        store
            .save(&mut Note::new("math", "Sets", "unrelated"))
            .unwrap();

        let hits = store.search("nll").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Borrow checker");

        let title_hits = store.search("BORROW").unwrap();
        assert_eq!(title_hits.len(), 1);
    }

    #[test]
    fn list_topics_orders_by_most_recently_updated() {
        let (store, _dir) = store();
        store
            .save(&mut dated("alpha", "A", "a", 2026, 9, 1, 10))
            .unwrap();
        let mut beta = dated("beta", "B", "b", 2026, 8, 1, 10);
        beta.updated = Utc.with_ymd_and_hms(2026, 9, 8, 0, 0, 0).unwrap();
        store.save(&mut beta).unwrap();
        fs::create_dir_all(store.notes_dir().join("empty")).unwrap();

        assert_eq!(store.list_topics().unwrap(), vec!["beta", "alpha", "empty"]);
    }

    #[test]
    fn latest_prefers_most_recently_updated() {
        let (store, _dir) = store();
        let mut older = dated("rust", "Old", "a", 2026, 9, 1, 10);
        let mut newer = dated("rust", "New", "b", 2026, 9, 1, 11);
        newer.updated = Utc.with_ymd_and_hms(2026, 9, 8, 0, 0, 0).unwrap();
        store.save(&mut older).unwrap();
        store.save(&mut newer).unwrap();
        let latest = store.latest("rust").unwrap().unwrap();
        assert_eq!(latest.title, "New");
        assert!(store.latest("empty").unwrap().is_none());
    }

    #[test]
    fn save_uniquifies_slug_collisions() {
        let (store, _dir) = store();
        let mut a = dated("rust", "Same Title", "one", 2026, 9, 7, 12);
        let mut b = dated("rust", "Same Title", "two", 2026, 9, 7, 12);
        store.save(&mut a).unwrap();
        store.save(&mut b).unwrap();
        assert!(a.path.unwrap().ends_with("2026-09-07-same-title.md"));
        assert!(b.path.unwrap().ends_with("2026-09-07-same-title-2.md"));
    }

    #[test]
    fn load_accepts_path_relative_to_notes_dir() {
        let (store, _dir) = store();
        let mut note = dated("rust", "Rel", "body", 2026, 9, 7, 12);
        store.save(&mut note).unwrap();
        let loaded = store.load(Path::new("rust/2026-09-07-rel.md")).unwrap();
        assert_eq!(loaded.title, "Rel");
    }

    #[test]
    fn list_and_load_plain_markdown_without_front_matter() {
        let (store, _dir) = store();
        let dir = store.notes_dir().join("rust");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("loose-ideas.md");
        fs::write(&path, "# no fence\njust a body\n").unwrap();

        let loaded = store.load(&path).unwrap();
        assert_eq!(loaded.topic, "rust");
        assert_eq!(loaded.title, "loose-ideas.md");
        assert_eq!(loaded.body, "# no fence\njust a body\n");

        let listed = store.list_notes("rust").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "loose-ideas.md");
    }
}
