use std::sync::{Arc, Mutex};
use gpui_kit::TestAppContext;
use atelier_forge::{ForgeError, NewPull, PullRef, RepoRef};
use atelier_project::{LocalProject, Project};
use super::*;
use crate::fake_forge::{FakeForge, git, pushed_branch};
fn scratch() -> RepoRef {
    RepoRef { host: "github.com".into(), owner: "flazouh".into(), name: "atelier-qa-scratch".into() }
}
/// The window draws the form, so its Tab stops are in the tree.
struct Host(Entity<PullForm>);
impl gpui_kit::Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        use gpui_kit::{ParentElement, Styled};
        gpui_kit::div().w(gpui_kit::px(700.)).h(gpui_kit::px(600.)).child(self.0.clone())
    }
}
type Made<'a> = (std::path::PathBuf, Arc<FakeForge>, Entity<PullForm>, Arc<Mutex<Vec<PullRef>>>, &'a mut gpui_kit::VisualTestContext);
fn form(cx: &mut TestAppContext, dir: std::path::PathBuf) -> Made<'_> {
    form_with(cx, dir, |_| {})
}
fn form_with(cx: &mut TestAppContext, dir: std::path::PathBuf, set: impl FnOnce(&FakeForge)) -> Made<'_> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
    });
    let project: Arc<dyn Project> = Arc::new(LocalProject::open(&dir).unwrap());
    let forge = Arc::new(FakeForge::default());
    set(&forge);
    let backend = crate::fake_agent::fake_agent("fake").backend;
    let opened: Arc<Mutex<Vec<PullRef>>> = Arc::default();
    let heard = opened.clone();
    let mut made = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let f = cx.new(|cx| PullForm::new(project, backend, None, forge.clone(), window, cx));
        cx.subscribe(&f, move |_, _, event: &FormEvent, _| {
            if let FormEvent::Opened(reference) | FormEvent::Existing(reference) = event {
                heard.lock().unwrap().push(reference.clone());
            }
        })
        .detach();
        made = Some(f.clone());
        Host(f)
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
    git(&dir, &["config", "--remove-section", &format!("url.{}", crate::fake_forge::bare_of(&dir).display())]);
    let (_dir, forge, f, _opened, cx) = form(cx, dir);
    assert_eq!(cx.update(|_, cx| f.read(cx).stage.clone()), FormStage::Failed(crate::open_project::NO_FORGE_REMOTE.into()));
    assert!(forge.created.lock().unwrap().is_empty());
}
/// A branch that already has an open pull request gets that one, and the form offers no second create.
#[gpui_kit::test]
fn a_branch_with_a_pull_request_gets_that_one(cx: &mut TestAppContext) {
    let (_dir, forge, f, opened, cx) = form_with(cx, pushed_branch(), |forge| {
        *forge.open.lock().unwrap() = Some(atelier_forge::PullBrief {
            reference: PullRef { repo: scratch(), number: 3 },
            title: "Make a two".into(),
            state: atelier_forge::PullState::Open,
            url: "https://github.com/flazouh/atelier-qa-scratch/pull/3".into(),
        });
    });
    let reference = PullRef { repo: scratch(), number: 3 };
    assert_eq!(cx.update(|_, cx| f.read(cx).stage.clone()), FormStage::Existing(reference.clone(), "Make a two".into()));
    assert_eq!(*opened.lock().unwrap(), [reference], "the session gets it");
    cx.update(|window, cx| f.update(cx, |f, cx| f.submit(window, cx)));
    cx.run_until_parked();
    assert!(forge.created.lock().unwrap().is_empty(), "no second pull request");
}
/// Commits made after the push are pushed first: the button says so, and Open pushes, then opens.
#[gpui_kit::test]
fn commits_not_pushed_are_pushed_first(cx: &mut TestAppContext) {
    let dir = pushed_branch();
    std::fs::write(dir.join("a.txt"), "two, again\n").unwrap();
    git(&dir, &["commit", "-qam", "Again"]);
    let (dir, forge, f, _opened, cx) = form(cx, dir);
    cx.update(|_, cx| {
        let f = f.read(cx);
        assert_eq!(f.ahead, 1);
        assert_eq!(f.open_label(), "Push and open pull request");
    });
    cx.update(|window, cx| f.update(cx, |f, cx| f.submit(window, cx)));
    cx.run_until_parked();
    assert_eq!(forge.created.lock().unwrap().len(), 1);
    let bare = crate::fake_forge::bare_of(&dir);
    assert_eq!(git(&bare, &["rev-parse", "fix/two"]), git(&dir, &["rev-parse", "HEAD"]), "the new commit went first");
}
/// The bases are fetched when the form opens: a new branch on the remote shows, and a gone one goes.
#[gpui_kit::test]
fn the_bases_are_fetched_when_the_form_opens(cx: &mut TestAppContext) {
    let dir = pushed_branch();
    let bare = crate::fake_forge::bare_of(&dir);
    git(&bare, &["branch", "next", "main"]);
    git(&bare, &["branch", "-D", "release"]);
    let (_dir, _forge, f, _opened, cx) = form(cx, dir);
    cx.update(|_, cx| {
        let f = f.read(cx);
        assert_eq!(f.bases, ["main", "next"]);
        assert!(!f.checking, "the check landed");
    });
}
/// Tab goes from the title to the body, the base, the draft switch and Open, in that order.
#[gpui_kit::test]
fn tab_walks_the_form_in_order(cx: &mut TestAppContext) {
    let (_dir, _forge, f, _opened, cx) = form(cx, pushed_branch());
    let stops = cx.update(|_, cx| f.read(cx).tab_stops(cx));
    cx.update(|window, cx| stops[0].focus(window, cx));
    for (at, want) in stops.iter().enumerate().skip(1) {
        cx.update(|window, cx| window.focus_next(cx));
        cx.run_until_parked();
        let on = cx.update(|window, _| want.is_focused(window));
        assert!(on, "Tab stop {at} is next");
    }
}
/// Each way the forge can refuse the create keeps the form, with the forge's words: gh missing, signed
/// out, and asked to wait.
#[gpui_kit::test]
fn each_refusal_of_the_create_keeps_the_form(cx: &mut TestAppContext) {
    let (_dir, forge, f, opened, cx) = form(cx, pushed_branch());
    for error in [ForgeError::ToolMissing { tool: "gh".into() }, ForgeError::NotSignedIn, ForgeError::RateLimited { retry_after: Some(30) }, ForgeError::Offline] {
        *forge.fails.lock().unwrap() = Some(error.clone());
        cx.update(|window, cx| f.update(cx, |f, cx| f.submit(window, cx)));
        cx.run_until_parked();
        cx.update(|_, cx| {
            let f = f.read(cx);
            assert_eq!(f.stage, FormStage::Open, "{error}");
            assert_eq!(f.refused.as_ref().map(|w| w.to_string()), Some(error.to_string()));
        });
    }
    assert!(opened.lock().unwrap().is_empty());
}
/// When the look for the branch's pull request fails, the form says why and offers no create: it
/// cannot know that a second pull request would not be made.
#[gpui_kit::test]
fn a_failed_look_for_the_branchs_pull_request_offers_no_create(cx: &mut TestAppContext) {
    let (_dir, forge, f, _opened, cx) = form_with(cx, pushed_branch(), |forge| *forge.reads_fail.lock().unwrap() = Some(ForgeError::NotSignedIn));
    assert_eq!(cx.update(|_, cx| f.read(cx).stage.clone()), FormStage::Failed(ForgeError::NotSignedIn.to_string().into()));
    cx.update(|window, cx| f.update(cx, |f, cx| f.submit(window, cx)));
    cx.run_until_parked();
    assert!(forge.created.lock().unwrap().is_empty());
}
