use std::{sync::Arc, time::Duration};
use beui::merge::{Action, MergeMethod as UiMethod};
use gpui_kit::{Entity, TestAppContext};
use atelier_forge::{CheckStatus, ForgeError, MergeMethod, PullState};
use atelier_pr_view::fixture::{FixtureForge, Write, sample};
use super::*;
fn card(cx: &mut TestAppContext, running: bool) -> (Arc<FixtureForge>, Entity<PullCard>, &mut gpui_kit::VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
    });
    let mut data = sample::data(7, "abc", Vec::new());
    if running {
        data.apply(atelier_pr_view::Part::Checks(vec![sample::check("test", CheckStatus::Running, None)]), sample::NOW);
    }
    let forge = Arc::new(FixtureForge::new().with_pull(data));
    let mut made = None;
    let (_root, cx) = cx.add_window_view(|_, cx| {
        let f: Arc<dyn atelier_forge::Forge> = forge.clone();
        let c = cx.new(|cx| PullCard::new(sample::reference(7), f, cx));
        made = Some(c.clone());
        Host(vec![c], true)
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (forge, made.unwrap(), cx)
}
/// The window, with its cards drawn or not, as a session panel on screen or off it.
struct Host(Vec<Entity<PullCard>>, bool);
impl gpui_kit::Render for Host {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement, Styled};
        gpui_kit::div().w(gpui_kit::px(600.)).children(self.1.then(|| self.0.clone()).into_iter().flatten())
    }
}
/// The card reads the pull request and its checks off the UI thread, and reads them again while
/// checks run.
#[gpui_kit::test]
fn the_card_reads_and_polls_while_checks_run(cx: &mut TestAppContext) {
    let (forge, card, cx) = card(cx, true);
    cx.update(|_, cx| {
        let c = card.read(cx);
        assert_eq!(c.pull(cx).map(|p| p.reference.number), Some(7));
        assert_eq!(c.checks(cx).len(), 1);
        assert_eq!(c.next(cx), Some(Duration::from_secs(10)));
    });
    let before = forge.count("pull");
    cx.executor().advance_clock(Duration::from_secs(11));
    cx.run_until_parked();
    assert_eq!(forge.count("pull"), before + 1, "read again after 10 s");
}
/// A merge asks first; Cancel sends nothing, and Merge sends the squash with the pull request's title,
/// after which the card shows it merged and stops reading.
#[gpui_kit::test]
fn a_merge_asks_first_then_squashes(cx: &mut TestAppContext) {
    let (forge, card, cx) = card(cx, false);
    cx.update(|_, cx| card.update(cx, |c, cx| c.press(Action::Merge(UiMethod::Squash), cx)));
    assert!(cx.update(|_, cx| card.read(cx).confirm.is_some()), "it asks");
    cx.update(|_, cx| card.update(cx, |c, cx| c.cancel_confirm(cx)));
    assert!(forge.writes().is_empty(), "Cancel sends nothing");
    cx.update(|_, cx| card.update(cx, |c, cx| c.press(Action::Merge(UiMethod::Squash), cx)));
    cx.update(|_, cx| card.update(cx, |c, cx| c.confirm(cx)));
    cx.run_until_parked();
    let merged = forge.writes().into_iter().find_map(|w| match w {
        Write::Merge { request, .. } => Some(request),
        _ => None,
    });
    let request = merged.expect("the merge went");
    assert_eq!(request.method, MergeMethod::Squash);
    assert_eq!(request.expected_head.as_deref(), Some("abc"), "only the head the reader saw");
    assert!(request.title.as_deref().is_some_and(|t| t.ends_with("(#7)")), "{request:?}");
    cx.update(|_, cx| {
        let c = card.read(cx);
        assert_eq!(c.pull(cx).map(|p| p.state), Some(PullState::Merged));
        assert_eq!(c.next(cx), None, "a merged pull request is not read again");
    });
}
/// Delete branch asks first too.
#[gpui_kit::test]
fn delete_branch_asks_first(cx: &mut TestAppContext) {
    let (forge, card, cx) = card(cx, false);
    cx.update(|_, cx| card.update(cx, |c, cx| c.press(Action::DeleteBranch, cx)));
    assert!(forge.writes().is_empty());
    let words = cx.update(|_, cx| card.read(cx).confirm_words(cx));
    assert!(words.as_deref().is_some_and(|w| w.contains("Delete")), "{words:?}");
    cx.update(|_, cx| card.update(cx, |c, cx| c.confirm(cx)));
    cx.run_until_parked();
    assert!(forge.writes().iter().any(|w| matches!(w, Write::DeleteBranch(_))));
}
/// The forge's refusal shows as it says it, and the card keeps reading.
#[gpui_kit::test]
fn a_refusal_shows_the_forges_words(cx: &mut TestAppContext) {
    let (forge, card, cx) = card(cx, false);
    forge.fail("merge", ForgeError::Rejected("Required checks are failing".into()));
    cx.update(|_, cx| card.update(cx, |c, cx| c.press(Action::Merge(UiMethod::Squash), cx)));
    cx.update(|_, cx| card.update(cx, |c, cx| c.confirm(cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let c = card.read(cx);
        assert_eq!(c.said.as_deref(), Some("Required checks are failing"));
        assert_eq!(c.pull(cx).map(|p| p.state), Some(PullState::Open));
    });
}
/// Two cards on one pull request share one read.
#[gpui_kit::test]
fn two_cards_on_one_pull_request_read_it_once(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
    });
    let mut data = sample::data(7, "abc", Vec::new());
    data.apply(atelier_pr_view::Part::Checks(vec![sample::check("test", CheckStatus::Running, None)]), sample::NOW);
    let forge = Arc::new(FixtureForge::new().with_pull(data));
    let (_root, cx) = cx.add_window_view(|_, cx| {
        let f: Arc<dyn atelier_forge::Forge> = forge.clone();
        let cards = (0..2).map(|_| cx.new(|cx| PullCard::new(sample::reference(7), f.clone(), cx))).collect();
        Host(cards, true)
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert_eq!(forge.count("pull"), 1, "one read for both");
    cx.executor().advance_clock(Duration::from_secs(11));
    cx.run_until_parked();
    assert_eq!(forge.count("pull"), 2, "and one each 10 s");
}
/// A card that is not on screen reads no more; drawn again, it reads at once.
#[gpui_kit::test]
fn a_card_off_screen_pauses_and_reads_when_it_shows(cx: &mut TestAppContext) {
    let (forge, _card, cx) = card(cx, true);
    let host = cx.update(|window, _| window.root::<Host>().flatten()).expect("the host");
    cx.update(|_, cx| host.update(cx, |h, cx| {
        h.1 = false;
        cx.notify();
    }));
    cx.run_until_parked();
    let before = forge.count("pull");
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_secs(11));
        cx.run_until_parked();
    }
    assert!(forge.count("pull") <= before + 1, "at most the read already under way: {} after {before}", forge.count("pull"));
    let paused = forge.count("pull");
    cx.update(|_, cx| host.update(cx, |h, cx| {
        h.1 = true;
        cx.notify();
    }));
    cx.run_until_parked();
    assert_eq!(forge.count("pull"), paused + 1, "read at once when it shows");
}
/// A read that fails offline says so on the card and backs off, and the next read that works clears it.
#[gpui_kit::test]
fn an_offline_read_says_so_and_backs_off(cx: &mut TestAppContext) {
    let (forge, card, cx) = card(cx, true);
    forge.fail("pull", ForgeError::Offline);
    cx.executor().advance_clock(Duration::from_secs(11));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let c = card.read(cx);
        assert_eq!(c.unread(cx).map(|w| w.to_string()), Some(ForgeError::Offline.to_string()));
        assert_eq!(c.next(cx), Some(Duration::from_secs(10)), "the first wait after a failure");
        assert!(c.pull(cx).is_some(), "the last pull request read stays on the card");
    });
    cx.executor().advance_clock(Duration::from_secs(11));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| card.read(cx).unread(cx)), None, "a read that works clears it");
}
