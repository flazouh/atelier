use std::sync::Arc;

use atelier_project::{Command, LocalProject, Project};

use super::run;

fn here() -> Arc<dyn Project> {
    Arc::new(LocalProject::open(std::env::temp_dir()).expect("the temp folder opens"))
}

#[test]
fn a_command_that_exits_zero_is_done() {
    assert!(run(here().as_ref(), &Command::new("sh").args(["-c", "echo hello"])).is_ok());
}

#[test]
fn a_command_that_fails_says_how_and_the_last_thing_it_wrote() {
    let failing = Command::new("sh").args(["-c", "echo 'login was cancelled' >&2; exit 3"]);
    let why = run(here().as_ref(), &failing).expect_err("a non-zero exit is not done");
    assert!(why.contains("code 3") && why.contains("login was cancelled"), "{why}");
}

#[test]
fn a_program_the_host_lacks_is_missing() {
    let why = run(here().as_ref(), &Command::new("atelier-no-such-program")).expect_err("it cannot start");
    assert!(why.contains("atelier-no-such-program"), "{why}");
}
