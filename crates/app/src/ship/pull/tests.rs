use std::{path::Path, process::Command as Git, sync::Arc};
use lathe_project::{LocalProject, Project};
use super::*;
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Git::new("git").args(["-c", "user.name=q", "-c", "user.email=q@q", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}
/// A clone of a bare remote with main, release and a feature branch, on the feature branch with one
/// commit of its own.
fn clone() -> (tempfile::TempDir, std::path::PathBuf, Arc<dyn Project>) {
    let top = tempfile::tempdir().unwrap();
    let (bare, work) = (top.path().join("remote.git"), top.path().join("work"));
    git(top.path(), &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
    git(top.path(), &["clone", "-q", bare.to_str().unwrap(), work.to_str().unwrap()]);
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-qm", "start"]);
    git(&work, &["push", "-q", "origin", "main", "main:release"]);
    git(&work, &["remote", "set-head", "origin", "main"]);
    git(&work, &["switch", "-qc", "fix/two"]);
    std::fs::write(work.join("a.txt"), "two\n").unwrap();
    git(&work, &["commit", "-qam", "Make a two"]);
    git(&work, &["push", "-q", "-u", "origin", "fix/two"]);
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(&work).unwrap());
    (top, work, project)
}
/// The bases to pick from are origin's branches, the default first, without the branch itself.
#[test]
fn the_bases_are_origins_branches_default_first() {
    let (_top, _work, project) = clone();
    assert_eq!(bases(project.as_ref(), "fix/two"), ["main", "release"]);
}
/// The prompt holds the commits the pull request brings and the files they change.
#[test]
fn the_prompt_holds_the_branchs_commits() {
    let (_top, _work, project) = clone();
    let prompt = prompt(project.as_ref(), "main");
    assert!(prompt.contains("Make a two"), "{prompt}");
    assert!(prompt.contains("a.txt"), "{prompt}");
    assert!(prompt.contains("title"), "it asks for a title and a body: {prompt}");
}
/// A draft's first line is the title and the rest the body, without fences, quotes or trailers.
#[test]
fn a_draft_splits_into_a_title_and_a_body() {
    assert_eq!(title_and_body("```\nKeep TWO\n\nFrom the review.\n```"), ("Keep TWO".into(), "From the review.".into()));
    assert_eq!(title_and_body("Title: Keep TWO"), ("Keep TWO".into(), String::new()));
    assert_eq!(title_and_body("# Keep TWO\n\nBody\n\nCo-Authored-By: x <x@x>"), ("Keep TWO".into(), "Body".into()));
}
