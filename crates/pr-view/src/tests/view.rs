//! The pull request view in a window, on a real git repository and an in-memory forge.
use std::{path::Path, sync::Arc, time::Duration};

use gpui_kit::{Entity, TestAppContext, VisualTestContext};
use beui::verdict::Verb;
use lathe_forge::{Change, ChangedFile, CheckStatus, Conclusion, Forge, PullRef};

use super::repo::{Scenario, put};
use crate::{
    fixture::{FixtureForge, Write, sample},
    services::{PrConfig, Services},
    view::PullView,
};

pub struct Harness {
    _scenario: Scenario,
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
    harness_with(|config| config)
}

impl Harness {
    /// A harness over a repository and a forge the caller made.
    pub fn over(repo: Scenario, forge: Arc<FixtureForge>, reference: PullRef, head: String) -> Harness {
        let base = repo.main_tip();
        let local = tempfile::tempdir().unwrap();
        let config = PrConfig::new("alex", local.path())
            .remote_data(repo.data.to_str().unwrap())
            .fetch_url(repo.origin.to_str().unwrap())
            .refresh(Duration::from_secs(60), Duration::from_secs(60));
        let services = Services::open(repo.project(), forge.clone() as Arc<dyn Forge>, config).unwrap();
        Harness { _scenario: repo, forge, services, reference, head, base, _local: local }
    }
}

pub fn harness_with(tune: impl FnOnce(PrConfig) -> PrConfig) -> Harness {
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
    let config = tune(config);
    let services = Services::open(scenario.project(), forge.clone() as Arc<dyn Forge>, config).unwrap();
    Harness { _scenario: scenario, forge, services, reference, head, base, _local: local }
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

fn write_seen(h: &Harness, cx: &mut VisualTestContext, view: &Entity<PullView>, count: usize) {
    settle(view, cx, |_| !h.forge.writes().is_empty() && h.forge.writes().len() >= count);
}

#[gpui_kit::test]
fn a_read_only_view_sends_nothing_and_says_so(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness_with(|c| c.read_only(true));
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    view.update_in(cx, |v, window, cx| {
        v.post_remark("hello".into(), window, cx);
        v.send_verdict(Verb::Approve, "ok".into(), window, cx);
        v.reply(lathe_forge::ThreadId("T1".into()), "x".into(), cx);
        v.resolve(lathe_forge::ThreadId("T1".into()), true, cx);
    });
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(50));
    cx.run_until_parked();
    assert!(h.forge.writes().is_empty(), "{:?}", h.forge.writes());
    view.read_with(cx, |v, _| assert!(v.notice.as_deref().is_some_and(|n| n.contains("Read-only"))));
}

#[gpui_kit::test]
fn a_remark_reaches_the_forge_and_comes_back_into_the_conversation(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    view.update_in(cx, |v, window, cx| v.post_remark("Looks fine to me".into(), window, cx));
    write_seen(&h, cx, &view, 1);
    assert_eq!(h.forge.writes(), vec![Write::Comment { reference: h.reference.clone(), body: "Looks fine to me".into() }]);
    settle(&view, cx, |v| v.model.data.remarks.iter().any(|r| r.body == "Looks fine to me"));
}

#[gpui_kit::test]
fn a_verdict_reply_and_resolve_each_send_one_write(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    view.update_in(cx, |v, window, cx| v.send_verdict(Verb::RequestChanges, "Please fix".into(), window, cx));
    write_seen(&h, cx, &view, 1);
    view.update(cx, |v, cx| v.reply(lathe_forge::ThreadId("T1".into()), "Because".into(), cx));
    write_seen(&h, cx, &view, 2);
    view.update(cx, |v, cx| v.resolve(lathe_forge::ThreadId("T1".into()), true, cx));
    write_seen(&h, cx, &view, 3);
    let writes = h.forge.writes();
    assert_eq!(writes[0], Write::Submit { reference: h.reference.clone(), verdict: lathe_forge::Verdict::RequestChanges, body: "Please fix".into() });
    assert_eq!(writes[1], Write::Reply { thread: lathe_forge::ThreadId("T1".into()), body: "Because".into() });
    assert_eq!(writes[2], Write::Resolve { thread: lathe_forge::ThreadId("T1".into()), resolved: true });
}

