//! The whole path over a real `ssh`: probe, deploy, dial, and a project that answers. It needs a
//! host that takes the user's key and a lathe-remote built for it:
//!     cargo build -p lathe-remote && LATHE_TEST_SSH_HOST=hp-agent \
//!         cargo test -p lathe-remote --test over_ssh -- --ignored --nocapture

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
}

#[test]
#[ignore]
fn a_host_that_does_not_exist_says_so() {
    let error = lathe_remote::ssh::connect("lathe-no-such-host.invalid", "/", &|_| {}).err().expect("no host");
    eprintln!("{error}");
    assert!(error.to_string().starts_with("lathe-no-such-host.invalid: "), "{error}");
}
