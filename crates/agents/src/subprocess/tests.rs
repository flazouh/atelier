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

use super::{exit_why, stderr_tail, strip_ansi};

#[test]
fn colour_codes_are_not_words() {
    assert_eq!(strip_ansi("Run \u{1b}[36msecurity find-identity\u{1b}[0m to see them"), "Run security find-identity to see them");
    assert_eq!(strip_ansi("\u{1b}[1;31merror:\u{1b}[0m the key is missing"), "error: the key is missing");
    assert_eq!(strip_ansi("\u{1b}[2K\u{1b}[1Gready"), "ready", "cursor codes go too");
    assert_eq!(strip_ansi("plain, with é and 日本語"), "plain, with é and 日本語");
    assert_eq!(strip_ansi(""), "");
}

#[test]
fn a_title_or_a_link_goes_to_its_end() {
    assert_eq!(strip_ansi("a\u{1b}]0;title\u{7}b"), "ab");
    assert_eq!(strip_ansi("\u{1b}]8;;https://x.test\u{1b}\\link\u{1b}]8;;\u{1b}\\"), "link");
}

#[test]
fn a_cut_off_escape_takes_nothing_else() {
    assert_eq!(strip_ansi("end\u{1b}"), "end");
    assert_eq!(strip_ansi("end\u{1b}[31"), "end");
    assert_eq!(strip_ansi("a\u{1b}Mb"), "ab", "a two-character escape");
}

#[test]
fn the_tail_a_row_shows_has_no_colour() {
    let stderr = "starting\n\u{1b}[31merror\u{1b}[0m: no identity\nRun \u{1b}[36msecurity\u{1b}[0m\n\n";
    assert_eq!(stderr_tail(stderr), "starting\nerror: no identity\nRun security");
    assert_eq!(exit_why(Some(1), &stderr_tail(stderr)), "the agent exited with code 1: Run security");
}
