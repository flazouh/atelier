use atelier_forge::{ChangedFile, Check, ForgeError, HeldComment, Pull, Remark, Thread};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartKind {
    Pull,
    Files,
    Threads,
    Remarks,
    Checks,
    ReviewPoint,
    Held,
}

impl PartKind {
    pub const ALL: [PartKind; 7] =
        [Self::Pull, Self::Files, Self::Threads, Self::Remarks, Self::Checks, Self::ReviewPoint, Self::Held];

    /// The part in the reader's words, for "could not read the checks".
    pub fn words(self) -> &'static str {
        match self {
            Self::Pull => "the pull request",
            Self::Files => "the changed files",
            Self::Threads => "the review threads",
            Self::Remarks => "the remarks",
            Self::Checks => "the checks",
            Self::ReviewPoint => "your last review",
            Self::Held => "your unsent comments",
        }
    }
}

/// One answer, or one failure, as it arrives.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Pull(Box<Pull>),
    Files(Vec<ChangedFile>),
    Threads(Vec<Thread>),
    Remarks(Vec<Remark>),
    Checks(Vec<Check>),
    ReviewPoint(Option<String>),
    Held(Vec<HeldComment>),
    Failed { part: PartKind, error: ForgeError },
}
