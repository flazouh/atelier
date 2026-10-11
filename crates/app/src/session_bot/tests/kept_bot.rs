use super::super::{kept_bot, seeded_bot};

#[test]
fn a_folder_nobody_opened_knows_the_starters_once_it_is_seeded() {
    let dir = tempfile::tempdir().unwrap();
    assert!(kept_bot(dir.path(), "dot").is_err(), "nothing is kept yet");
    let dot = seeded_bot(dir.path(), "dot").expect("the starters are seeded, then Dot is read");
    assert_eq!((dot.id.as_str(), dot.name.as_str()), ("dot", "Dot"));
    assert_eq!(kept_bot(dir.path(), "dot").unwrap(), dot, "and Dot is kept from then on");
}

#[test]
fn a_bot_that_is_not_kept_and_an_id_that_is_no_id_are_said_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let missing = seeded_bot(dir.path(), "nobody").unwrap_err();
    assert!(missing.contains("nobody"), "{missing}");
    let bad = seeded_bot(dir.path(), "Not An Id").unwrap_err();
    assert!(bad.contains("Not An Id"), "{bad}");
}
