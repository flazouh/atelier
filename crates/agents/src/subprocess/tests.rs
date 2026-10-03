use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use atelier_project::{Command, LocalProject, Project};

use super::{CANCELLED, run};

fn here() -> Arc<dyn Project> {
    Arc::new(LocalProject::open(std::env::temp_dir()).expect("the temp folder opens"))
}

#[test]
fn a_command_that_exits_zero_is_done() {
    assert!(run(here().as_ref(), &Command::new("sh").args(["-c", "echo hello"]), &AtomicBool::new(false)).is_ok());
}

#[test]
fn a_command_that_fails_says_how_and_the_last_thing_it_wrote() {
    let failing = Command::new("sh").args(["-c", "echo 'login was cancelled' >&2; exit 3"]);
    let why = run(here().as_ref(), &failing, &AtomicBool::new(false)).expect_err("a non-zero exit is not done");
    assert!(why.contains("code 3") && why.contains("login was cancelled"), "{why}");
}

#[test]
fn a_program_the_host_lacks_is_missing() {
    let why = run(here().as_ref(), &Command::new("atelier-no-such-program"), &AtomicBool::new(false)).expect_err("it cannot start");
    assert!(why.contains("atelier-no-such-program"), "{why}");
}

#[test]
fn a_command_that_runs_on_is_stopped_when_it_is_cancelled() {
    let stop = Arc::new(AtomicBool::new(false));
    let later = stop.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        later.store(true, Ordering::SeqCst);
    });
    let started = std::time::Instant::now();
    let why = run(here().as_ref(), &Command::new("sleep").args(["30"]), &stop).expect_err("a cancelled command is not done");
    assert!(started.elapsed() < std::time::Duration::from_secs(10), "it did not wait the thirty seconds");
    assert_eq!(why, CANCELLED);
}

#[test]
fn a_command_that_ends_before_it_is_cancelled_is_done() {
    let stop = AtomicBool::new(false);
    assert!(run(here().as_ref(), &Command::new("true"), &stop).is_ok());
}