#[gpui_kit::test]
fn a_merge_names_the_head_the_reader_saw_and_a_moved_branch_is_refused(cx: &mut TestAppContext) {
    use beui::merge::{Action, Choice, MergeMethod};
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    let choice = Choice { method: MergeMethod::Squash, auto: false, delete_branch: true };
    // The branch moves under the reader before the press.
    h.forge.edit(&h.reference, |d| d.pull.as_mut().unwrap().head_sha = "f".repeat(40));
    view.update_in(cx, |v, window, cx| v.merge_pull(Action::Merge(MergeMethod::Squash), choice, "Title".into(), "Body".into(), window, cx));
    write_seen(&h, cx, &view, 1);
    settle(&view, cx, |v| v.notice.as_deref().is_some_and(|n| n.contains("moved")));
    assert!(matches!(&h.forge.writes()[0], Write::Merge { request, .. } if request.expected_head.as_deref() == Some(h.head.as_str()) && request.delete_branch));
    assert_ne!(h.forge.data(&h.reference).unwrap().pull.unwrap().state, lathe_forge::PullState::Merged);
}

#[gpui_kit::test]
fn a_merge_that_goes_through_shows_the_pull_request_as_merged(cx: &mut TestAppContext) {
    use beui::merge::{Action, Choice, MergeMethod};
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    let choice = Choice { method: MergeMethod::Merge, auto: false, delete_branch: false };
    view.update_in(cx, |v, window, cx| v.merge_pull(Action::Merge(MergeMethod::Merge), choice, String::new(), String::new(), window, cx));
    settle(&view, cx, |v| v.model.pull().is_some_and(|p| p.state == lathe_forge::PullState::Merged));
}

#[gpui_kit::test]
fn a_mark_survives_a_reopen_and_a_new_version_of_the_file_loses_it(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    let path = {
        let (view, cx) = open(&h, cx);
        settle(&view, cx, |v| v.current_view().is_some());
        let path = view.read_with(cx, |v, _| v.model.place.clone().unwrap().path);
        view.update(cx, |v, cx| v.toggle_seen(cx));
        for _ in 0..200 {
            cx.run_until_parked();
            if h.services.reviewed.marks(&h.reference).unwrap().contains_key(&path) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(h.services.reviewed.marks(&h.reference).unwrap().contains_key(&path), "the mark is on disk");
        path
    };
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    view.read_with(cx, |v, _| assert!(v.model.is_seen(&path), "seen again after the reopen"));
    // The file changes: the mark names another version.
    let marks = h.services.reviewed.marks(&h.reference).unwrap();
    assert!(!crate::state::is_seen(&marks, &path, "another-version"));
}

#[gpui_kit::test]
fn a_thread_that_arrives_while_reading_shows_up_and_the_reader_stays_on_their_file(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some() && v.first_load_done);
    view.update_in(cx, |v, window, cx| v.open_file("src/b.rs", window, cx));
    settle(&view, cx, |v| v.current_view().is_some_and(|f| f.path == "src/b.rs"));
    h.forge.edit(&h.reference, |d| {
        d.threads.push(sample::thread("T2", "src/b.rs", 1, vec![sample::comment("c9", "Bo", "New here", sample::NOW)]));
        d.pull.as_mut().unwrap().updated_at += 60;
    });
    cx.executor().advance_clock(Duration::from_millis(400));
    settle(&view, cx, |v| v.model.data.threads.len() == 2);
    view.read_with(cx, |v, _| {
        assert_eq!(v.model.place.clone().unwrap().path, "src/b.rs", "the reader keeps their place");
        assert!(v.notice.as_deref().is_some_and(|n| n.contains("new comment")), "{:?}", v.notice);
    });
}

#[gpui_kit::test]
fn not_being_signed_in_shows_why_and_a_retry_reads_the_pull_request(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    h.forge.fail("pull", lathe_forge::ForgeError::NotSignedIn);
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.model.error_of(crate::data::PartKind::Pull).is_some());
    view.read_with(cx, |v, _| assert!(!v.model.ready()));
    view.update(cx, |v, cx| v.retry(cx));
    settle(&view, cx, |v| v.model.ready() && v.current_view().is_some());
}

