use std::collections::HashMap;

/// One entry of `git status --porcelain=v2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub untracked: bool,
    /// The file is gone from the working tree.
    pub deleted: bool,
    /// The path a rename came from.
    pub renamed_from: Option<String>,
}

/// The working tree as git sees it: which paths differ from the last commit, and what the files that
/// differ hold, as blob ids. Paths are relative to the project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    pub entries: HashMap<String, Entry>,
    pub blobs: HashMap<String, String>,
}
