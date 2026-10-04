//! The forge's data as the models atelier-ui draws, checked through atelier-ui's own words: the merge model's
//! blockers and standing line, the chip, the checks summary.
mod support;

use atelier_ui::{
    merge::{self, Blocker, MergeFacts, ReviewNeed, Rights as UiRights},
    pr::{ChecksSummary, PrState, ReviewState},
};
use atelier_forge::{Check, CheckStatus, Conclusion, Forge, present};
use serde_json::{Value, json};
use support::{github, patched_pull, pull_ref, recorded};

fn facts(change: impl FnOnce(&mut Value)) -> MergeFacts {
    let pull = github(&patched_pull(change)).pull(&pull_ref()).unwrap();
    present::merge_facts(&pull, None, &[])
}

/// An open pull request that nothing holds up.
fn open(r: &mut Value) {
    r["pullRequest"]["merged"] = json!(false);
    r["pullRequest"]["state"] = json!("OPEN");
    r["pullRequest"]["reviewDecision"] = json!("APPROVED");
    r["pullRequest"]["mergeStateStatus"] = json!("CLEAN");
    r["pullRequest"]["mergeable"] = json!("MERGEABLE");
    r["viewerPermission"] = json!("WRITE");
}

#[test]
fn a_clean_approved_pull_is_ready_to_merge_in_beuis_words() {
    let facts = facts(open);
    assert_eq!(merge::standing(&facts), "Ready to merge");
    assert!(merge::blockers(&facts).is_empty());
    assert_eq!(facts.methods, [merge::MergeMethod::Squash]);
    assert_eq!(facts.default_method, merge::MergeMethod::Squash);
    assert_eq!((facts.auto_merge, facts.delete_branch, facts.rights), (Some(false), true, UiRights::Merge));
}

#[test]
fn a_recorded_merged_pull_is_merged() {
    let pull = github(&recorded()).pull(&pull_ref()).unwrap();
    let facts = present::merge_facts(&pull, None, &[]);
    assert_eq!(merge::standing(&facts), "Merged");
}

#[test]
fn a_draft_a_conflict_and_a_branch_behind_each_block_and_say_so() {
    let draft = facts(|r| {
        open(r);
        r["pullRequest"]["isDraft"] = json!(true);
    });
    assert!(draft.draft && merge::blockers(&draft).contains(&Blocker::Draft));

    let conflicting = facts(|r| {
        open(r);
        r["pullRequest"]["mergeable"] = json!("CONFLICTING");
    });
    assert!(matches!(merge::blockers(&conflicting).first(), Some(Blocker::Conflicts(files)) if files.len() == 1 && files[0].as_ref() == "main"));

    let behind = facts(|r| {
        open(r);
        r["pullRequest"]["mergeStateStatus"] = json!("BEHIND");
    });
    assert!(matches!(merge::blockers(&behind).first(), Some(Blocker::Behind(ways)) if ways.len() == 2));
}

#[test]
fn conflicting_files_the_reader_found_replace_the_base_branch_stand_in() {
    let pull = github(&patched_pull(|r| {
        open(r);
        r["pullRequest"]["mergeable"] = json!("CONFLICTING");
    }))
    .pull(&pull_ref())
    .unwrap();
    let files = vec!["a.rs".to_string(), "b.rs".to_string()];
    let facts = present::merge_facts(&pull, None, &files);
    assert_eq!(facts.conflicts.iter().map(|f| f.to_string()).collect::<Vec<_>>(), files);
}

#[test]
fn the_review_that_is_missing_or_that_asked_for_changes_blocks() {
    let missing = facts(|r| {
        open(r);
        r["pullRequest"]["reviewDecision"] = json!("REVIEW_REQUIRED");
    });
    assert_eq!(missing.review, ReviewNeed::Missing);
    assert!(merge::blockers(&missing).contains(&Blocker::ReviewMissing));

    let changes = facts(|r| {
        open(r);
        r["pullRequest"]["reviewDecision"] = json!("CHANGES_REQUESTED");
        r["pullRequest"]["latestReviews"] = json!({"nodes": [{"author": {"login": "grace"}, "state": "CHANGES_REQUESTED"}, {"author": {"login": "ada"}, "state": "APPROVED"}]});
    });
    assert!(matches!(&changes.review, ReviewNeed::ChangesAsked(who) if who.len() == 1 && who[0].as_ref() == "grace"));
    assert_eq!(merge::standing(&changes), "Blocked: changes asked by grace");
}

