//! The whole path over a real `ssh`: probe, deploy, dial, and a project that answers. It needs a
//! host that takes the user's key, and a lathe-remote built for it in `$LATHE_REMOTE_DIR/<platform>/`:
//!     cargo build -p lathe-remote && mkdir -p /tmp/remote/linux-x86_64 && cp target/debug/lathe-remote /tmp/remote/linux-x86_64/
//!     LATHE_REMOTE_DIR=/tmp/remote LATHE_TEST_SSH_HOST=hp-agent cargo test -p lathe-remote --test over_ssh -- --ignored --nocapture

use lathe_project::Project;

#[test]
#[ignore]
fn a_folder_over_ssh_lists_and_reads() {
    let host = std::env::var("LATHE_TEST_SSH_HOST").expect("LATHE_TEST_SSH_HOST names a host");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "over ssh\n").unwrap();
    let project = lathe_remote::ssh::connect(&host, &dir.path().display().to_string(), &|line| eprintln!("{line}")).expect("it connects");
    assert_eq!(project.read("hello.txt").unwrap(), b"over ssh\n");
    assert!(project.list().unwrap().iter().any(|e| e.path == "hello.txt"));
    // Protocol 2: a remove, and the data folder on the host.
    project.remove("hello.txt").unwrap();
    assert!(!dir.path().join("hello.txt").exists());
    project.data_write("over-ssh/check.txt", b"kept").unwrap();
    assert_eq!(project.data_read("over-ssh/check.txt").unwrap(), b"kept");
    assert!(project.data_list("over-ssh").unwrap().iter().any(|e| e.path == "over-ssh/check.txt"));
}

#[test]
#[ignore]
fn a_host_that_does_not_exist_says_so() {
    let error = lathe_remote::ssh::connect("lathe-no-such-host.invalid", "/", &|_| {}).err().expect("no host");
    eprintln!("{error}");
    assert!(error.to_string().contains("lathe-no-such-host.invalid"), "{error}");
    assert_eq!(error.to_string().matches("lathe-no-such-host.invalid").count(), 1, "ssh's own words, not a second prefix: {error}");
}

/// What a remote project costs over a real ssh: the connect (probe, the copy's check, the dial and
/// the hello), a file open (the read of a 10,000-line file), and the listing of `LATHE_TEST_SSH_ROOT`.
/// Targets: connect < 3 s, file open < 150 ms on a LAN, listing a 1,000-file tree < 500 ms.
///     LATHE_REMOTE_DIR=… LATHE_TEST_SSH_HOST=hp-agent LATHE_TEST_SSH_ROOT=/home/alex/code/local/lathe \
///         cargo test --release -p lathe-remote --test over_ssh -- --ignored --nocapture remote_costs
#[test]
#[ignore]
fn remote_costs() {
    use std::time::{Duration, Instant};
    let host = std::env::var("LATHE_TEST_SSH_HOST").expect("LATHE_TEST_SSH_HOST names a host");
    let root = std::env::var("LATHE_TEST_SSH_ROOT").expect("LATHE_TEST_SSH_ROOT names a folder on it");
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    let at = Instant::now();
    let project = lathe_remote::ssh::connect(&host, &root, &|_| {}).expect("it connects");
    println!("connect: {:.1} ms", ms(at.elapsed()));
    let text: String = (0..10_000).map(|i| format!("let line_{i} = {i};\n")).collect();
    project.write("lathe-bench.rs", text.as_bytes()).unwrap();
    let mut reads: Vec<Duration> = (0..20)
        .map(|_| {
            let at = Instant::now();
            assert_eq!(project.read("lathe-bench.rs").unwrap().len(), text.len());
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
    std::fs::remove_file(std::path::Path::new(&root).join("lathe-bench.rs")).ok();
    let _ = std::process::Command::new("ssh").args([host.as_str(), &format!("rm -f {root}/lathe-bench.rs")]).status();
}
