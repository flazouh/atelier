//! What a forge tells atelier, in GitQuiet's words. Plain data: no forge, no UI, no clock.
mod checks;
mod conversation;
mod merge;
mod pull;
mod repository;

pub use checks::{Check, CheckStatus, Conclusion, Job, JobRef, RunInfo, Step};
pub use conversation::{Author, Comment, HeldComment, NewLine, Remark, Side, Thread, ThreadId, Verdict};
pub use merge::{MergeMethod, MergeOutcome, MergeRequest, MergeSettings, QueuePlace, Rights, UpdateMethod};
pub use pull::{
    Change, ChangedFile, CheckCounts, CheckState, Involved, MergeState, NewPull, Opinion, Pull, PullBrief, PullId, PullRef,
    PullState, PullSummary, PullUpdate, ReviewDecision, Reviewer, Shelf, Standing,
};
pub use repository::{RepoRef, Repository};
