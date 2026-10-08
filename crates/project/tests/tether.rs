//! A process a project starts ends with the app that started it, even when the app is killed and the
//! process would keep running with its stdin closed. The test runs itself again as the "app": that run
//! starts a child, says its pid, and is killed.
#![cfg(unix)]

use std::{
    io::{BufRead, BufReader},
    process::{Command as Os, Stdio},
    thread,
    time::{Duration, Instant},
};

use atelier_project::{Command, LocalProject, Project};

const APP: &str = "ATELIER_TETHER_APP";
/// The folder the test hands to the app.
const APP_DIR: &str = "ATELIER_TETHER_DIR";

/// Whether process `pid` runs. A zombie, ended but not yet reaped, does not: where nothing reaps orphans, a
/// child the watchdog killed stays one.
fn alive(pid: &str) -> bool {
    Os::new("ps").args(["-o", "stat=", "-p", pid]).output().is_ok_and(|out| {
        let state = String::from_utf8_lossy(&out.stdout);
        let state = state.trim();
        !state.is_empty() && !state.starts_with('Z')
    })
}

/// The "app": starts a child that ignores the end of its stdin and outlives a TERM, and a grandchild under it,
/// says both pids, then waits to be killed.
#[test]
fn app_that_starts_a_stubborn_child() {
    if std::env::var_os(APP).is_none() {
        return;
    }
    // The folder is the test's, not the app's: the app is killed, and a killed app cannot remove what it made.
    let dir = std::path::PathBuf::from(std::env::var_os(APP_DIR).expect("the test names a folder for the app"));
    let project = LocalProject::open(&dir).unwrap();
    let script = "trap '' TERM HUP; sleep 300 & echo $$ $!; exec 0<&-; while :; do sleep 1; done";
    let mut child = project.spawn(&Command::new("sh").args(["-c", script])).unwrap();
    let mut pids = String::new();
    BufReader::new(&mut child.stdout).read_line(&mut pids).unwrap();
    println!("child {}", pids.trim());
    thread::sleep(Duration::from_secs(60));
}

/// Starts the app, kills it, and gives the pids of its child and grandchild once `reap` has run on it, and the folder the app
/// worked in, which goes when the test ends.
fn kill_the_app(reap: impl FnOnce(&mut std::process::Child)) -> (std::process::Child, Vec<String>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Os::new(std::env::current_exe().unwrap())
        .args(["app_that_starts_a_stubborn_child", "--exact", "--nocapture", "--test-threads=1"])
        .env(APP, "1")
        .env(APP_DIR, dir.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let pids: Vec<String> = BufReader::new(app.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
        .find_map(|line| line.rsplit_once("child ").map(|(_, pids)| pids.split_whitespace().map(str::to_string).collect()))
        .expect("the app says its child's pids");
    assert!(pids.iter().all(|pid| alive(pid)));
    app.kill().unwrap();
    reap(&mut app);
    (app, pids, dir)
}

/// The pids still running after the watchdog has had its time; each is killed, so no test leaves one behind.
fn left_after_a_while(pids: &[String]) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while pids.iter().any(|pid| alive(pid)) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    let left: Vec<String> = pids.iter().filter(|pid| alive(pid)).cloned().collect();
    for pid in &left {
        let _ = Os::new("kill").args(["-9", pid]).status();
    }
    left
}

#[test]
fn a_child_and_its_own_children_end_when_its_app_is_killed() {
    let (_app, pids, _dir) = kill_the_app(|app| drop(app.wait()));
    assert_eq!(left_after_a_while(&pids), Vec::<String>::new(), "outlived their app");
}

/// A killed app that nobody has reaped yet is a zombie: it is gone, and its child goes too.
#[test]
fn a_child_ends_when_its_app_is_killed_and_not_yet_reaped() {
    let (mut app, pids, _dir) = kill_the_app(|_| ());
    let left = left_after_a_while(&pids);
    app.wait().unwrap();
    assert_eq!(left, Vec::<String>::new(), "outlived their unreaped app");
}
