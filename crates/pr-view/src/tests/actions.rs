use atelier_ui::merge::{Action, Choice, MergeMethod as Ui, UpdateWay};
use atelier_forge::{MergeMethod, MergeRequest, UpdateMethod};

use crate::{actions::{Ask, ask}, notes::compose, PartKind};
use atelier_forge::ForgeError;

fn choice(method: Ui) -> Choice {
    Choice { method, auto: false, delete_branch: true }
}

#[test]
fn a_squash_sends_the_commits_words_and_the_head_the_reader_saw() {
    let request = ask(Action::Merge(Ui::Squash), choice(Ui::Squash), "  Title  ", "Body", "abc123");
    let Ask::Merge(request) = request else { panic!("a merge") };
    assert_eq!(request.method, MergeMethod::Squash);
    assert_eq!((request.title.as_deref(), request.message.as_deref()), (Some("Title"), Some("Body")));
    assert_eq!(request.expected_head.as_deref(), Some("abc123"));
    assert!(!request.when_ready && request.delete_branch);
}

#[test]
fn a_merge_commit_and_a_rebase_leave_the_words_to_the_forge() {
    for (ui, method) in [(Ui::Merge, MergeMethod::Merge), (Ui::Rebase, MergeMethod::Rebase)] {
        let Ask::Merge(request) = ask(Action::Merge(ui), choice(ui), "Title", "Body", "abc") else { panic!("a merge") };
        assert_eq!((request.method, request.title, request.message), (method, None, None));
    }
    let Ask::Merge(empty) = ask(Action::Merge(Ui::Squash), choice(Ui::Squash), "  ", "", "abc") else { panic!("a merge") };
    assert_eq!((empty.title, empty.message), (None, None), "empty words are the forge's");
}

#[test]
fn merge_when_ready_and_bypass_and_the_queue_are_merges_with_their_flags() {
    let merge = |action, c| match ask(action, c, "", "", "a") {
        Ask::Merge(request) => request,
        other => panic!("a merge, not {other:?}"),
    };
    assert!(merge(Action::MergeWhenReady(Ui::Squash), choice(Ui::Squash)).when_ready);
    assert!(!merge(Action::BypassAndMerge(Ui::Merge), choice(Ui::Merge)).when_ready);
    let queued: MergeRequest = merge(Action::AddToQueue, choice(Ui::Rebase));
    assert_eq!(queued.method, MergeMethod::Rebase, "the queue lands with the method chosen");
}

#[test]
fn the_other_presses_ask_for_their_own_call() {
    let c = choice(Ui::Merge);
    assert_eq!(ask(Action::ReadyForReview, c, "", "", "h"), Ask::Ready);
    assert_eq!(ask(Action::UpdateBranch(UpdateWay::Merge), c, "", "", "h1"), Ask::UpdateBranch { method: UpdateMethod::Merge, expected_head: "h1".into() });
    assert_eq!(ask(Action::UpdateBranch(UpdateWay::Rebase), c, "", "", "h2"), Ask::UpdateBranch { method: UpdateMethod::Rebase, expected_head: "h2".into() });
    assert_eq!(ask(Action::CancelMergeWhenReady, c, "", "", "h"), Ask::CancelAutoMerge);
    assert_eq!(ask(Action::RemoveFromQueue, c, "", "", "h"), Ask::Dequeue);
    assert_eq!(ask(Action::DeleteBranch, c, "", "", "h"), Ask::DeleteBranch);
    assert_eq!(ask(Action::Revert, c, "", "", "h"), Ask::Revert);
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
