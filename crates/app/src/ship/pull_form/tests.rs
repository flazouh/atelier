use std::sync::{Arc, Mutex};
use gpui_kit::TestAppContext;
use lathe_forge::{ForgeError, NewPull, PullRef, RepoRef};
use lathe_project::{LocalProject, Project};
use super::*;
use crate::fake_forge::{FakeForge, git, pushed_branch};
fn scratch() -> RepoRef {
    RepoRef { host: "github.com".into(), owner: "flazouh".into(), name: "lathe-qa-scratch".into() }
}
type Made<'a> = (std::path::PathBuf, Arc<FakeForge>, Entity<PullForm>, Arc<Mutex<Vec<PullRef>>>, &'a mut gpui_kit::VisualTestContext);
fn form(cx: &mut TestAppContext, dir: std::path::PathBuf) -> Made<'_> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
    });
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(&dir).unwrap());
    let forge = Arc::new(FakeForge::default());
    let backend = crate::fake_agent::fake_agent("fake").backend;
    let opened: Arc<Mutex<Vec<PullRef>>> = Arc::default();
    let heard = opened.clone();
    let mut made = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let f = cx.new(|cx| PullForm::new(project, backend, None, forge.clone(), window, cx));
        cx.subscribe(&f, move |_, _, event: &FormEvent, _| {
            if let FormEvent::Opened(reference) = event {
                heard.lock().unwrap().push(reference.clone());
            }
        })
        .detach();
        made = Some(f.clone());
        gpui_kit::Empty
    });
    let f = made.unwrap();
    cx.update(|window, cx| f.update(cx, |f, cx| f.open(window, cx)));
    cx.run_until_parked();
    (dir, forge, f, opened, cx)
}
/// The form reads the branch and the bases, the agent drafts the title and the body, and Open asks the
/// forge for the pull request the reader set: here a draft into main.
#[gpui_kit::test]
fn the_form_drafts_and_opens_the_pull_request(cx: &mut TestAppContext) {
    let (_dir, forge, f, opened, cx) = form(cx, pushed_branch());
    cx.update(|_, cx| {
        let f = f.read(cx);
        assert_eq!(f.stage, FormStage::Open);
        assert_eq!(f.head, "fix/two");
        assert_eq!(f.bases, ["main", "release"]);
        assert_eq!(f.base, 0);
        assert!(!f.draft);
        assert_eq!(f.title.read(cx).value().as_ref(), "Keep TWO");
        assert_eq!(f.body.read(cx).value().as_ref(), "From the review.");
    });
    cx.update(|_, cx| f.update(cx, |f, cx| f.set_draft(true, cx)));
    cx.update(|window, cx| f.update(cx, |f, cx| f.submit(window, cx)));
    cx.run_until_parked();
    let want = NewPull { title: "Keep TWO".into(), body: "From the review.".into(), base: "main".into(), head: "fix/two".into(), draft: true };
    assert_eq!(*forge.created.lock().unwrap(), [(scratch(), want)]);
    let reference = PullRef { repo: scratch(), number: 7 };
    assert_eq!(cx.update(|_, cx| f.read(cx).stage.clone()), FormStage::Opened(reference.clone()));
    assert_eq!(*opened.lock().unwrap(), [reference]);
}
/// A forge that refuses keeps the form open with its words, and the reader's text stays.
#[gpui_kit::test]
fn a_refusal_keeps_the_form_and_says_why(cx: &mut TestAppContext) {
    let (_dir, forge, f, opened, cx) = form(cx, pushed_branch());
    *forge.fails.lock().unwrap() = Some(ForgeError::NotSignedIn);
    cx.update(|window, cx| f.update(cx, |f, cx| f.submit(window, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let f = f.read(cx);
        assert_eq!(f.stage, FormStage::Open);
        assert_eq!(f.refused.as_ref().map(|w| w.to_string()), Some(ForgeError::NotSignedIn.to_string()));
        assert_eq!(f.title.read(cx).value().as_ref(), "Keep TWO");
    });
    assert!(opened.lock().unwrap().is_empty());
}
/// A project whose origin is not on GitHub says so, and asks the forge nothing.
#[gpui_kit::test]
fn no_github_remote_says_so(cx: &mut TestAppContext) {
    let dir = pushed_branch();
    git(&dir, &["remote", "set-url", "origin", "/somewhere/else.git"]);
    let (_dir, forge, f, _opened, cx) = form(cx, dir);
    assert_eq!(cx.update(|_, cx| f.read(cx).stage.clone()), FormStage::Failed(crate::open_project::NO_FORGE_REMOTE.into()));
    assert!(forge.created.lock().unwrap().is_empty());
}
