use beui::merge::{Action, Choice, MergeMethod as Ui, UpdateWay};
use lathe_forge::MergeMethod;

use crate::{actions::merge_request, notes::compose, PartKind};
use lathe_forge::ForgeError;

fn choice(method: Ui) -> Choice {
    Choice { method, auto: false, delete_branch: true }
}

#[test]
fn a_squash_sends_the_commits_words_and_the_head_the_reader_saw() {
    let request = merge_request(Action::Merge(Ui::Squash), choice(Ui::Squash), "  Title  ", "Body", "abc123").unwrap();
    assert_eq!(request.method, MergeMethod::Squash);
    assert_eq!((request.title.as_deref(), request.message.as_deref()), (Some("Title"), Some("Body")));
    assert_eq!(request.expected_head.as_deref(), Some("abc123"));
    assert!(!request.when_ready && request.delete_branch);
}

#[test]
fn a_merge_commit_and_a_rebase_leave_the_words_to_the_forge() {
    for (ui, method) in [(Ui::Merge, MergeMethod::Merge), (Ui::Rebase, MergeMethod::Rebase)] {
        let request = merge_request(Action::Merge(ui), choice(ui), "Title", "Body", "abc").unwrap();
        assert_eq!((request.method, request.title, request.message), (method, None, None));
    }
    let empty = merge_request(Action::Merge(Ui::Squash), choice(Ui::Squash), "  ", "", "abc").unwrap();
    assert_eq!((empty.title, empty.message), (None, None), "empty words are the forge's");
}

#[test]
fn merge_when_ready_and_bypass_and_the_queue_are_merges_with_their_flags() {
    assert!(merge_request(Action::MergeWhenReady(Ui::Squash), choice(Ui::Squash), "", "", "a").unwrap().when_ready);
    assert!(!merge_request(Action::BypassAndMerge(Ui::Merge), choice(Ui::Merge), "", "", "a").unwrap().when_ready);
    let queued = merge_request(Action::AddToQueue, choice(Ui::Rebase), "", "", "a").unwrap();
    assert_eq!(queued.method, MergeMethod::Rebase, "the queue lands with the method chosen");
}

#[test]
fn what_the_forge_interface_cannot_do_yet_says_so() {
    for action in [Action::ReadyForReview, Action::UpdateBranch(UpdateWay::Merge), Action::CancelMergeWhenReady, Action::RemoveFromQueue, Action::DeleteBranch, Action::Revert] {
        let words = merge_request(action, choice(Ui::Merge), "", "", "a").unwrap_err();
        assert!(words.contains("not built yet"), "{action:?}: {words}");
    }
}

#[test]
fn the_line_under_the_header_tells_the_most_pressing_thing() {
    let errors = vec![(PartKind::Checks, ForgeError::Offline)];
    assert_eq!(compose(Some("Sending…"), Some("git"), Some("note"), &errors).as_deref(), Some("Sending…"));
    assert_eq!(compose(None, Some("git"), Some("note"), &errors).as_deref(), Some("Could not read the checks: The forge could not be reached"));
    assert_eq!(compose(Some(""), Some("no cache"), Some("note"), &[]).as_deref(), Some("The files cannot be shown: no cache"));
    assert_eq!(compose(None, None, Some("note"), &[]).as_deref(), Some("note"));
    assert_eq!(compose(None, None, None, &[]), None);
}
