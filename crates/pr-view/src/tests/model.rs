use lathe_forge::{Change, CheckStatus, Conclusion, ForgeError, JobRef, RepoRef};

use crate::{
    Part, PartKind, PullData,
    base::{Base, BaseChoice},
    fixture::sample,
    git::{Commit, FileEntry},
    model::{Place, PrModel},
};

fn entry(path: &str, blob: &str) -> FileEntry {
    FileEntry { path: path.into(), old_path: None, change: Change::Modified, additions: 2, deletions: 1, binary: false, old_blob: Some("old".into()), new_blob: Some(blob.into()) }
}

fn base() -> Base {
    Base { choice: BaseChoice::Whole, sha: "b".repeat(40), note: None }
}

fn model_with(paths: &[(&str, &str)]) -> PrModel {
    let reference = sample::reference(1);
    let mut model = PrModel::new(reference, "alex", sample::data(1, &"h".repeat(40), Vec::new()));
    model.set_git(paths.iter().map(|(p, b)| entry(p, b)).collect(), Vec::new(), base(), "h".repeat(40));
    model
}

#[test]
fn the_first_file_is_open_and_next_and_previous_walk_the_tree_order() {
    let mut model = model_with(&[("src/b.rs", "1"), ("src/a.rs", "2"), ("README.md", "3")]);
    let order = model.order();
    assert_eq!(model.place.as_ref().unwrap().path, order[0].as_ref(), "the first file in the tree is open");
    assert!(model.step(1));
    assert_eq!(model.place.as_ref().unwrap().path, order[1].as_ref());
    assert!(model.step(1));
    assert_eq!(model.place.as_ref().unwrap().path, order[2].as_ref());
    assert!(model.step(-1));
    assert_eq!(model.place.as_ref().unwrap().path, order[1].as_ref());
}

#[test]
fn a_file_is_seen_for_its_version_and_a_push_that_changes_it_unsees_it() {
    let mut model = model_with(&[("a.rs", "v1"), ("b.rs", "v1")]);
    model.open("a.rs");
    let change = model.toggle_seen().unwrap();
    assert_eq!((change.path.as_str(), change.version.as_str(), change.seen), ("a.rs", "v1", true));
    assert!(model.is_seen("a.rs") && !model.is_seen("b.rs"));
    assert_eq!(model.progress().reviewed, 1);
    assert_eq!(model.progress().files, 2);
    assert_eq!(model.seen_set().len(), 1);
    // The same file, pushed to a new version: not seen any more. The other is left as it was.
    let marks = model.marks.clone();
    model.set_git(vec![entry("a.rs", "v2"), entry("b.rs", "v1")], Vec::new(), base(), "h".repeat(40));
    model.marks = marks;
    assert!(!model.is_seen("a.rs"));
    // Marking again marks the new version; marking a seen file takes it back.
    model.open("a.rs");
    assert!(model.toggle_seen().unwrap().seen && model.is_seen("a.rs"));
    let back = model.toggle_seen().unwrap();
    assert!(!back.seen && !model.is_seen("a.rs"));
}

#[test]
fn put_all_back_clears_the_marks() {
    let mut model = model_with(&[("a.rs", "1"), ("b.rs", "2")]);
    model.toggle_seen();
    model.step(1);
    model.toggle_seen();
    assert_eq!(model.progress().reviewed, 2);
    assert!(model.put_back());
    assert_eq!(model.progress().reviewed, 0);
    assert!(!model.put_back());
}

#[test]
fn a_file_brought_in_is_out_of_the_count_and_the_walk_goes_on_from_where_it_came() {
    let mut model = model_with(&[("a.rs", "1"), ("b.rs", "2"), ("c.rs", "3")]);
    model.open("b.rs");
    model.jump_to("src/other.rs", Some((4, 2)));
    let place = model.place.clone().unwrap();
    assert_eq!(place, Place { path: "src/other.rs".into(), brought_in: true });
    assert!(model.toggle_seen().is_none(), "a file brought in is not marked");
    assert_eq!(model.progress().files, 3);
    // Next from a file brought in goes to the file after the one it was reached from.
    assert!(model.step(1));
    assert_eq!(model.place.as_ref().unwrap().path, "c.rs");
    assert!(model.back.is_empty(), "opening a file from the keys starts the way back afresh");
}

#[test]
fn the_way_back_returns_along_the_stack_with_the_caret() {
    let mut model = model_with(&[("a.rs", "1"), ("b.rs", "2")]);
    model.open("a.rs");
    model.jump_to("b.rs", Some((3, 1)));
    assert!(!model.place.as_ref().unwrap().brought_in, "b.rs is a changed file");
    model.jump_to("lib/x.rs", Some((9, 9)));
    assert_eq!(model.go_back(), Some(Some((9, 9))));
    assert_eq!(model.place.as_ref().unwrap().path, "b.rs");
    assert_eq!(model.go_back(), Some(Some((3, 1))));
    assert_eq!(model.place.as_ref().unwrap().path, "a.rs");
    assert_eq!(model.go_back(), None);
}

