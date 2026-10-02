use std::path::{Path, PathBuf};

use super::types::SCRATCH_URL;

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(["-c", "user.name=q", "-c", "user.email=q@q", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A clone of a bare remote with `main` and `release`, on `fix/two` with one commit, pushed. Then
/// origin's address is made a GitHub one, which names the repository, and an `insteadOf` rule sends
/// every fetch and push back to the bare remote, so GitHub is never reached.
pub fn pushed_branch() -> PathBuf {
    let top = crate::test_dirs::path();
    let (bare, work) = (top.join("remote.git"), top.join("work"));
    git(&top, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    git(&top, &["clone", "-q", bare.to_str().unwrap(), work.to_str().unwrap()]);
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-qm", "start"]);
    git(&work, &["push", "-q", "origin", "main", "main:release"]);
    git(&work, &["remote", "set-head", "origin", "main"]);
    git(&work, &["switch", "-qc", "fix/two"]);
    std::fs::write(work.join("a.txt"), "two\n").unwrap();
    git(&work, &["commit", "-qam", "Make a two"]);
    git(&work, &["push", "-q", "-u", "origin", "fix/two"]);
    git(&work, &["remote", "set-url", "origin", SCRATCH_URL]);
    git(&work, &["config", &format!("url.{}.insteadOf", bare.display()), SCRATCH_URL]);
    work
}

/// The bare remote behind `work`'s origin.
pub fn bare_of(work: &Path) -> PathBuf {
    work.parent().unwrap().join("remote.git")
}
