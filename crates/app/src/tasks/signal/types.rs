/// Who made a commit or opened a pull request from the review: the person at the keys.
pub(super) const READER: &str = "you";

/// What happened in a session, as the session sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskEvent {
    /// The agent started, for a session begun from a task.
    Started,
    /// A turn ended. `ok` is false when it failed or the reader interrupted it.
    TurnEnded { ok: bool },
    /// The agent started for a session that has no task yet: it may be linked to one already (a session
    /// the app opened again after a restart).
    Adopt,
    /// The reader sent a message after the first.
    Replied,
    /// The reader made a commit from the session's review.
    Committed { sha: String, subject: String },
    /// The forge opened a pull request for the session's branch.
    PrOpened { number: u64, repo: String },
}
