use super::{SessionReview, TurnReview};
use crate::{Change, Content, FileReview};

fn edit(path: &str, before: &str, after: &str) -> FileReview {
    FileReview::from_texts(path, Some(before.into()), Some(after.into()), true)
}

fn turn(files: Vec<FileReview>) -> TurnReview {
    TurnReview::new(files)
}

#[test]
fn a_turn_finds_its_files_by_path() {
    let t = turn(vec![edit("b.rs", "1\n", "2\n"), edit("a.rs", "1\n", "2\n"), edit("c/d.rs", "1\n", "2\n")]);
    let paths: Vec<_> = t.files().iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.rs", "b.rs", "c/d.rs"]);
    assert_eq!(t.file("b.rs").unwrap().path, "b.rs");
    assert!(t.file("zzz").is_none());
}

#[test]
fn one_turn_of_a_session_is_that_turns_own_hunks() {
    let mut session = SessionReview::new();
    assert_eq!(session.push(turn(vec![edit("a.rs", "1\n2\n", "1\nX\n")])), 0);
    assert_eq!(session.push(turn(vec![edit("a.rs", "1\nX\n", "1\nX\nY\n")])), 1);
    assert_eq!(session.turns()[0].file("a.rs").unwrap().counts(), (1, 1));
    assert_eq!(session.turns()[1].file("a.rs").unwrap().counts(), (1, 0));
}

#[test]
fn the_whole_session_is_each_file_against_its_text_before_the_first_turn_that_touched_it() {
    let mut session = SessionReview::new();
    session.push(turn(vec![edit("a.rs", "1\n2\n3\n", "1\nX\n3\n"), edit("only_first.rs", "a\n", "b\n")]));
    session.push(turn(vec![edit("a.rs", "1\nX\n3\n", "1\nX\nY\n"), edit("only_second.rs", "p\n", "q\n")]));
    let whole = session.whole();
    let paths: Vec<_> = whole.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.rs", "only_first.rs", "only_second.rs"]);
    let a = &whole[0];
    assert_eq!((a.before.as_deref(), a.after.as_deref()), (Some("1\n2\n3\n"), Some("1\nX\nY\n")));
    assert_eq!(a.counts(), (2, 2));
}

#[test]
fn a_file_a_later_turn_put_back_is_not_in_the_whole_session() {
    let mut session = SessionReview::new();
    session.push(turn(vec![edit("a.rs", "1\n", "2\n")]));
    session.push(turn(vec![edit("a.rs", "2\n", "1\n")]));
    assert!(session.whole().is_empty());
}

#[test]
fn what_the_user_rejected_in_an_earlier_turn_is_not_in_the_whole_session() {
    let mut session = SessionReview::new();
    // Turn one changed rows 1 and 3. The user kept the first, so turn two starts from that text.
    session.push(turn(vec![edit("a.rs", "1\n2\n3\n", "ONE\n2\nTHREE\n")]));
    session.push(turn(vec![edit("a.rs", "ONE\n2\n3\n", "ONE\nTWO\n3\n")]));
    let whole = session.whole();
    assert_eq!(whole.len(), 1);
    assert_eq!((whole[0].before.as_deref(), whole[0].after.as_deref()), (Some("1\n2\n3\n"), Some("ONE\nTWO\n3\n")));
}

#[test]
fn a_file_added_then_deleted_leaves_nothing_and_added_then_edited_stays_added() {
    let mut session = SessionReview::new();
    session.push(turn(vec![
        FileReview::from_texts("gone.rs", None, Some("x\n".into()), true),
        FileReview::from_texts("new.rs", None, Some("x\n".into()), true),
    ]));
    session.push(turn(vec![
        FileReview::from_texts("gone.rs", Some("x\n".into()), None, true),
        edit("new.rs", "x\n", "x\ny\n"),
    ]));
    let whole = session.whole();
    assert_eq!(whole.len(), 1);
    assert_eq!((whole[0].path.as_str(), &whole[0].change, whole[0].counts()), ("new.rs", &Change::Added, (2, 0)));
}

#[test]
fn a_file_that_was_binary_or_unknown_in_any_turn_stays_so() {
    let mut session = SessionReview::new();
    session.push(turn(vec![FileReview::binary("logo.png", Change::Added), FileReview::unknown("mystery.txt", Some("a\n".into()))]));
    session.push(turn(vec![edit("mystery.txt", "a\n", "b\n")]));
    let whole = session.whole();
    let content = |p: &str| whole.iter().find(|f| f.path == p).map(|f| f.content.clone());
    assert_eq!(content("logo.png"), Some(Content::Binary));
    assert_eq!(content("mystery.txt"), Some(Content::Unknown));
}

#[test]
fn an_empty_session_has_no_changes() {
    assert!(SessionReview::new().whole().is_empty());
}