#[gpui_kit::test]
fn a_file_the_pull_request_did_not_change_is_brought_in_from_the_head(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    view.update_in(cx, |v, window, cx| v.bring_in("README.md", window, cx));
    let mut text = String::new();
    for _ in 0..400 {
        cx.run_until_parked();
        text = view.read_with(cx, |v, cx| v.editor.read(cx).value().to_string());
        if text == "readme" {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(text, "readme");
    view.read_with(cx, |v, _| assert!(v.model.place.as_ref().is_some_and(|p| p.brought_in)));
}

#[gpui_kit::test]
fn since_last_review_is_the_opening_choice_only_when_a_review_point_is_known(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    view.read_with(cx, |v, _| {
        // The sample has no review by the reader: the whole pull request.
        assert_eq!(v.model.base.as_ref().unwrap().choice, crate::base::BaseChoice::Whole);
    });
}

#[gpui_kit::test]
fn the_rail_lists_a_page_of_threads_and_shows_more_when_asked(cx: &mut TestAppContext) {
    setup(cx);
    let h = harness();
    h.forge.edit(&h.reference, |d| {
        d.threads = (0..45).map(|i| sample::thread(&format!("T{i}"), "src/a.rs", 2, vec![sample::comment(&format!("c{i}"), "Ada", "x", sample::NOW - 60)])).collect();
    });
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some() && v.model.data.threads.len() == 45);
    view.read_with(cx, |v, _| assert_eq!(v.page.0, crate::layout::PAGE));
    view.update(cx, |v, cx| v.show_more(false, cx));
    view.read_with(cx, |v, _| {
        assert_eq!(v.page.0, 2 * crate::layout::PAGE);
        assert_eq!(v.model.conversation_page(sample::NOW, v.page.0, v.page.1).hidden_threads, 45 - 2 * crate::layout::PAGE);
    });
}

#[gpui_kit::test]
fn the_other_merge_box_presses_each_reach_the_forge_with_their_own_call(cx: &mut TestAppContext) {
    use beui::merge::{Action, Choice, MergeMethod, UpdateWay};
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    let choice = Choice { method: MergeMethod::Merge, auto: false, delete_branch: false };
    let press = |action: Action, view: &Entity<PullView>, cx: &mut VisualTestContext| {
        view.update_in(cx, |v, window, cx| v.merge_pull(action, choice, String::new(), String::new(), window, cx));
    };
    press(Action::UpdateBranch(UpdateWay::Rebase), &view, cx);
    write_seen(&h, cx, &view, 1);
    press(Action::CancelMergeWhenReady, &view, cx);
    write_seen(&h, cx, &view, 2);
    press(Action::RemoveFromQueue, &view, cx);
    write_seen(&h, cx, &view, 3);
    press(Action::DeleteBranch, &view, cx);
    write_seen(&h, cx, &view, 4);
    press(Action::Revert, &view, cx);
    write_seen(&h, cx, &view, 5);
    press(Action::ReadyForReview, &view, cx);
    write_seen(&h, cx, &view, 6);
    let writes = h.forge.writes();
    assert_eq!(writes[0], Write::UpdateBranch { reference: h.reference.clone(), method: lathe_forge::UpdateMethod::Rebase, expected_head: h.head.clone() });
    assert_eq!(writes[1], Write::CancelAutoMerge(h.reference.clone()));
    assert_eq!(writes[2], Write::Dequeue(h.reference.clone()));
    assert_eq!(writes[3], Write::DeleteBranch(h.reference.clone()));
    assert_eq!(writes[4], Write::Revert(h.reference.clone()));
    assert!(matches!(&writes[5], Write::Update { update, .. } if update.ready == Some(true)));
    settle(&view, cx, |v| v.notice.is_some());
}

#[gpui_kit::test]
fn an_update_of_a_branch_that_moved_is_refused_and_says_so(cx: &mut TestAppContext) {
    use beui::merge::{Action, Choice, MergeMethod, UpdateWay};
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    h.forge.edit(&h.reference, |d| d.pull.as_mut().unwrap().head_sha = "e".repeat(40));
    let choice = Choice { method: MergeMethod::Merge, auto: false, delete_branch: false };
    view.update_in(cx, |v, window, cx| v.merge_pull(Action::UpdateBranch(UpdateWay::Merge), choice, String::new(), String::new(), window, cx));
    write_seen(&h, cx, &view, 1);
    settle(&view, cx, |v| v.notice.as_deref().is_some_and(|n| n.contains("modified")));
}

#[gpui_kit::test]
fn a_read_only_view_sends_none_of_the_merge_box_presses(cx: &mut TestAppContext) {
    use beui::merge::{Action, Choice, MergeMethod};
    setup(cx);
    let h = harness_with(|c| c.read_only(true));
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    let choice = Choice { method: MergeMethod::Merge, auto: false, delete_branch: false };
    for action in [Action::ReadyForReview, Action::DeleteBranch, Action::Revert, Action::RemoveFromQueue, Action::CancelMergeWhenReady] {
        view.update_in(cx, |v, window, cx| v.merge_pull(action, choice, String::new(), String::new(), window, cx));
    }
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(50));
    cx.run_until_parked();
    assert!(h.forge.writes().is_empty(), "{:?}", h.forge.writes());
}

