use atelier_ui::inline_review::{Decision, InlineHunk, apply_to_text};

use super::{Merged, Side};

fn hunks(merged: &Merged) -> Vec<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    merged.hunks().iter().map(|h| (h.removed.clone(), h.added.clone())).collect()
}

#[test]
fn equal_texts_have_no_hunks() {
    let merged = Merged::diff("a\nb\nc\n", "a\nb\nc\n");
    assert!(merged.hunks().is_empty() && merged.is_unchanged());
    assert_eq!(merged.text(), "a\nb\nc\n");
    assert_eq!(merged.counts(), (0, 0));
}

#[test]
fn a_changed_row_is_its_old_row_then_its_new_row() {
    let merged = Merged::diff("a\nb\nc\n", "a\nB\nc\n");
    assert_eq!(merged.text(), "a\nb\nB\nc\n");
    assert_eq!(hunks(&merged), [(1..2, 2..3)]);
    assert_eq!(merged.counts(), (1, 1));
}

#[test]
fn an_insertion_has_no_removed_rows_and_a_deletion_no_added_rows() {
    let inserted = Merged::diff("a\nc\n", "a\nb\nc\n");
    assert_eq!(hunks(&inserted), [(1..1, 1..2)]);
    assert_eq!(inserted.text(), "a\nb\nc\n");
    let deleted = Merged::diff("a\nb\nc\n", "a\nc\n");
    assert_eq!(hunks(&deleted), [(1..2, 2..2)]);
    assert_eq!(deleted.text(), "a\nb\nc\n");
}

#[test]
fn several_hunks_sit_in_row_order_with_the_context_between() {
    let merged = Merged::diff("1\n2\n3\n4\n5\n6\n7\n", "1\nX\n3\n4\n5\nY\nZ\n7\n");
    assert_eq!(merged.text(), "1\n2\nX\n3\n4\n5\n6\nY\nZ\n7\n");
    assert_eq!(hunks(&merged), [(1..2, 2..3), (6..7, 7..9)]);
    assert_eq!(merged.counts(), (3, 2));
}

#[test]
fn a_new_file_is_all_added_and_a_deleted_file_all_removed() {
    let new = Merged::diff("", "a\nb\n");
    assert_eq!(hunks(&new), [(0..0, 0..2)]);
    assert_eq!((new.baseline(), new.current()), (String::new(), "a\nb\n".to_string()));
    let gone = Merged::diff("a\nb\n", "");
    assert_eq!(hunks(&gone), [(0..2, 2..2)]);
    assert_eq!((gone.baseline(), gone.current()), ("a\nb\n".to_string(), String::new()));
    let none = Merged::diff("", "");
    assert!(none.is_unchanged() && none.text().is_empty());
}

#[test]
fn a_file_that_lost_its_final_line_end_differs_only_in_the_flags() {
    let merged = Merged::diff("a\nb\n", "a\nb");
    assert!(merged.hunks().is_empty());
    assert!(!merged.is_unchanged(), "the ending differs");
    assert_eq!((merged.baseline(), merged.current()), ("a\nb\n".to_string(), "a\nb".to_string()));
}

#[test]
fn a_hunk_at_the_end_of_a_file_with_no_line_end_does_not_run_into_the_next_row() {
    let merged = Merged::diff("a\nb", "a\nb\nc\n");
    assert_eq!((merged.baseline(), merged.current()), ("a\nb".to_string(), "a\nb\nc\n".to_string()));
    assert!(merged.text().lines().all(|row| !row.contains("bc")));
    let replaced = Merged::diff("a\nb", "a\nB");
    assert_eq!(replaced.text(), "a\nb\nB\n");
    assert_eq!((replaced.baseline(), replaced.current()), ("a\nb".to_string(), "a\nB".to_string()));
}

/// A small deterministic generator, so a failure repeats.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn random_text(rng: &mut Rng) -> String {
    let vocabulary = ["", "a", "b", "c", "fn main() {", "}", "  x", "é", "a b", "\tq", "same"];
    let rows = rng.next() % 9;
    let mut text = (0..rows).map(|_| vocabulary[(rng.next() % vocabulary.len() as u64) as usize]).collect::<Vec<_>>().join("\n");
    if rows > 0 && !rng.next().is_multiple_of(4) {
        text.push('\n');
    }
    text
}

