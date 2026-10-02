use std::{
    os::unix::process::CommandExt,
    process::{Command as Os, Stdio},
    thread,
    time::Duration,
};

use super::{Control, LocalChild, TETHER};

/// A watchdog that cannot start is an error for the spawn to report, not a child left unguarded.
#[test]
fn a_watchdog_that_cannot_start_is_an_error() {
    assert!(super::tether_with("/nonexistent/atelier-sh", std::process::id()).is_err());
}

fn sleeper() -> std::process::Child {
    Os::new("sleep").arg("30").process_group(0).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap()
}

fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// A kill that works is a kill that succeeded, with the child still unreaped and after it was reaped too.
#[test]
fn killing_a_child_succeeds_before_and_after_it_is_waited_for() {
    let mut child = LocalChild::new(sleeper());
    assert!(child.kill().is_ok());
    assert!(child.wait().is_ok());
    assert!(child.kill().is_ok());
}

/// The watchdog asks `ps` whether a zombie app is gone, but a host without `ps` is no host whose app is
/// gone: the child lives while the app does.
#[test]
fn a_watchdog_on_a_host_without_ps_keeps_the_child_while_the_app_runs() {
    let bin = tempfile::tempdir().unwrap();
    let sleep = ["/bin/sleep", "/usr/bin/sleep"].into_iter().find(|p| std::path::Path::new(p).exists()).unwrap();
    std::os::unix::fs::symlink(sleep, bin.path().join("sleep")).unwrap();
    let mut child = sleeper();
    let watchdog = Os::new("/bin/sh")
        .args(["-c", TETHER, "atelier-tether", &std::process::id().to_string(), &child.id().to_string()])
        .env("PATH", bin.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(watchdog.success());
    thread::sleep(Duration::from_millis(3500));
    let running = child.try_wait().unwrap().is_none();
    let _ = unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
    let _ = child.wait();
    assert!(running, "the watchdog ended the child of an app that runs");
}

/// Once the child's group is gone too, nothing is sent to its id, which another group may hold by then.
#[test]
fn a_watchdog_signals_no_group_that_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    let mut gone = Os::new("true").spawn().unwrap();
    let app = gone.id();
    let _ = gone.wait();
    assert!(!alive(app));
    let script = format!("kill() {{ echo \"$*\" >> '{}'; return 1; }}\n{TETHER}", log.display());
    let status = Os::new("/bin/sh").args(["-c", &script, "atelier-tether", &app.to_string(), "999999"]).status().unwrap();
    assert!(status.success());
    thread::sleep(Duration::from_millis(3500));
    let sent = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(sent.contains("-0 -999999"), "the watchdog asks whether the group exists: {sent}");
    assert!(!sent.contains("TERM"), "the watchdog stops a group that is gone: {sent}");
}