#[gpui_kit::test]
fn the_comment_key_starts_a_draft_on_the_row_of_the_caret(cx: &mut TestAppContext) {
    use gpui_kit::component::input::Position;
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    let handler = view.update(cx, |v, cx| v.handlers(cx)).for_command(beui::keys::Command::Comment).cloned().expect("the view answers the comment key");
    assert!(view.read_with(cx, |v, _| v.draft.is_none()));
    // Row 1 of the shown file is a row that changed or sits beside a change; any row with a line will do.
    cx.update(|window, cx| {
        let editor = view.read(cx).editor.clone();
        editor.update(cx, |e, cx| e.set_cursor_position(Position { line: 1, character: 0 }, window, cx));
        handler(window, cx);
    });
    cx.run_until_parked();
    let row = view.read_with(cx, |v, _| v.draft.as_ref().map(|d| d.row));
    assert_eq!(row, Some(1), "a draft is open on the caret's row");
}

/// Under 700 px one part shows at a time, with a switch over it; from 700 up both show.
#[gpui_kit::test]
fn a_narrow_pane_shows_details_or_files_and_the_switch_and_the_keys_change_it(cx: &mut TestAppContext) {
    use crate::layout::Part;
    use gpui_kit::{Modifiers, px, size};
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    for width in [450., 550.] {
        cx.simulate_resize(size(px(width), px(800.)));
        for _ in 0..4 {
            cx.run_until_parked();
            view.update(cx, |_, cx| cx.notify());
        }
        assert!(cx.debug_bounds("pr-part-details").is_some(), "{width}: the switch shows");
        assert!(cx.debug_bounds("pr-rail").is_some_and(|b| f32::from(b.size.width) <= width), "{width}: the rail fits the pane");
        assert!(cx.debug_bounds("pr-diff").is_none(), "{width}: the diff is the other part");
    }
    // The switch chooses Files.
    let files = cx.debug_bounds("pr-part-files").unwrap().center();
    cx.simulate_click(files, Modifiers::default());
    for _ in 0..4 {
        cx.run_until_parked();
        view.update(cx, |_, cx| cx.notify());
    }
    assert_eq!(view.read_with(cx, |v, _| v.part), Part::Files);
    assert!(cx.debug_bounds("pr-rail").is_none() && cx.debug_bounds("pr-diff").is_some());
    // The Details key comes back to the rail.
    let handler = view.update(cx, |v, cx| v.handlers(cx)).for_command(beui::keys::Command::ToggleDetails).cloned().expect("the view answers the details key");
    cx.update(|window, cx| handler(window, cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.part), Part::Details);
    // From 700 up both parts show.
    cx.simulate_resize(size(px(1000.), px(800.)));
    for _ in 0..4 {
        cx.run_until_parked();
        view.update(cx, |_, cx| cx.notify());
    }
    assert!(cx.debug_bounds("pr-part-details").is_none(), "no switch when both show");
    assert!(cx.debug_bounds("pr-rail").is_some() && cx.debug_bounds("pr-diff").is_some());
}

/// The rail scrolls; a fade at its foot says there is more, and goes at the end.
#[gpui_kit::test]
fn the_rail_fades_at_its_foot_while_there_is_more_below_and_not_at_the_end(cx: &mut TestAppContext) {
    use gpui_kit::{point, px, size};
    setup(cx);
    let h = harness();
    let (view, cx) = open(&h, cx);
    settle(&view, cx, |v| v.current_view().is_some());
    cx.simulate_resize(size(px(450.), px(360.)));
    for _ in 0..5 {
        cx.run_until_parked();
        view.update(cx, |_, cx| cx.notify());
    }
    assert!(cx.debug_bounds("pr-rail-fade").is_some(), "the rail is taller than the pane: a fade shows");
    let max = view.read_with(cx, |v, _| v.rail_scroll.max_offset().y);
    view.update(cx, |v, cx| {
        v.rail_scroll.set_offset(point(px(0.), -max));
        cx.notify();
    });
    for _ in 0..3 {
        cx.run_until_parked();
        view.update(cx, |_, cx| cx.notify());
    }
    assert!(cx.debug_bounds("pr-rail-fade").is_none(), "at the end there is no fade");
}
