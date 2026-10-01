use super::helpers::first;

/// Why a push did not happen, as the reader's next step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushError {
    /// The repository has no remote named `origin`.
    NoRemote,
    /// The remote has commits the branch lacks.
    Rejected,
    /// The remote could not be reached; git's words.
    Offline(String),
    /// The remote refused the login, or none was given; git's words.
    Login(String),
    /// Anything else, such as a rule on the remote; git's words.
    Git(String),
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRemote => write!(f, "This repository has no remote named origin, so there is nowhere to push"),
            Self::Rejected => write!(f, "The remote has commits this branch does not have. Pull and rebase, then push"),
            Self::Offline(words) => write!(f, "Could not reach the remote ({}). Push again when the network is back", first(words)),
            Self::Login(words) => write!(f, "The remote refused the login ({}). Sign in with gh auth login, then push again", first(words)),
            Self::Git(words) => write!(f, "The push failed: {}", first(words)),
        }
    }
}

/// Why a pull with a rebase stopped. Each leaves the branch and the files as they were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebaseError {
    /// The working tree holds edits no commit has; they are the reader's, and stay.
    OtherEdits,
    /// The remote's commits and the branch's change the same lines of these files.
    Conflict(Vec<String>),
    Offline(String),
    Git(String),
    /// The rebase stopped for `why`, and the edits set aside could not go back: they stay in `entry`.
    EditsKept { why: String, entry: String },
}

impl std::fmt::Display for RebaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OtherEdits => write!(f, "Pull and rebase needs the other edits committed or undone first; nothing changed"),
            Self::Conflict(files) => write!(f, "The remote changed the same lines in {}. Nothing changed: the rebase was undone", files.join(", ")),
            Self::Offline(words) => write!(f, "Could not reach the remote ({})", first(words)),
            Self::Git(words) => write!(f, "Pull and rebase failed: {}", first(words)),
            Self::EditsKept { why, entry } => write!(f, "{why}. Your edits stay in {entry} (\"{ENTRY_NAME}\")"),
        }
    }
}

/// The stash entry's message when the reader sets their edits aside for a rebase.
pub const ENTRY_NAME: &str = "atelier: edits set aside to pull and rebase";

/// The reader's edits after the rebase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PutBack {
    /// They are back in the working tree, and the stash entry is gone.
    Back,
    /// They clash with the new commits in `files`: the files show the clash, and the edits stay in
    /// the stash `entry` (such as `stash@{0}`).
    Kept { entry: String, files: Vec<String> },
}

impl std::fmt::Display for PutBack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Back => write!(f, "Your edits are back"),
            Self::Kept { entry, files } => write!(
                f,
                "Your edits clash with the new commits in {}. The files show the clash, and your edits stay in {entry} (\"{ENTRY_NAME}\")",
                files.join(", ")
            ),
        }
    }
}
