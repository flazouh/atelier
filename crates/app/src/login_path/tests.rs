use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};
use super::helpers::{ask, marked, merge};

fn shell(body: &str) -> String {
    let dir = crate::test_dirs::path();
    let path = dir.join("fake-shell");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn the_login_shells_folders_come_first_then_the_ones_already_set() {
    let path = merge(Some("/a/bin:/usr/bin"), "/usr/bin:/bin", Path::new("/home/x"), |_| false);
    assert_eq!(path, "/a/bin:/usr/bin:/bin");
}

#[test]
fn no_folder_is_named_twice_and_none_is_empty() {
    assert_eq!(merge(Some("/a::/a:/b"), ":/b:/c:", Path::new("/h"), |_| false), "/a:/b:/c");
}

#[test]
fn the_installers_folders_are_added_only_when_they_exist_and_the_shell_did_not_name_them() {
    let exists = |dir: &Path| dir == Path::new("/home/x/.local/bin") || dir == Path::new("/opt/homebrew/bin");
    assert_eq!(
        merge(None, "/usr/bin:/bin", Path::new("/home/x"), exists),
        "/usr/bin:/bin:/home/x/.local/bin:/opt/homebrew/bin",
        "the app opened from the Finder finds `claude` in ~/.local/bin and git from Homebrew"
    );
    assert_eq!(merge(Some("/opt/homebrew/bin"), "/usr/bin", Path::new("/home/x"), exists), "/opt/homebrew/bin:/usr/bin:/home/x/.local/bin");
}

#[test]
fn only_what_is_inside_the_marks_is_the_path() {
    assert_eq!(marked("Welcome back!\n__atelier_path__/a:/b__atelier_path__\nbye"), Some("/a:/b"));
    assert_eq!(marked("no marks"), None);
    assert_eq!(marked("__atelier_path____atelier_path__"), None, "an empty answer is no answer");
    assert_eq!(marked("__atelier_path__/a, never closed"), None);
}

#[test]
fn a_shell_that_prints_a_banner_still_gives_its_path() {
    let fake = shell("echo 'Last login: today'; printf '%s' '__atelier_path__/x/bin:/usr/bin__atelier_path__'");
    assert_eq!(ask(&fake, Duration::from_secs(5)).as_deref(), Some("/x/bin:/usr/bin"));
}

#[test]
fn a_shell_that_hangs_is_given_up_on_and_stopped() {
    let fake = shell("sleep 30");
    let started = std::time::Instant::now();
    assert_eq!(ask(&fake, Duration::from_millis(150)), None);
    assert!(started.elapsed() < Duration::from_secs(5), "the wait ends at the limit");
}

#[test]
fn a_shell_that_fails_or_is_missing_leaves_the_path_alone() {
    assert_eq!(ask(&shell("exit 3"), Duration::from_secs(5)), None);
    assert_eq!(ask("/no/such/shell", Duration::from_secs(1)), None);
}
