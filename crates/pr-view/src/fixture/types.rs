use atelier_forge::{
    MergeRequest, NewLine, NewPull, PullRef, PullUpdate, Reviewer, ThreadId, UpdateMethod,
    Verdict,
};

/// A change the reader made, as the forge got it.
#[derive(Clone, Debug, PartialEq)]
pub enum Write {
    Comment { reference: PullRef, body: String },
    Reply { thread: ThreadId, body: String },
    Resolve { thread: ThreadId, resolved: bool },
    Hold(NewLine),
    Submit { reference: PullRef, verdict: Verdict, body: String },
    Merge { reference: PullRef, request: MergeRequest },
    RequestReview { reference: PullRef, reviewers: Vec<Reviewer> },
    Update { reference: PullRef, update: PullUpdate },
    UpdateBranch { reference: PullRef, method: UpdateMethod, expected_head: String },
    CancelAutoMerge(PullRef),
    Dequeue(PullRef),
    DeleteBranch(PullRef),
    Revert(PullRef),
    Create(NewPull),
}
