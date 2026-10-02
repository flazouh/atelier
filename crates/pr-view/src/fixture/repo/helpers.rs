use std::{path::Path, process::Command};

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Rui")
        .env("GIT_AUTHOR_EMAIL", "rui@example.test")
        .env("GIT_COMMITTER_NAME", "Rui")
        .env("GIT_COMMITTER_EMAIL", "rui@example.test")
        .env("GIT_AUTHOR_DATE", "2026-09-01T10:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-09-01T10:00:00Z")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Whether git succeeds, for a question that may be answered no.
pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").args(args).current_dir(dir).output().unwrap().status.success()
}

pub fn put(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, text).unwrap();
}

pub fn remove(root: &Path, path: &str) {
    std::fs::remove_file(root.join(path)).unwrap();
}
