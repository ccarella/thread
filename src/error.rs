//! Shared error type for the TUI and the note store.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("missing yaml front matter")]
    MissingFrontMatter,

    #[error("invalid datetime: {0}")]
    InvalidDatetime(String),

    #[error("invalid topic name: {0}")]
    InvalidTopic(String),

    #[error("note not found: {0}")]
    NotFound(PathBuf),
}
