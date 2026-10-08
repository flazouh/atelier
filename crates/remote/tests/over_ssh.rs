//! The whole path over a real `ssh`: probe, deploy, dial, and a project that answers. It needs a
//! host that takes the user's key, and a atelier-remote built for it in `$ATELIER_REMOTE_DIR/<platform>/`:
//!     cargo build -p atelier-remote && mkdir -p /tmp/remote/linux-x86_64 && cp target/debug/atelier-remote /tmp/remote/linux-x86_64/
//!     ATELIER_REMOTE_DIR=/tmp/remote ATELIER_TEST_SSH_HOST=dev-host cargo test -p atelier-remote --test over_ssh -- --ignored --nocapture

use atelier_project::Project;

#[test]
#[ignore]
fn a_folder_over_ssh_lists_and_reads() {
    let host = std::env::var("ATELIER_TEST_SSH_HOST").expect("ATELIER_TEST_SSH_HOST names a host");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "over ssh\n").unwrap();
    let project = atelier_remote::ssh::connect(&host, &dir.path().display().to_string(), &|line| eprintln!("{line}")).expect("it connects");
    assert_eq!(project.read("hello.txt").unwrap(), b"over ssh\n");
    assert!(project.list().unwrap().iter().any(|e| e.path == "hello.txt"));
    // Protocol 2: a remove, and the data folder on the host.
    project.remove("hello.txt").unwrap();
    assert!(!dir.path().join("hello.txt").exists());
    project.data_write("over-ssh/check.txt", b"kept").unwrap();
    assert_eq!(project.data_read("over-ssh/check.txt").unwrap(), b"kept");
    assert!(project.data_list("over-ssh").unwrap().iter().any(|e| e.path == "over-ssh/check.txt"));
    // Protocol 3: the data folder's path on the host, where the check landed.
    let folder = project.data_path().expect("the host names its data folder");
    assert!(folder.join("over-ssh/check.txt").exists(), "{}", folder.display());
}

#[test]
#[ignore]
fn a_host_that_does_not_exist_says_so() {
    let error = atelier_remote::ssh::connect("atelier-no-such-host.invalid", "/", &|_| {}).err().expect("no host");
    eprintln!("{error}");
    assert!(error.to_string().contains("atelier-no-such-host.invalid"), "{error}");
    assert_eq!(error.to_string().matches("atelier-no-such-host.invalid").count(), 1, "ssh's own words, not a second prefix: {error}");
}

/// What a remote project costs over a real ssh: the connect (probe, the copy's check, the dial and
/// the hello), a file open (the read of a 10,000-line file), and the listing of `ATELIER_TEST_SSH_ROOT`.
/// Targets: connect < 3 s, file open < 150 ms on a LAN, listing a 1,000-file tree < 500 ms.
///     ATELIER_REMOTE_DIR=… ATELIER_TEST_SSH_HOST=dev-host ATELIER_TEST_SSH_ROOT=/home/user/code/local/atelier \
///         cargo test --release -p atelier-remote --test over_ssh -- --ignored --nocapture remote_costs
#[test]
#[ignore]
fn remote_costs() {
    use std::time::{Duration, Instant};
    let host = std::env::var("ATELIER_TEST_SSH_HOST").expect("ATELIER_TEST_SSH_HOST names a host");
    let root = std::env::var("ATELIER_TEST_SSH_ROOT").expect("ATELIER_TEST_SSH_ROOT names a folder on it");
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    let at = Instant::now();
    let project = atelier_remote::ssh::connect(&host, &root, &|_| {}).expect("it connects");
    println!("connect: {:.1} ms", ms(at.elapsed()));
    let text: String = (0..10_000).map(|i| format!("let line_{i} = {i};\n")).collect();
    project.write("atelier-bench.rs", text.as_bytes()).unwrap();
    let mut reads: Vec<Duration> = (0..20)
        .map(|_| {
            let at = Instant::now();
            assert_eq!(project.read("atelier-bench.rs").unwrap().len(), text.len());
            at.elapsed()
        })
        .collect();
    reads.sort();
    println!("file open, {} KB: median {:.2} ms, p95 {:.2} ms", text.len() / 1024, ms(reads[10]), ms(reads[18]));
    let mut lists: Vec<Duration> = (0..20)
        .map(|_| {
            let at = Instant::now();
            project.list().unwrap();
            at.elapsed()
        })
        .collect();
    lists.sort();
    let files = project.list().unwrap().iter().filter(|e| !e.dir).count();
    println!("listing {files} files: median {:.2} ms, p95 {:.2} ms", ms(lists[10]), ms(lists[18]));
    std::fs::remove_file(std::path::Path::new(&root).join("atelier-bench.rs")).ok();
    let _ = std::process::Command::new("ssh").args([host.as_str(), &format!("rm -f {root}/atelier-bench.rs")]).status();
}

/// Protocol 5: the tasks of a project over a real ssh live in the host's data folder.
#[test]
#[ignore]
fn tasks_over_ssh_live_on_the_host() {
    use atelier_tracker::{NewTask, Tracker};
    let host = std::env::var("ATELIER_TEST_SSH_HOST").expect("ATELIER_TEST_SSH_HOST names a host");
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("tasks")).unwrap();
    let root = dir.path().join("tasks");
    let project = atelier_remote::ssh::connect(&host, &root.display().to_string(), &|line| eprintln!("{line}")).expect("connects");
    let tracker = project.tracker().expect("the host opens its store");
    let task = tracker.create(&NewTask::titled("Over ssh"), "qa").unwrap();
    assert!(task.key.starts_with("TAS-"), "{}", task.key);
    assert_eq!(tracker.get(&task.id).unwrap().map(|t| t.title), Some("Over ssh".to_string()));
    let file = project.data_path().expect("a data folder").join("tracker.sqlite");
    let local = atelier_tracker::LocalTracker::open(&file, "TAS").unwrap();
    assert_eq!(local.get(&task.id).unwrap().map(|t| t.key), Some(task.key.clone()));
}
