use atelier_review::FileReview;

use super::*;

fn turn(files: &[(&str, Option<&str>, Option<&str>)]) -> TurnReview {
    TurnReview::new(files.iter().map(|(p, b, a)| FileReview::from_texts(*p, b.map(str::to_string), a.map(str::to_string), true)).collect())
}

/// Everything a review holds comes back from its record, through JSON as the data folder keeps it.
#[test]
fn a_review_comes_back_from_its_record() {
    let mut state = ReviewState::default();
    state.finish_turn(turn(&[("a.txt", Some("1\n2\n3\n"), Some("1\nTWO\n3\nFOUR\n")), ("new.txt", None, Some("n\n"))]), 4);
    let a = state.turns.turns()[0].file("a.txt").unwrap().clone();
    // The reader accepted a.txt's first hunk.
    let Content::Text(merged) = &a.content else { panic!("text") };
    let decided = merged.decide(&merged.hunks()[0].id, atelier_ui::Decision::Accept).unwrap();
    state.decided.insert((Scope::Turn(0), "a.txt".into()), (Some(decided.clone()), Some(decided.current())));
    state.set_reviewed(0, &a, true);
    let anchor = decided.anchor(0..1).unwrap();
    state.comments.add(0, "a.txt", anchor.clone(), "why?");
    let attachments = state.send_comments();
    assert_eq!(attachments.len(), 1);
    state.comments.add(0, "a.txt", anchor, "and this?");
    state.committed.insert((Scope::Turn(0), "a.txt".into()), "b89cbbe".into());
    state.approvals.insert("toolu_1".into(), Approval::AlwaysAllowed);
    state.last_activity = Some(1_790_700_000);
    let scratch = atelier_forge::RepoRef { host: "github.com".into(), owner: "flazouh".into(), name: "atelier-qa-scratch".into() };
    state.pull = Some(atelier_forge::PullRef { repo: scratch, number: 7 });

    let json = serde_json::to_string(&state.record()).unwrap();
    let back = ReviewState::from_record(serde_json::from_str(&json).unwrap());
    assert_eq!(back.record(), state.record(), "the same record");
    assert_eq!(back.turns.turns()[0].files(), state.turns.turns()[0].files());
    assert_eq!(back.turn_marks, [(4, 0)]);
    let kept = &back.decided[&(Scope::Turn(0), "a.txt".to_string())];
    assert_eq!(kept.0.as_ref().map(|m| m.hunks().len()), Some(1), "one hunk still to decide");
    assert_eq!(kept.0.as_ref().map(Merged::current), Some(decided.current()));
    assert!(back.is_reviewed(0, &a));
    assert_eq!(back.committed[&(Scope::Turn(0), "a.txt".to_string())], "b89cbbe");
    assert_eq!(back.approvals["toolu_1"], Approval::AlwaysAllowed);
    assert_eq!(back.last_activity, Some(1_790_700_000), "a resume knows when the agent last worked");
    assert_eq!(back.pull.as_ref().map(|p| p.number), Some(7));
    assert_eq!(back.comments.all().len(), 1);
    assert_eq!(back.sent.iter().map(|(c, answered)| (c.body.as_str(), *answered)).collect::<Vec<_>>(), [("why?", false)]);
}

/// A mark counts for the version of the file it was made on: one on a file that changed since stays
/// expired after the record comes back.
#[test]
fn a_mark_on_an_older_version_does_not_come_back() {
    let mut state = ReviewState::default();
    state.finish_turn(turn(&[("a.txt", Some("1\n"), Some("2\n"))]), 1);
    let a = state.turns.turns()[0].file("a.txt").unwrap().clone();
    state.set_reviewed(0, &a, true);
    let mut record = state.record();
    record.marks[0].2 ^= 1;
    assert!(!ReviewState::from_record(record).is_reviewed(0, &a));
}

#[test]
fn a_session_id_makes_a_safe_file_name() {
    assert_eq!(record_path("abc-123"), "review/abc-123.json");
    assert_eq!(record_path("../x/y"), "review/___x_y.json");
}
/// A record written before commits were kept still reads.
#[test]
fn a_record_with_no_commits_reads() {
    let back: Record = serde_json::from_str(r#"{"turns":[],"turn_marks":[],"decided":[],"marks":[],"comments":[],"sent":[]}"#).unwrap();
    assert!(ReviewState::from_record(back).committed.is_empty());
}
