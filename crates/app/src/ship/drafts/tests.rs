use super::*;
use crate::ship::kept::Kept;

#[test]
fn a_drafted_branch_name_is_made_safe() {
    assert_eq!(branch_name("fix/release-lease-on-exit").as_deref(), Some("fix/release-lease-on-exit"));
    assert_eq!(branch_name("`Fix: Release the Lease!`\nmore").as_deref(), Some("fix-release-the-lease"));
    assert_eq!(branch_name("  ").as_deref(), None);
    assert!(branch_name(&"x".repeat(200)).is_some_and(|n| n.len() <= 60));
}

#[test]
fn a_drafted_message_loses_its_fences_and_quotes() {
    assert_eq!(message("```\nFix the lease\n\nIt was never released.\n```"), "Fix the lease\n\nIt was never released.");
    assert_eq!(message("\"Fix it\""), "Fix it");
    assert_eq!(message("  Keep TWO  "), "Keep TWO");
}

/// The prompt shows the kept change as a diff against HEAD, file by file.
#[test]
fn the_prompt_holds_the_kept_diff() {
    let kept = Kept { path: "a.txt".into(), text: Some("1\nTWO\n".into()), before: None };
    let diff = kept_diff(&[(Some("1\n2\n".to_string()), kept)]);
    assert!(diff.contains("--- a.txt") && diff.contains("-2") && diff.contains("+TWO"), "{diff}");
    assert!(commit_prompt(&diff).contains(&diff));
    assert!(branch_prompt(&diff).contains("branch"));
}
/// A drafted message loses the trailers the agent adds on its own habit, such as Co-Authored-By;
/// the reader adds their own.
#[test]
fn a_drafted_message_loses_its_trailers() {
    let draft = "Add a note\n\nOne line in NOTES.md.\n\nCo-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>\nSigned-off-by: A <a@a>";
    assert_eq!(message(draft), "Add a note\n\nOne line in NOTES.md.");
    assert_eq!(message("Add a note\n\nco-authored-by: x <x@x>"), "Add a note");
}

#[test]
fn the_refs_line_names_the_task_once_and_the_cleaner_keeps_it() {
    assert_eq!(with_refs("Fix the scroll\n\nIt jumped.", "LAT-42"), "Fix the scroll\n\nIt jumped.\n\nRefs LAT-42");
    assert_eq!(with_refs("Fix the scroll", "LAT-42"), "Fix the scroll\n\nRefs LAT-42");
    let already = "Fix the scroll\n\nRefs LAT-42";
    assert_eq!(with_refs(already, "LAT-42"), already, "not added twice");
    assert_eq!(with_refs(&message("Fix it\n\nCo-Authored-By: A <a@a>"), "LAT-42"), "Fix it\n\nRefs LAT-42", "the agent trailer goes, Refs is added");
    assert_eq!(message("Fix it\n\nRefs LAT-42"), "Fix it\n\nRefs LAT-42", "the cleaner keeps a Refs line");
}
