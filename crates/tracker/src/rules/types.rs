use crate::{PrLink, SessionLink, TaskId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// A session started from a task moves it to In Progress.
    SessionStartMovesToInProgress,
    /// The agent finishing its work moves the task to In Review.
    AgentFinishMovesToInReview,
    /// The pull request merging moves the task to Done.
    MergeMovesToDone,
    /// A reply in the session of a task in review moves it back to In Progress.
    SessionResumeMovesToInProgress,
}

impl Rule {
    pub const ALL: [Rule; 4] = [
        Self::SessionStartMovesToInProgress,
        Self::AgentFinishMovesToInReview,
        Self::MergeMovesToDone,
        Self::SessionResumeMovesToInProgress,
    ];

    /// The id kept in the settings, and written in the activity log as `rule:<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Self::SessionStartMovesToInProgress => "session-start",
            Self::AgentFinishMovesToInReview => "agent-finish",
            Self::MergeMovesToDone => "merge",
            Self::SessionResumeMovesToInProgress => "session-resume",
        }
    }

    pub fn from_id(id: &str) -> Option<Rule> {
        Self::ALL.into_iter().find(|r| r.id() == id)
    }

    /// What the rule does, for the settings.
    pub fn words(self) -> &'static str {
        match self {
            Self::SessionStartMovesToInProgress => "Starting a session on a task moves it to In Progress",
            Self::AgentFinishMovesToInReview => "The agent finishing its work moves the task to In Review",
            Self::MergeMovesToDone => "Merging the pull request moves the task to Done",
            Self::SessionResumeMovesToInProgress => "A reply in the session moves a task in review back to In Progress",
        }
    }
}

/// What happened, in the neutral words of the session and forge events.
#[derive(Clone, Debug, PartialEq)]
pub enum Signal {
    /// A session was started from this task.
    SessionStarted { task: TaskId, session: SessionLink },
    /// A session ended. `ok` is false when it failed, which moves nothing.
    SessionFinished { session_id: String, ok: bool },
    /// The reader sent a message in a session that is linked to tasks.
    SessionResumed { session_id: String },
    /// A session made a commit. It moves nothing; it is logged on the tasks of the session.
    Committed { session_id: String, sha: String, subject: String, by: String },
    /// A pull request was opened for this task, by `by`.
    PrOpened { task: TaskId, pr: PrLink, by: String },
    /// A pull request was merged, by `by`. Every task it is linked to hears of it.
    PrMerged { number: u64, by: String },
}