#[test]
fn the_place_is_kept_when_the_file_list_changes() {
    let mut model = model_with(&[("a.rs", "1"), ("b.rs", "2"), ("c.rs", "3")]);
    model.open("b.rs");
    model.set_git(vec![entry("a.rs", "1"), entry("b.rs", "9"), entry("d.rs", "4")], Vec::new(), base(), "h".repeat(40));
    assert_eq!(model.place.as_ref().unwrap().path, "b.rs", "still there, and open");
    model.set_git(vec![entry("a.rs", "1"), entry("d.rs", "4")], Vec::new(), base(), "h".repeat(40));
    assert_eq!(model.place.as_ref().unwrap().path, "d.rs", "the file that stands where it stood");
    model.set_git(Vec::new(), Vec::new(), base(), "h".repeat(40));
    assert!(model.place.is_none());
    model.set_git(vec![entry("z.rs", "1")], Vec::new(), base(), "h".repeat(40));
    assert_eq!(model.place.as_ref().unwrap().path, "z.rs", "with files again, the first opens");
}

#[test]
fn a_file_brought_in_stays_open_when_the_list_changes() {
    let mut model = model_with(&[("a.rs", "1")]);
    model.jump_to("elsewhere.rs", None);
    model.set_git(vec![entry("a.rs", "2")], Vec::new(), base(), "h".repeat(40));
    assert_eq!(model.place, Some(Place { path: "elsewhere.rs".into(), brought_in: true }));
}

#[test]
fn until_git_answers_the_forges_file_list_stands_in() {
    let reference = sample::reference(2);
    let files = vec![lathe_forge::ChangedFile { path: "x.rs".into(), additions: 1, deletions: 0, change: Change::Added }];
    let mut model = PrModel::new(reference.clone(), "alex", PullData::new(reference));
    assert!(!model.ready() && model.files().is_empty());
    model.apply_part(Part::Files(files), 5);
    assert_eq!(model.files().len(), 1);
    assert_eq!(model.place.as_ref().unwrap().path, "x.rs");
    assert!(model.toggle_seen().is_none(), "there is no version to mark before git answers");
}

#[test]
fn a_part_that_fails_is_remembered_until_it_arrives() {
    let mut model = model_with(&[]);
    model.apply_part(Part::Failed { part: PartKind::Checks, error: ForgeError::Offline }, 1);
    assert_eq!(model.error_of(PartKind::Checks), Some(&ForgeError::Offline));
    model.apply_part(Part::Failed { part: PartKind::Checks, error: ForgeError::NotSignedIn }, 2);
    assert_eq!(model.errors.len(), 1, "one failure a part");
    model.apply_part(Part::Checks(Vec::new()), 3);
    assert!(model.error_of(PartKind::Checks).is_none());
}

#[test]
fn a_pull_whose_head_git_has_not_seen_asks_for_git_again() {
    let mut model = model_with(&[("a.rs", "1")]);
    let mut pull = model.pull().unwrap().clone();
    let effect = model.apply_part(Part::Pull(Box::new(pull.clone())), 5);
    assert!(!effect.head_moved, "the head git was prepared for");
    pull.head_sha = "f".repeat(40);
    assert!(model.apply_part(Part::Pull(Box::new(pull)), 6).head_moved);
}

#[test]
fn the_rail_reads_the_data_the_way_the_parts_want_it() {
    let mut model = model_with(&[("a.rs", "1")]);
    model.data.threads = vec![
        { let mut t = sample::thread("done", "a.rs", 1, vec![sample::comment("1", "Ada", "old", 1)]); t.resolved = true; t },
        sample::thread("open", "a.rs", 2, vec![sample::comment("2", "Rui", "new", 2)]),
    ];
    model.data.remarks = vec![sample::comment("r", "bot", "built", 3)];
    let (threads, remarks) = model.conversation(sample::NOW);
    assert_eq!(threads.iter().map(|t| t.resolved).collect::<Vec<_>>(), [false, true], "open first");
    assert_eq!(remarks.len(), 1);
    assert_eq!(model.unsent(), 0);
    assert!(!model.in_review());
    model.data.held = vec![lathe_forge::HeldComment { thread: lathe_forge::ThreadId("h".into()), comment: sample::comment("h", "alex", "x", 1), path: "a.rs".into(), line: Some(1) }];
    assert!((model.unsent(), model.in_review()) == (1, true));
    assert!(!model.mine(), "the fixture's author is Rui");
    assert!(model.merge_facts(&[]).is_some());
}

#[test]
fn failing_jobs_are_read_a_few_at_a_time_and_only_once() {
    let mut model = model_with(&[]);
    let repo = RepoRef::new("github.com", "o", "r");
    model.data.checks = (1..=12)
        .map(|i| {
            let mut c = sample::check(&format!("job {i}"), CheckStatus::Done, Some(Conclusion::Failure));
            c.job = Some(JobRef { repo: repo.clone(), id: i });
            c
        })
        .collect();
    assert_eq!(model.jobs_to_read(8).len(), 8);
    model.jobs.insert(1, crate::checks::JobLog { job: lathe_forge::Job { reference: JobRef { repo, id: 1 }, name: "x".into(), status: CheckStatus::Done, conclusion: None, run_id: 1, attempt: 1, steps: Vec::new() }, log: String::new() });
    let next: Vec<u64> = model.jobs_to_read(8).iter().map(|j| j.id).collect();
    assert!(!next.contains(&1) && next.len() == 8 && next[0] == 2);
    let _ = Commit { sha: String::new(), title: String::new(), author: String::new(), at: 0 };
}
