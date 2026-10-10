use crate::{
    BotId, BotStore, BotsError, DiskBotStore, Playbook, Run, RunState, seed_starters,
    starter_playbooks,
};

fn store() -> (tempfile::TempDir, DiskBotStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = DiskBotStore::new(dir.path().join("bots-data"));
    seed_starters(&store).unwrap();
    (dir, store)
}

fn id(text: &str) -> BotId {
    text.parse().unwrap()
}

fn deliver() -> Playbook {
    starter_playbooks().remove(0)
}

fn design() -> Playbook {
    starter_playbooks().remove(1)
}

fn new_run(name: &str, playbook: &Playbook, now_ms: i64) -> Run {
    Run::start(id(name), playbook, "Add a dark theme.", now_ms)
        .unwrap()
        .0
}

#[test]
fn a_run_comes_back_as_it_was_kept_from_one_file_of_its_own() {
    let (_dir, store) = store();
    assert!(store.runs().unwrap().is_empty(), "a new folder has no run");
    let run = new_run("run-1", &deliver(), 1_000);
    let run = run.step_starts(0, "s-0").unwrap().0;
    let run = run.step_ends(0, "Planned.").unwrap().0;
    store.save_run(&run).unwrap();
    assert!(store.root().join("runs").join("run-1.json").is_file());
    assert_eq!(store.run(&id("run-1")).unwrap(), run);
    // A new store on the same folder, as after the app starts again.
    let again = DiskBotStore::new(store.root());
    assert_eq!(again.runs().unwrap(), [run]);
}

#[test]
fn a_save_after_a_move_replaces_the_kept_run() {
    let (_dir, store) = store();
    let run = new_run("run-1", &deliver(), 1_000);
    store.save_run(&run).unwrap();
    let run = run.step_starts(0, "s-0").unwrap().0;
    store.save_run(&run).unwrap();
    assert_eq!(store.runs().unwrap().len(), 1);
    assert_eq!(
        store.run(&id("run-1")).unwrap().steps[0].session.as_deref(),
        Some("s-0")
    );
}

#[test]
fn runs_are_listed_the_newest_first() {
    let (_dir, store) = store();
    store.save_run(&new_run("b-old", &deliver(), 1_000)).unwrap();
    store.save_run(&new_run("a-new", &design(), 3_000)).unwrap();
    store.save_run(&new_run("c-mid", &deliver(), 2_000)).unwrap();
    store.save_run(&new_run("d-mid", &deliver(), 2_000)).unwrap();
    let ids: Vec<String> = store
        .runs()
        .unwrap()
        .into_iter()
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(
        ids,
        ["a-new", "c-mid", "d-mid", "b-old"],
        "two of the same time go by id"
    );
}

#[test]
fn a_missing_run_is_not_found_and_a_removed_one_is_gone() {
    let (_dir, store) = store();
    assert!(matches!(
        store.run(&id("ghost")),
        Err(BotsError::NotFound(_))
    ));
    store.save_run(&new_run("run-1", &deliver(), 1)).unwrap();
    store.remove_run(&id("run-1")).unwrap();
    assert!(store.runs().unwrap().is_empty());
    assert!(matches!(
        store.remove_run(&id("run-1")),
        Err(BotsError::NotFound(_))
    ));
}

#[test]
fn a_run_with_a_problem_is_refused_and_nothing_is_written() {
    let (_dir, store) = store();
    let mut run = new_run("run-1", &deliver(), 1);
    run.brief.clear();
    let err = store.save_run(&run).unwrap_err();
    assert!(
        matches!(&err, BotsError::Invalid(p) if p[0].contains("the brief")),
        "{err}"
    );
    assert!(store.runs().unwrap().is_empty());
}

#[test]
fn a_run_can_only_be_kept_when_the_bots_it_still_needs_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let store = DiskBotStore::new(dir.path());
    let err = store
        .save_run(&new_run("run-1", &design(), 1))
        .unwrap_err();
    assert!(
        matches!(&err, BotsError::Invalid(p) if p.len() == 4 && p[3].contains("still needs `quill`")),
        "{err}"
    );
    assert!(store.runs().unwrap().is_empty());
}

