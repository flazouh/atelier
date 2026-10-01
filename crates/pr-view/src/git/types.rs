use std::{
    time::{Duration},
};

use super::helpers::short;

/// How long a fetch may take before it is given up.
pub(super) const FETCH_TIMEOUT: Duration = Duration::from_secs(900);

pub(super) const QUICK: Duration = Duration::from_secs(60);

/// A file bigger than this is listed but not diffed.
pub const MAX_TEXT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitError {
    /// git ran and said no.
    Failed { what: &'static str, stderr: String },
    /// git could not be started, or was cut off.
    Spawn(String),
    /// The head is not on the forge any more: the branch was deleted or rewritten before it was read.
    HeadGone(String),
    /// A value that is not a commit id or a name git takes.
    Invalid(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed { what, stderr } => write!(f, "git could not {what}: {}", stderr.trim()),
            Self::Spawn(why) => write!(f, "git did not run: {why}"),
            Self::HeadGone(sha) => write!(f, "the pull request's head {} is not on the forge any more", short(sha)),
            Self::Invalid(what) => write!(f, "{what} is not something git can be asked about"),
        }
    }
}

impl std::error::Error for GitError {}

pub type GitResult<T> = Result<T, GitError>;

/// A file's text on one side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blob {
    Text(String),
    Binary,
    /// The side has no such file: a new file's old side.
    Missing,
    TooLarge(u64),
}

impl Blob {
    /// The text, or `""` for a side that has none.
    pub fn text_or_empty(&self) -> &str {
        match self {
            Self::Text(text) => text,
            _ => "",
        }
    }
}

/// git for one project.
/// Where an older atelier kept the cache and the checkouts.
pub const LEGACY_DATA: &str = "~/.local/share/atelier/pr";
