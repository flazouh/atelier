use crate::Shelf;

/// The GitHub searches behind each shelf. `None` is a pull request the reader is only assigned to or
/// mentioned in: it belongs to no shelf.
pub(super) const SEARCHES: &[(Option<Shelf>, &str)] = &[
    (Some(Shelf::NeedsAction), "is:pr is:open archived:false user-review-requested:@me"),
    (Some(Shelf::NeedsAction), "is:pr is:open archived:false author:@me review:changes_requested"),
    (Some(Shelf::NeedsAction), "is:pr is:open archived:false author:@me draft:false status:failure"),
    (Some(Shelf::TeamReviewRequested), "is:pr is:open archived:false team-review-requested:@me"),
    (Some(Shelf::WaitingForReview), "is:pr is:open archived:false author:@me draft:false -review:approved"),
    (Some(Shelf::ReadyToMerge), "is:pr is:open archived:false author:@me draft:false review:approved"),
    (Some(Shelf::YourDrafts), "is:pr is:open archived:false author:@me draft:true"),
    (Some(Shelf::MergeQueue), "is:pr is:open archived:false author:@me is:queued"),
    (None, "is:pr is:open archived:false assignee:@me"),
    (None, "is:pr is:open archived:false mentions:@me"),
];
