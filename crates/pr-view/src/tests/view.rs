//! The pull request view in a window, on a real git repository and an in-memory forge.
use std::{path::Path, sync::Arc, time::Duration};

use gpui_kit::{Entity, TestAppContext, VisualTestContext};
use lathe_forge::{Change, ChangedFile, CheckStatus, Conclusion, Forge, PullRef};

use super::repo::{Scenario, put};
use crate::{
    fixture::{FixtureForge, sample},
    services::{PrConfig, Services},
    view::PullView,
};

pub struct Harness {
    pub scenario: Scenario,
    pub forge: Arc<FixtureForge>,
    pub services: Arc<Services>,
    pub reference: PullRef,
    pub head: String,
    pub base: String,
    _local: tempfile::TempDir,
}

/// A repository with `README.md` and `src/a.rs` on main, and pull request 7 changing `src/a.rs` and adding
/// `src/b.rs`, with one thread on `src/a.rs`.
pub fn harness() -> Harness {
    let scenario = Scenario::new(&[("README.md", "readme\n"), ("src/a.rs", "fn one() {}\nfn two() {}\nfn three() {}\n")]);
    let base = scenario.main_tip();
    let head = scenario.pull(7, "main", &[&|r: &Path| {
        put(r, "src/a.rs", "fn one() {}\nfn two() { 2 }\nfn three() {}\n");
        put(r, "src/b.rs", "pub fn b() {}\n");
    }]);
    let files = vec![
        ChangedFile { path: "src/a.rs".into(), additions: 1, deletions: 1, change: Change::Modified },
        ChangedFile { path: "src/b.rs".into(), additions: 1, deletions: 0, change: Change::Added },
    ];
    let mut data = sample::data(7, &head, files);
    data.pull.as_mut().unwrap().base_sha = base.clone();
    data.threads = vec![sample::thread("T1", "src/a.rs", 2, vec![sample::comment("c1", "Ada", "Why two?", sample::NOW - 600)])];
    data.checks = vec![sample::check("linux", CheckStatus::Done, Some(Conclusion::Success))];
    let reference = data.reference.clone();
    let forge = Arc::new(FixtureForge::new().with_pull(data));
    let local = tempfile::tempdir().unwrap();
    let config = PrConfig::new("alex", local.path())
        .remote_data(scenario.data.to_str().unwrap())
        .fetch_url(scenario.origin.to_str().unwrap())
        .refresh(Duration::from_millis(150), Duration::from_millis(150));
    let services = Services::open(scenario.project(), forge.clone() as Arc<dyn Forge>, config).unwrap();
    Harness { scenario, forge, services, reference, head, base, _local: local }
}

pub fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
        cx.set_reduce_motion(true);
    });
    cx.executor().allow_parking();
}

pub fn open<'a>(h: &Harness, cx: &'a mut TestAppContext) -> (Entity<PullView>, &'a mut VisualTestContext) {
    let (reference, services) = (h.reference.clone(), h.services.clone());
    cx.add_window_view(move |window, cx| PullView::new(reference, services, window, cx))
}

/// Lets the view's background work finish: git, the forge, the messages between.
pub fn settle(view: &Entity<PullView>, cx: &mut VisualTestContext, done: impl Fn(&PullView) -> bool) {
    for _ in 0..400 {
        cx.run_until_parked();
        if view.read_with(cx, |v, _| done(v)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the view did not settle");
}

#[gpui_kit::test]
fn opening_a_pull_request_reads_the_forge_and_git_and_shows_the_first_file(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    view.read_with(cx, |v, cx| {
        assert!(v.model.data.complete(), "every part arrived");
        assert_eq!(v.model.entries.as_ref().unwrap().len(), 2);
        assert_eq!(v.model.commits.len(), 1);
        let place = v.model.place.clone().unwrap();
        assert!(!place.brought_in);
        let shown = v.current_view().unwrap();
        assert_eq!(shown.path, place.path);
        // The editor holds the merged text of the file on screen.
        assert_eq!(v.editor.read(cx).value().as_ref(), shown.shown().unwrap().text().trim_end_matches('\n'));
        assert_eq!(v.model.base.as_ref().unwrap().sha, h.base);
    });
}