#[test]
fn both_versions_come_back_exactly_from_any_pair_of_texts() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..2000 {
        let (before, after) = (random_text(&mut rng), random_text(&mut rng));
        let merged = Merged::diff(&before, &after);
        assert_eq!(merged.baseline(), before, "baseline of {before:?} -> {after:?}");
        assert_eq!(merged.current(), after, "current of {before:?} -> {after:?}");
    }
}

#[test]
fn hunks_never_touch_and_never_overlap() {
    let mut rng = Rng(42);
    for _ in 0..1000 {
        let merged = Merged::diff(&random_text(&mut rng), &random_text(&mut rng));
        for pair in merged.hunks().windows(2) {
            assert!(pair[0].added.end <= pair[1].removed.start, "{:?}", merged.hunks());
        }
        for hunk in merged.hunks() {
            assert_eq!(hunk.removed.end, hunk.added.start);
            assert!(!hunk.rows().is_empty());
        }
    }
}

#[test]
fn deciding_a_hunk_is_what_the_inline_review_does_to_the_text() {
    let mut rng = Rng(7);
    for _ in 0..500 {
        let merged = Merged::diff(&random_text(&mut rng), &random_text(&mut rng));
        for hunk in merged.hunks() {
            for decision in [Decision::Accept, Decision::Reject] {
                let by_beui = apply_to_text(merged.text(), &[(hunk.clone(), decision)]);
                let ours = merged.decide(&hunk.id, decision).unwrap();
                assert_eq!(ours.text(), by_beui, "{decision:?} of {hunk:?} in {:?}", merged.text());
            }
        }
    }
}

#[test]
fn after_a_decision_the_other_hunks_move_and_the_versions_follow() {
    let merged = Merged::diff("1\n2\n3\n4\n5\n", "1\nX\n3\n4\nY\nZ\n");
    let first = merged.hunks()[0].id.clone();
    let accepted = merged.decide(&first, Decision::Accept).unwrap();
    assert_eq!(accepted.text(), "1\nX\n3\n4\n5\nY\nZ\n");
    assert_eq!(hunks(&accepted), [(4..5, 5..7)]);
    assert_eq!(accepted.baseline(), "1\nX\n3\n4\n5\n", "what the user accepted is part of the baseline now");
    assert_eq!(accepted.current(), "1\nX\n3\n4\nY\nZ\n");

    let rejected = merged.decide(&first, Decision::Reject).unwrap();
    assert_eq!(rejected.text(), "1\n2\n3\n4\n5\nY\nZ\n");
    assert_eq!(rejected.baseline(), "1\n2\n3\n4\n5\n");
    assert_eq!(rejected.current(), "1\n2\n3\n4\nY\nZ\n", "what the user rejected is gone from the file");
}

#[test]
fn deciding_every_hunk_leaves_a_text_with_no_hunks() {
    let mut merged = Merged::diff("a\nb\nc\n", "a\nB\nc\nd\n");
    while let Some(id) = merged.hunks().first().map(|h| h.id.clone()) {
        merged = merged.decide(&id, Decision::Accept).unwrap();
    }
    assert_eq!(merged.text(), "a\nB\nc\nd\n");
    assert!(merged.is_unchanged());
}

#[test]
fn deciding_the_last_hunk_of_a_file_takes_the_ending_of_the_side_that_stays() {
    let merged = Merged::diff("a\nb", "a\nB\n");
    let id = merged.hunks()[0].id.clone();
    let accepted = merged.decide(&id, Decision::Accept).unwrap();
    assert_eq!((accepted.baseline(), accepted.current()), ("a\nB\n".to_string(), "a\nB\n".to_string()));
    let rejected = merged.decide(&id, Decision::Reject).unwrap();
    assert_eq!((rejected.baseline(), rejected.current()), ("a\nb".to_string(), "a\nb".to_string()));
}

#[test]
fn a_hunk_that_is_not_there_decides_nothing() {
    assert!(Merged::diff("a\n", "b\n").decide("nope", Decision::Accept).is_none());
}

#[test]
fn the_users_typing_moves_the_hunks_and_keeps_their_ids() {
    let merged = Merged::diff("a\nb\nc\n", "a\nB\nc\n");
    let typed = merged.edited("new first row\na\nb\nB\nc\n");
    assert_eq!(hunks(&typed), [(2..3, 3..4)]);
    assert_eq!(typed.current(), "new first row\na\nB\nc\n");
    assert_eq!(typed.baseline(), "new first row\na\nb\nc\n");
    assert_eq!(typed.hunks()[0].id, merged.hunks()[0].id, "the hunk is the same hunk");
}

