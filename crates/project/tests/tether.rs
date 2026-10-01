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

fn alive(pid: &str) -> bool {
    Os::new("kill").args(["-0", pid]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// The "app": starts a child that ignores the end of its stdin and outlives a TERM, then waits to be killed.
#[test]
fn app_that_starts_a_stubborn_child() {
    if std::env::var_os(APP).is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let project = LocalProject::open(dir.path()).unwrap();
    let mut child = project.spawn(&Command::new("sh").args(["-c", "trap '' TERM HUP; echo $$; exec 0<&-; while :; do sleep 1; done"])).unwrap();
    let mut pid = String::new();
    BufReader::new(&mut child.stdout).read_line(&mut pid).unwrap();
    println!("child {}", pid.trim());
    thread::sleep(Duration::from_secs(60));
}

#[test]
fn a_child_ends_when_its_app_is_killed() {
    let mut app = Os::new(std::env::current_exe().unwrap())
        .args(["app_that_starts_a_stubborn_child", "--exact", "--nocapture", "--test-threads=1"])
        .env(APP, "1")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = BufReader::new(app.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
        .find_map(|line| line.rsplit_once("child ").map(|(_, pid)| pid.trim().to_string()))
        .expect("the app says its child's pid");
    assert!(alive(&pid));
    app.kill().unwrap();
    app.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while alive(&pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    let left = alive(&pid);
    if left {
        let _ = Os::new("kill").args(["-9", &pid]).status();
    }
    assert!(!left, "the child {pid} outlived its app");
}
