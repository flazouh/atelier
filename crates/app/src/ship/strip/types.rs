use gpui_kit::SharedString;
use atelier_forge::PullRef;

/// The context the strip's fields sit in, for ⌘↵.
pub const CONTEXT: &str = "ShipComposer";

pub enum StripEvent {
    /// The reader asked to commit: the owner opens the strip with what the review kept.
    WantsOpen,
    /// The reader asked to accept every file and commit it.
    AcceptAllAndCommit,
    /// A commit of these paths, with its full id.
    Committed { sha: String, paths: Vec<String>, subject: String },
    /// The branch went to origin.
    Pushed,
    /// A rebase gave the branch's own commits new ids: each old id with its new one.
    Rewrote(Vec<(String, String)>),
    /// The forge opened this pull request for the branch, or the branch had it already.
    PullOpened(PullRef),
    /// The reader asked to see this pull request.
    ShowPull(PullRef),
}

/// Why a pull and rebase stopped, for the strip.
pub(super) enum Stop {
    OtherEdits,
    Words(String),
}

/// Where the strip is.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    Closed,
    /// Commit was asked with nothing accepted: Accept all and commit is offered.
    NothingKept,
    /// Reading HEAD and the branch.
    Reading,
    Open,
    Committing,
    /// The last commit: its words for the strip.
    Committed(SharedString),
    Failed(SharedString),
    /// A push, or a pull and a rebase, running: its words.
    Pushing(SharedString),
    /// The branch is on origin: the words that say so.
    Pushed(SharedString),
    /// The remote has commits the branch lacks; Pull and rebase is offered.
    Rejected,
    /// Pull and rebase stopped before it started, for the reader's other edits; setting them aside is
    /// offered.
    EditsInTheWay,
    /// Pushed, but the edits set aside clash with the new commits: the words say where they are.
    Clashed(SharedString),
    /// The pull request form shows.
    Pull,
    /// The pull request is open: the words that say so.
    PullOpened(SharedString),
}