#[test]
fn an_agent_edit_adds_its_hunks_and_keeps_what_the_user_decided() {
    // The agent changed rows 2 and 5; the user accepted the first and rejected the second.
    let merged = Merged::diff("1\n2\n3\n4\n5\n6\n", "1\nX\n3\n4\nY\n6\n");
    let (first, second) = (merged.hunks()[0].id.clone(), merged.hunks()[1].id.clone());
    let decided = merged.decide(&first, Decision::Accept).unwrap().decide(&second, Decision::Reject).unwrap();
    assert!(decided.hunks().is_empty());
    assert_eq!(decided.current(), "1\nX\n3\n4\n5\n6\n");

    // The agent now edits row 4 of the file as it stands.
    let again = decided.rebased_on("1\nX\n3\nFOUR\n5\n6\n");
    assert_eq!(hunks(&again), [(3..4, 4..5)], "only the new edit is open");
    assert_eq!(again.baseline(), "1\nX\n3\n4\n5\n6\n", "the accepted row is in the baseline, the rejected row is back");
    assert_eq!(again.current(), "1\nX\n3\nFOUR\n5\n6\n");
}

#[test]
fn a_rebase_keeps_the_ids_of_hunks_that_did_not_change() {
    let merged = Merged::diff("1\n2\n3\n4\n5\n6\n", "1\nX\n3\n4\nY\n6\n");
    let rebased = merged.rebased_on("1\nX\n3\n4\nY\n6\n7\n");
    let ids = |m: &Merged| m.hunks().iter().map(|h| h.id.clone()).collect::<Vec<_>>();
    assert_eq!(&ids(&rebased)[..2], &ids(&merged)[..]);
    assert_eq!(rebased.hunks().len(), 3);
}

#[test]
fn hunks_that_say_the_same_thing_get_different_ids() {
    let merged = Merged::diff("a\nx\nb\nx\nc\n", "a\ny\nb\ny\nc\n");
    let ids: Vec<_> = merged.hunks().iter().map(|h| h.id.clone()).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(ids[0].split('.').next(), ids[1].split('.').next(), "the same words, told apart by a number");
}

#[test]
fn a_comment_on_rows_names_the_lines_of_the_version_they_belong_to() {
    // merged rows: 0 "1", 1 "2" (removed), 2 "X" (added), 3 "3", 4 "4"
    let merged = Merged::diff("1\n2\n3\n4\n", "1\nX\n3\n4\n");
    let on_new = merged.anchor(2..4).unwrap();
    assert_eq!((on_new.side, on_new.first_line, on_new.last_line, on_new.quote.as_str()), (Side::Current, 2, 3, "X\n3"));
    let on_old = merged.anchor(1..2).unwrap();
    assert_eq!((on_old.side, on_old.first_line, on_old.last_line, on_old.quote.as_str()), (Side::Removed, 2, 2, "2"));
    let plain = merged.anchor(4..5).unwrap();
    assert_eq!((plain.side, plain.first_line), (Side::Current, 4));
}

#[test]
fn a_range_stops_where_its_side_stops() {
    let merged = Merged::diff("1\n2\n3\n", "1\nX\n3\n");
    let anchor = merged.anchor(0..4).unwrap();
    assert_eq!((anchor.side, anchor.first_line, anchor.last_line, anchor.quote.as_str()), (Side::Current, 1, 1, "1"));
    let removed = merged.anchor(1..4).unwrap();
    assert_eq!((removed.side, removed.quote.as_str()), (Side::Removed, "2"));
}

#[test]
fn a_range_that_is_empty_or_past_the_text_has_no_anchor() {
    let merged = Merged::diff("a\n", "b\n");
    assert!(merged.anchor(1..1).is_none());
    assert!(merged.anchor(9..12).is_none());
}

#[test]
fn a_large_file_is_diffed_row_by_row_without_losing_a_row() {
    let before: String = (0..20_000).map(|i| format!("line {i}\n")).collect();
    let after: String = (0..20_000).map(|i| if i % 500 == 7 { format!("changed {i}\n") } else { format!("line {i}\n") }).collect();
    let merged = Merged::diff(&before, &after);
    assert_eq!(merged.hunks().len(), 40);
    assert_eq!((merged.baseline(), merged.current()), (before, after));
    let InlineHunk { removed, added, .. } = &merged.hunks()[0];
    assert_eq!((removed.len(), added.len()), (1, 1));
}
