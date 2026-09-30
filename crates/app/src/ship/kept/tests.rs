use lathe_review::{FileReview, Merged};

use super::*;

fn file(path: &str, before: Option<&str>, after: Option<&str>) -> FileReview {
    FileReview::from_texts(path, before.map(str::to_string), after.map(str::to_string), true)
}

fn merged(file: &FileReview) -> Merged {
    match &file.content {
        lathe_review::Content::Text(m) => m.clone(),
        _ => panic!("text"),
    }
}

/// What a file keeps is its text before the turn with the accepted hunks in it: undecided and rejected
/// hunks stay out.
#[test]
fn a_file_keeps_its_accepted_hunks_only() {
    let f = file("a.txt", Some("1\n2\n3\n"), Some("1\nTWO\n3\nFOUR\n"));
    let m = merged(&f);
    assert_eq!(kept(&f, Some(&m)), None, "nothing decided, nothing kept");
    let accepted = m.decide(&m.hunks()[0].id, beui::Decision::Accept).unwrap();
    assert_eq!(kept(&f, Some(&accepted)), Some(Kept { path: "a.txt".into(), text: Some("1\nTWO\n3\n".into()) }));
    let rejected = m.decide(&m.hunks()[0].id, beui::Decision::Reject).unwrap();
    assert_eq!(kept(&f, Some(&rejected)), None, "a rejected hunk keeps the old text");
}

/// A new file the reader accepted is added; one they rejected is not. A deletion accepted whole is a
/// removal. A file with no text keeps nothing.
#[test]
fn new_files_deletions_and_binaries() {
    let new = file("n.txt", None, Some("n\n"));
    let m = merged(&new);
    let all = |m: &Merged, d| m.hunks().iter().fold(m.clone(), |acc, h| acc.decide(&h.id, d).unwrap_or(acc));
    assert_eq!(kept(&new, Some(&all(&m, beui::Decision::Accept))), Some(Kept { path: "n.txt".into(), text: Some("n\n".into()) }));
    assert_eq!(kept(&new, Some(&all(&m, beui::Decision::Reject))), None);
    let gone = file("g.txt", Some("g\n"), None);
    let m = merged(&gone);
    assert_eq!(kept(&gone, Some(&all(&m, beui::Decision::Accept))), Some(Kept { path: "g.txt".into(), text: None }));
    assert_eq!(kept(&gone, None), None);
}
