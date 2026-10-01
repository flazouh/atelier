use std::process::Command as Git;

use super::*;

#[test]
fn texts_in_head_come_in_one_call_and_a_missing_one_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| assert!(Git::new("git").args(["-c", "user.name=q", "-c", "user.email=q@x", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir.path()).status().unwrap().success());
    git(&["init", "-q"]);
    std::fs::write(dir.path().join("a.txt"), "1\n2\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "é\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "s"]);
    let project = atelier_project::LocalProject::open(dir.path()).unwrap();
    assert_eq!(texts(&project, &["a.txt", "new.txt", "b.txt"]), [Some("1\n2\n".into()), None, Some("é\n".into())]);
}
