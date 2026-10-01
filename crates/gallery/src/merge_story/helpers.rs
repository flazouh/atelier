use atelier_ui::{
    merge::{MergeFacts, MergeMethod, PullState, Queue, ReviewNeed, Rights, UpdateWay},
    };

/// Each state the button shows, with its name in the story.
pub fn states() -> Vec<(&'static str, MergeFacts)> {
    let open = MergeFacts { default_method: MergeMethod::Squash, delete_branch: true, auto_merge: Some(false), ..MergeFacts::default() };
    vec![
        ("Ready", open.clone()),
        ("Draft", MergeFacts { draft: true, ..open.clone() }),
        ("Conflicts", MergeFacts { conflicts: vec!["src/request.rs".into(), "src/relay.rs".into()], ..open.clone() }),
        ("Behind its base", MergeFacts { behind: Some(vec![UpdateWay::Merge, UpdateWay::Rebase]), ..open.clone() }),
        ("Checks failing", MergeFacts { checks_failing: 1, ..open.clone() }),
        ("Checks running", MergeFacts { checks_running: 2, ..open.clone() }),
        ("Checks running, auto chosen", MergeFacts { checks_running: 2, ..open.clone() }),
        ("Review missing", MergeFacts { review: ReviewNeed::Missing, ..open.clone() }),
        ("Changes asked", MergeFacts { review: ReviewNeed::ChangesAsked(vec!["Ada".into()]), ..open.clone() }),
        ("Merge when ready on", MergeFacts { checks_running: 2, auto_merge: Some(true), ..open.clone() }),
        ("Merge queue", MergeFacts { queue: Some(Queue { queued: false, position: None }), ..open.clone() }),
        ("In the queue", MergeFacts { queue: Some(Queue { queued: true, position: Some(3) }), ..open.clone() }),
        ("Admin, checks failing", MergeFacts { checks_failing: 1, rights: Rights::Bypass, ..open.clone() }),
        ("Cannot merge", MergeFacts { rights: Rights::Cannot, ..open.clone() }),
        // Last, as the PR card story reads it.
        ("Merged", MergeFacts { state: PullState::Merged, ..open }),
    ]
}