#[test]
fn a_bot_that_a_run_still_needs_cannot_be_removed() {
    let (_dir, store) = store();
    // No playbook names Dot, so only a run can hold it.
    let mut solo = deliver();
    solo.steps.truncate(1);
    solo.steps[0].bot = id("dot");
    let run = new_run("fix-1", &solo, 1);
    store.save_run(&run).unwrap();
    let err = store.remove_bot(&id("dot")).unwrap_err();
    assert!(
        matches!(&err, BotsError::Invalid(p) if p[0].contains("the run fix-1")),
        "{err}"
    );
    // The step at work, then failed: the run can still try it again, so it still needs the bot.
    let run = run.step_starts(0, "s-0").unwrap().0;
    let run = run.step_fails(0, "Out of quota.").unwrap().0;
    store.save_run(&run).unwrap();
    assert!(store.remove_bot(&id("dot")).is_err());
    assert_eq!(store.bots().unwrap().len(), 10);
}

#[test]
fn a_run_that_is_done_or_stopped_holds_no_bot_and_stays_as_a_record() {
    let (_dir, store) = store();
    let mut solo = deliver();
    solo.steps.truncate(1);
    solo.steps[0].bot = id("dot");
    let done = new_run("fix-1", &solo, 1).step_starts(0, "s-0").unwrap().0;
    let done = done.step_ends(0, "Fixed.").unwrap().0;
    store.save_run(&done).unwrap();
    solo.steps[0].bot = id("gus");
    let stopped = new_run("ops-1", &solo, 2).stop().unwrap().0;
    store.save_run(&stopped).unwrap();
    store.remove_bot(&id("dot")).unwrap();
    store.remove_bot(&id("gus")).unwrap();
    assert_eq!(store.run(&id("fix-1")).unwrap().state(), RunState::Done);
    assert_eq!(store.run(&id("fix-1")).unwrap().steps[0].bot, id("dot"));
    // The record can be kept again, though its bot is gone.
    store.save_run(&stopped).unwrap();
    assert_eq!(store.runs().unwrap().len(), 2);
}

#[test]
fn a_run_holds_only_the_bots_of_the_steps_it_has_left() {
    let (_dir, store) = store();
    let mut pair = deliver();
    pair.steps.truncate(2);
    pair.steps[0].bot = id("dot");
    pair.steps[1].bot = id("gus");
    let run = new_run("fix-1", &pair, 1).step_starts(0, "s-0").unwrap().0;
    let run = run.step_ends(0, "Fixed.").unwrap().0;
    store.save_run(&run).unwrap();
    store.remove_bot(&id("dot")).unwrap();
    assert!(store.remove_bot(&id("gus")).is_err(), "the step of Gus has not had its turn");
}

#[test]
fn a_playbook_can_be_removed_or_edited_while_a_run_of_it_goes_on() {
    let (_dir, store) = store();
    let run = new_run("run-1", &design(), 1);
    store.save_run(&run).unwrap();
    store.remove_playbook(&id("design")).unwrap();
    assert_eq!(store.run(&id("run-1")).unwrap(), run, "the run has its own steps");
    let err = store.remove_bot(&id("quill")).unwrap_err();
    assert!(
        matches!(&err, BotsError::Invalid(p) if p[0].contains("the run run-1")),
        "{err}"
    );
}

#[test]
fn a_broken_run_file_names_its_path() {
    let (_dir, store) = store();
    store.save_run(&new_run("run-1", &deliver(), 1)).unwrap();
    std::fs::write(store.root().join("runs").join("run-1.json"), "{ not json").unwrap();
    let err = store.runs().unwrap_err();
    assert!(
        matches!(&err, BotsError::Parse { path, .. } if path.ends_with("run-1.json")),
        "{err}"
    );
}