#[test]
fn without_the_check_list_a_rule_that_blocks_takes_its_counts_from_the_totals() {
    let blocked = facts(|r| {
        open(r);
        r["pullRequest"]["mergeStateStatus"] = json!("BLOCKED");
        r["pullRequest"]["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["checkRunCountsByState"] = json!([{"state": "FAILURE", "count": 2}]);
    });
    assert_eq!(blocked.checks_failing, 2);
    let unstable = facts(|r| {
        open(r);
        r["pullRequest"]["mergeStateStatus"] = json!("UNSTABLE");
        r["pullRequest"]["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["checkRunCountsByState"] = json!([{"state": "FAILURE", "count": 2}]);
    });
    assert_eq!(unstable.checks_failing, 0, "a check that is not required does not hold the merge up");
    assert!(merge::blockers(&unstable).is_empty());
}

fn check(name: &str, required: bool, status: CheckStatus, conclusion: Option<Conclusion>) -> Check {
    Check { name: name.into(), status, conclusion, url: None, required, started_at: None, completed_at: None, job: None, run: None }
}

#[test]
fn with_the_check_list_only_required_checks_count_and_exactly() {
    let pull = github(&patched_pull(open)).pull(&pull_ref()).unwrap();
    let list = [
        check("build", true, CheckStatus::Done, Some(Conclusion::Failure)),
        check("test", true, CheckStatus::Done, Some(Conclusion::TimedOut)),
        check("deploy", true, CheckStatus::Running, None),
        check("lint", false, CheckStatus::Done, Some(Conclusion::Failure)),
        check("docs", true, CheckStatus::Done, Some(Conclusion::Success)),
    ];
    let facts = present::merge_facts(&pull, Some(&list), &[]);
    assert_eq!((facts.checks_failing, facts.checks_running), (2, 1));
    assert_eq!(merge::standing(&facts), "Blocked: 2 checks failing");
}

#[test]
fn a_queue_and_auto_merge_and_rights_reach_the_model() {
    let queued = facts(|r| {
        open(r);
        r["mergeQueue"] = json!({"id": "MQ"});
        r["pullRequest"]["isInMergeQueue"] = json!(true);
        r["pullRequest"]["mergeQueueEntry"] = json!({"position": 3});
    });
    assert_eq!(merge::standing(&queued), "In the merge queue, number 3");
    let waiting = facts(|r| {
        open(r);
        r["pullRequest"]["autoMergeRequest"] = json!({"enabledAt": "2026-09-29T15:14:16Z"});
    });
    assert_eq!(merge::standing(&waiting), "Merges when ready");
    let reader = facts(|r| {
        open(r);
        r["viewerPermission"] = json!("READ");
    });
    assert_eq!(merge::standing(&reader), "Ready to merge, by someone with write access");
    let admin = facts(|r| {
        open(r);
        r["pullRequest"]["viewerCanMergeAsAdmin"] = json!(true);
    });
    assert_eq!(admin.rights, UiRights::Bypass);
}

#[test]
fn a_brief_becomes_a_chip_and_a_state_keeps_its_meaning() {
    let briefs = github(&recorded()).briefs(&support::bun(), &[44169]).unwrap();
    let chip = present::chip(&briefs[0].as_ref().unwrap().brief);
    assert_eq!((chip.number, chip.state), (44169, PrState::Merged));
    assert_eq!(chip.repo.as_ref(), "oven-sh/bun");
    assert_eq!(chip.label().as_ref(), "#44169");
    assert_eq!(chip.url.as_ref(), "https://github.com/oven-sh/bun/pull/44169");
}

#[test]
fn checks_and_review_states_map_to_beuis() {
    let pull = github(&recorded()).pull(&pull_ref()).unwrap();
    assert_eq!(present::checks(pull.checks).summary(), ChecksSummary::Passed(13));
    assert_eq!(present::review_state(pull.review), ReviewState::Requested);
}
