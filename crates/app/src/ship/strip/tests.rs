use std::process::Command as Git;

use gpui_kit::TestAppContext;

use super::*;
use crate::{
    fake_agent::{ended, git_project, start_in},
    review_pane::{ReviewPane, Scope},
    ship::kept::kept,
};

/// The whole step: a turn changes a file, the reader accepts one hunk of two, the strip reads what the
/// commit takes, the agent drafts a branch and a message, and Commit lands the accepted hunk alone on
/// the new branch.
#[gpui_kit::test]
fn the_strip_commits_the_accepted_hunk_on_a_drafted_branch(cx: &mut TestAppContext) {
    let dir = git_project(&[("a.txt", "1\n2\n3\n4\n5\n6\n7\n8\n")]);
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("a.txt"), "1\nTWO\n3\n4\n5\n6\nSEVEN\n8\n").unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let pane = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project.clone(), Scope::Turn(0), Some("a.txt"), window, cx)));
    cx.run_until_parked();
    let first = cx.update(|_, cx| pane.read(cx).files[0].merged.as_ref().unwrap().hunks()[0].id.to_string());
    cx.update(|window, cx| {
        pane.update(cx, |p, cx| {
            p.decide_hunk(&first, beui::Decision::Accept, window, cx);
        })
    });
    cx.run_until_parked();
    let kept_now: Vec<Kept> = cx.update(|_, cx| pane.read(cx).files.iter().filter_map(|f| kept(&f.review, f.merged.as_ref())).collect());
    let backend = crate::fake_agent::fake_agent("fake").backend;
    let strip = cx.update(|window, cx| cx.new(|cx| ShipStrip::new(project.clone(), backend, None, window, cx)));
    cx.update(|window, cx| strip.update(cx, |s, cx| s.open(kept_now, window, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let s = strip.read(cx);
        assert_eq!(s.stage, Stage::Open);
        assert_eq!(s.lines, [Line { path: "a.txt".into(), added: 1, removed: 1 }]);
        assert!(s.on_default, "a fresh repository sits on its default branch");
        assert_eq!(s.message.read(cx).value().as_ref(), "Keep TWO\n\nFrom the review.");
        assert_eq!(s.new_branch.read(cx).value().as_ref(), "fix/keep-two");
    });
    cx.update(|window, cx| strip.update(cx, |s, cx| s.commit(window, cx)));
    cx.run_until_parked();
    let git = |args: &[&str]| String::from_utf8(Git::new("git").args(args).current_dir(&dir).output().unwrap().stdout).unwrap();
    assert!(matches!(cx.update(|_, cx| strip.read(cx).stage.clone()), Stage::Committed(_)));
    assert_eq!(git(&["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix/keep-two");
    assert_eq!(git(&["log", "-1", "--format=%s"]).trim(), "Keep TWO");
    assert_eq!(git(&["show", "HEAD:a.txt"]), "1\nTWO\n3\n4\n5\n6\n7\n8\n", "the accepted hunk alone");
    cx.update(|_, cx| {
        let s = strip.read(cx);
        assert!(s.message.read(cx).value().is_empty(), "the next commit drafts its own message");
        assert!(s.new_branch.read(cx).value().is_empty());
    });
}
/// In a repository with no commit yet, where the turn makes the first file, the strip counts the kept
/// text against nothing and makes the first commit, on the drafted branch.
#[gpui_kit::test]
fn the_strip_makes_the_first_commit_of_a_new_repository(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap().keep();
    assert!(Git::new("git").args(["init", "-q", "-b", "main"]).current_dir(&dir).status().unwrap().success());
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("a.txt"), "1\nTWO\n").unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let pane = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project.clone(), Scope::Turn(0), None, window, cx)));
    cx.run_until_parked();
    let first = cx.update(|_, cx| pane.read(cx).files[0].merged.as_ref().unwrap().hunks()[0].id.to_string());
    cx.update(|window, cx| pane.update(cx, |p, cx| p.decide_hunk(&first, beui::Decision::Accept, window, cx)));
    cx.run_until_parked();
    let kept_now: Vec<Kept> = cx.update(|_, cx| pane.read(cx).files.iter().filter_map(|f| kept(&f.review, f.merged.as_ref())).collect());
    let backend = crate::fake_agent::fake_agent("fake").backend;
    let strip = cx.update(|window, cx| cx.new(|cx| ShipStrip::new(project.clone(), backend, None, window, cx)));
    cx.update(|window, cx| strip.update(cx, |s, cx| s.open(kept_now, window, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let s = strip.read(cx);
        assert_eq!(s.lines, [Line { path: "a.txt".into(), added: 2, removed: 0 }], "nothing is in HEAD yet");
        assert!(s.on_default, "main is the default");
    });
    cx.update(|window, cx| strip.update(cx, |s, cx| s.commit(window, cx)));
    cx.run_until_parked();
    let git = |args: &[&str]| String::from_utf8(Git::new("git").args(args).current_dir(&dir).output().unwrap().stdout).unwrap();
    let stage = cx.update(|_, cx| strip.read(cx).stage.clone());
    assert!(matches!(&stage, Stage::Committed(w) if w.starts_with("Committed 1 file as ")), "{stage:?}");
    assert_eq!(git(&["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix/keep-two");
    assert_eq!(git(&["rev-list", "--count", "HEAD"]).trim(), "1", "the first commit");
    assert_eq!(git(&["show", "HEAD:a.txt"]), "1\nTWO\n");
}
/// A commit made from the pane's own strip tells the project to read git again, and shows each
/// committed file as committed, with nothing left to decide or undo, here and when the review opens
/// again.
#[gpui_kit::test]
fn a_commit_from_the_pane_marks_its_files_committed(cx: &mut TestAppContext) {
    let dir = git_project(&[("a.txt", "1\n2\n")]);
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("a.txt"), "1\nTWO\n").unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let pane = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project.clone(), Scope::Turn(0), None, window, cx)));
    let told = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let heard = told.clone();
    let _sub = cx.update(|_, cx| {
        cx.subscribe(&pane, move |_, event: &crate::review_pane::PaneEvent, _| {
            if matches!(event, crate::review_pane::PaneEvent::GitChanged) {
                heard.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        })
    });
    cx.run_until_parked();
    let first = cx.update(|_, cx| pane.read(cx).files[0].merged.as_ref().unwrap().hunks()[0].id.to_string());
    cx.update(|window, cx| pane.update(cx, |p, cx| p.decide_hunk(&first, beui::Decision::Accept, window, cx)));
    cx.update(|window, cx| pane.update(cx, |p, cx| p.open_ship(window, cx)));
    cx.run_until_parked();
    let strip = cx.update(|_, cx| pane.read(cx).ship.clone());
    cx.update(|window, cx| strip.update(cx, |s, cx| s.commit(window, cx)));
    cx.run_until_parked();
    let git = |args: &[&str]| String::from_utf8(Git::new("git").args(args).current_dir(&dir).output().unwrap().stdout).unwrap();
    let short = git(&["rev-parse", "--short=7", "HEAD"]).trim().to_string();
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 1, "the project hears of the commit");
    cx.update(|_, cx| {
        let p = pane.read(cx);
        assert_eq!(p.files[0].committed.as_deref(), Some(short.as_str()));
        assert_eq!(p.committed_words(0).as_deref(), Some(format!("Committed in {short}").as_str()));
        assert!(!p.can_undo(0), "an undo would change the file under the commit");
    });
    let again = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project.clone(), Scope::Turn(0), None, window, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(again.read(cx).files[0].committed.as_deref(), Some(short.as_str()), "kept with the session"));
}
/// The strip on a turn that changed a.txt from "1\n2\n" to "1\nTWO\n", with the change accepted, and
/// its pane, in a repository `prepare` set up first.
type Opened<'a> = (std::path::PathBuf, Entity<ReviewPane>, Entity<ShipStrip>, &'a mut gpui_kit::VisualTestContext);
fn opened_strip(cx: &mut TestAppContext, prepare: impl FnOnce(&std::path::Path)) -> Opened<'_> {
    let dir = git_project(&[("a.txt", "1\n2\n")]);
    prepare(&dir);
    let (session, fake, cx) = start_in(cx, dir.clone(), vec![vec![ended()]], false);
    let root = dir.clone();
    fake.work.lock().unwrap().push(Box::new(move || std::fs::write(root.join("a.txt"), "1\nTWO\n").unwrap()));
    cx.update(|_, cx| session.update(cx, |s, cx| s.send("edit".into(), cx)));
    cx.run_until_parked();
    let project: Arc<dyn Project> = Arc::new(lathe_project::LocalProject::open(&dir).unwrap());
    let pane = cx.update(|window, cx| cx.new(|cx| ReviewPane::new(session.clone(), project.clone(), Scope::Turn(0), None, window, cx)));
    cx.run_until_parked();
    let first = cx.update(|_, cx| pane.read(cx).files[0].merged.as_ref().unwrap().hunks()[0].id.to_string());
    cx.update(|window, cx| pane.update(cx, |p, cx| p.decide_hunk(&first, beui::Decision::Accept, window, cx)));
    cx.update(|window, cx| pane.update(cx, |p, cx| p.open_ship(window, cx)));
    cx.run_until_parked();
    let strip = cx.update(|_, cx| pane.read(cx).ship.clone());
    (dir, pane, strip, cx)
}
/// A drafted branch name that is taken becomes the first free one.
#[gpui_kit::test]
fn a_drafted_branch_name_that_is_taken_gets_a_number(cx: &mut TestAppContext) {
    let (_dir, _pane, strip, cx) = opened_strip(cx, |dir| {
        assert!(Git::new("git").args(["branch", "fix/keep-two"]).current_dir(dir).status().unwrap().success());
    });
    cx.update(|_, cx| assert_eq!(strip.read(cx).new_branch.read(cx).value().as_ref(), "fix/keep-two-2"));
}
/// A hook that refuses the commit keeps the card open with its words, on the branch made for it, so
/// Commit works again once the hook agrees.
#[gpui_kit::test]
fn a_refused_commit_keeps_the_card_open_on_the_new_branch(cx: &mut TestAppContext) {
    let (dir, _pane, strip, cx) = opened_strip(cx, |dir| {
        let hook = dir.join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\necho 'lint: no trailing spaces' >&2\nexit 1\n").unwrap();
        assert!(Git::new("chmod").args(["+x", hook.to_str().unwrap()]).status().unwrap().success());
    });
    let moved = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let heard = moved.clone();
    let _sub = cx.update(|_, cx| {
        cx.subscribe(&strip, move |_, event: &StripEvent, _| {
            if matches!(event, StripEvent::BranchMade) {
                heard.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        })
    });
    cx.update(|window, cx| strip.update(cx, |s, cx| s.commit(window, cx)));
    cx.run_until_parked();
    assert_eq!(moved.load(std::sync::atomic::Ordering::SeqCst), 1, "the owner hears of the new branch");
    let git = |args: &[&str]| String::from_utf8(Git::new("git").args(args).current_dir(&dir).output().unwrap().stdout).unwrap();
    cx.update(|_, cx| {
        let s = strip.read(cx);
        assert_eq!(s.stage, Stage::Open, "the card stays open");
        let refused = s.refused.clone().unwrap_or_default();
        assert!(refused.contains("lint: no trailing spaces"), "{refused}");
        assert!(refused.contains("fix/keep-two"), "it says the branch was made: {refused}");
        assert!(!s.on_default, "the next try commits on the new branch");
        assert_eq!(s.message.read(cx).value().as_ref(), "Keep TWO\n\nFrom the review.", "the message stays");
    });
    assert_eq!(git(&["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "fix/keep-two");
    std::fs::remove_file(dir.join(".git/hooks/pre-commit")).unwrap();
    cx.update(|window, cx| strip.update(cx, |s, cx| s.commit(window, cx)));
    cx.run_until_parked();
    assert!(matches!(cx.update(|_, cx| strip.read(cx).stage.clone()), Stage::Committed(_)));
    assert_eq!(git(&["log", "-1", "--format=%s"]).trim(), "Keep TWO");
}
