use super::{pair_rows, word_changes};

fn changed<'a>(text: &'a str, ranges: &[std::ops::Range<usize>]) -> Vec<&'a str> {
    ranges.iter().map(|r| &text[r.clone()]).collect()
}

#[test]
fn a_changed_word_is_the_only_change() {
    let (old, new) = ("let count = 1;", "let total = 1;");
    let (r, a) = word_changes(old, new);
    assert_eq!((changed(old, &r), changed(new, &a)), (vec!["count"], vec!["total"]));
}

#[test]
fn an_added_argument_is_added_and_nothing_is_removed() {
    let (old, new) = ("call(a)", "call(a, b)");
    let (r, a) = word_changes(old, new);
    assert!(r.is_empty());
    assert_eq!(changed(new, &a), [", b"]);
}

#[test]
fn identical_rows_change_nothing() {
    assert_eq!(word_changes("same", "same"), (vec![], vec![]));
    assert_eq!(word_changes("", ""), (vec![], vec![]));
}

#[test]
fn a_row_against_an_empty_row_is_changed_whole() {
    let (r, a) = word_changes("abc def", "");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0], 0..7);
    assert!(a.is_empty());
}

#[test]
fn punctuation_stands_alone_and_words_keep_their_underscores() {
    let (old, new) = ("foo_bar.baz()", "foo_bar.qux()");
    let (r, a) = word_changes(old, new);
    assert_eq!((changed(old, &r), changed(new, &a)), (vec!["baz"], vec!["qux"]));
}

#[test]
fn ranges_are_on_character_boundaries_in_text_with_accents_and_emoji() {
    let (old, new) = ("café ☕ ok", "café 🍵 ok");
    let (r, a) = word_changes(old, new);
    assert_eq!((changed(old, &r), changed(new, &a)), (vec!["☕"], vec!["🍵"]));
}

#[test]
fn changed_words_with_an_unchanged_space_between_stay_two_ranges_and_touching_ones_join() {
    let (old, new) = ("a b c d", "a X Y d");
    let (r, a) = word_changes(old, new);
    assert_eq!((changed(old, &r), changed(new, &a)), (vec!["b", "c"], vec!["X", "Y"]));
    let (old, new) = ("foo(a)", "foo[b]");
    let (r, a) = word_changes(old, new);
    assert_eq!((changed(old, &r), changed(new, &a)), (vec!["(a)"], vec!["[b]"]));
}

#[test]
fn rows_that_replace_line_for_line_are_paired_in_order() {
    let pairs = pair_rows(&["let a = 1;", "let b = 2;"], &["let a = 10;", "let b = 20;"]);
    assert_eq!(pairs.len(), 2);
    assert_eq!((pairs[0].removed_row, pairs[0].added_row), (0, 0));
    assert_eq!((pairs[1].removed_row, pairs[1].added_row), (1, 1));
    assert_eq!(pairs[0].removed, vec![8..9]);
    assert_eq!(pairs[0].added, vec![8..10]);
}

#[test]
fn rows_with_nothing_in_common_are_not_paired() {
    assert!(pair_rows(&["fn old_name() {"], &["completely different"]).is_empty());
}

#[test]
fn a_line_split_in_two_pairs_the_first_new_row_and_leaves_the_other_alone() {
    let pairs = pair_rows(&["call(alpha, beta, gamma);"], &["call(alpha,", "    beta, gamma);"]);
    assert_eq!(pairs.len(), 1);
    assert_eq!((pairs[0].removed_row, pairs[0].added_row), (0, 0));
}

#[test]
fn extra_rows_on_either_side_have_no_partner() {
    let pairs = pair_rows(&["keep this line here", "gone gone gone"], &["keep this line there"]);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].removed_row, 0);
    let none = pair_rows(&[], &["new"]);
    assert!(none.is_empty());
}
