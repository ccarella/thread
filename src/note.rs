//! Note model and YAML front-matter parse/serialize.

use crate::error::{Error, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Scratch,
    Keep,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Scratch => "scratch",
            Status::Keep => "keep",
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FrontMatter {
    topic: String,
    title: String,
    status: Status,
    created: String,
    updated: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub path: Option<PathBuf>,
    pub topic: String,
    pub title: String,
    pub status: Status,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub parent: Option<String>,
    pub body: String,
}

impl Note {
    pub fn new(
        topic: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            path: None,
            topic: topic.into(),
            title: title.into(),
            status: Status::Scratch,
            created: now,
            updated: now,
            parent: None,
            body: body.into(),
        }
    }

    pub fn from_markdown(path: Option<PathBuf>, input: &str) -> Result<Self> {
        let (yaml, body) = split_front_matter(input)?;
        let fm: FrontMatter = serde_yaml::from_str(yaml)?;
        Ok(Self {
            path,
            topic: fm.topic,
            title: fm.title,
            status: fm.status,
            created: parse_datetime(&fm.created)?,
            updated: parse_datetime(&fm.updated)?,
            parent: fm.parent.filter(|p| !p.is_empty()),
            body: body.to_string(),
        })
    }

    pub fn to_markdown(&self) -> Result<String> {
        let fm = FrontMatter {
            topic: self.topic.clone(),
            title: self.title.clone(),
            status: self.status,
            created: self.created.to_rfc3339(),
            updated: self.updated.to_rfc3339(),
            parent: self.parent.clone(),
        };
        let yaml = serde_yaml::to_string(&fm)?;
        Ok(format!("---\n{yaml}---\n{}", self.body))
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        if q.is_empty() {
            return true;
        }
        self.title.to_lowercase().contains(&q)
            || self.body.to_lowercase().contains(&q)
            || self.topic.to_lowercase().contains(&q)
    }
}

/// ASCII kebab-case slug from a title. Empty / non-latin titles become `"note"`.
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !slug.is_empty() && !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "note".to_string()
    } else {
        slug
    }
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        let naive = date.and_hms_opt(0, 0, 0).expect("midnight is a valid time");
        return Ok(naive.and_utc());
    }
    Err(Error::InvalidDatetime(raw.to_string()))
}

fn split_front_matter(input: &str) -> Result<(&str, &str)> {
    let input = input.trim_start_matches('\u{feff}');
    let rest = input
        .strip_prefix("---\n")
        .or_else(|| input.strip_prefix("---\r\n"))
        .ok_or(Error::MissingFrontMatter)?;

    if let Some(idx) = rest.find("\n---\n") {
        return Ok((&rest[..idx], &rest[idx + "\n---\n".len()..]));
    }
    if let Some(idx) = rest.find("\n---\r\n") {
        return Ok((&rest[..idx], &rest[idx + "\n---\r\n".len()..]));
    }
    if let Some(yaml) = rest.strip_suffix("\n---") {
        return Ok((yaml, ""));
    }
    if let Some(yaml) = rest.strip_suffix("\r\n---") {
        return Ok((yaml, ""));
    }
    Err(Error::MissingFrontMatter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample() -> Note {
        Note {
            path: None,
            topic: "rust".into(),
            title: "Ownership notes".into(),
            status: Status::Scratch,
            created: Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap(),
            updated: Utc.with_ymd_and_hms(2026, 9, 7, 13, 30, 0).unwrap(),
            parent: None,
            body: "Borrow checker thoughts.\n".into(),
        }
    }

    #[test]
    fn parse_fixture() {
        let raw = "---\n\
topic: rust\n\
title: Ownership notes\n\
status: scratch\n\
created: 2026-09-07T12:00:00+00:00\n\
updated: 2026-09-07T13:30:00+00:00\n\
---\n\
Borrow checker thoughts.\n";
        let note = Note::from_markdown(None, raw).unwrap();
        assert_eq!(note, sample());
    }

    #[test]
    fn parse_accepts_date_only_and_optional_parent() {
        let raw = "---\n\
topic: philosophy\n\
title: Forms\n\
status: keep\n\
created: 2026-01-02\n\
updated: 2026-01-03\n\
parent: philosophy/2026-01-01-intro.md\n\
---\n\
body";
        let note = Note::from_markdown(None, raw).unwrap();
        assert_eq!(note.status, Status::Keep);
        assert_eq!(
            note.created,
            Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap()
        );
        assert_eq!(
            note.parent.as_deref(),
            Some("philosophy/2026-01-01-intro.md")
        );
        assert_eq!(note.body, "body");
    }

    #[test]
    fn serialize_then_parse_roundtrip() {
        let mut note = sample();
        note.parent = Some("rust/2026-09-01-start.md".into());
        note.status = Status::Keep;
        let markdown = note.to_markdown().unwrap();
        assert!(markdown.starts_with("---\n"));
        assert!(markdown.contains("status: keep"));
        assert!(markdown.contains("parent: rust/2026-09-01-start.md"));
        let parsed = Note::from_markdown(None, &markdown).unwrap();
        assert_eq!(parsed.topic, note.topic);
        assert_eq!(parsed.title, note.title);
        assert_eq!(parsed.status, note.status);
        assert_eq!(parsed.created, note.created);
        assert_eq!(parsed.updated, note.updated);
        assert_eq!(parsed.parent, note.parent);
        assert_eq!(parsed.body, note.body);
    }

    #[test]
    fn missing_front_matter_is_an_error() {
        let err = Note::from_markdown(None, "# just markdown\n").unwrap_err();
        assert!(matches!(err, Error::MissingFrontMatter));
    }

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Ownership notes"), "ownership-notes");
        assert_eq!(slugify("  Hello, World!  "), "hello-world");
        assert_eq!(slugify("日本語"), "note");
        assert_eq!(slugify(""), "note");
        assert_eq!(Status::Scratch.to_string(), "scratch");
        assert_eq!(Status::Keep.as_str(), "keep");
    }
}
