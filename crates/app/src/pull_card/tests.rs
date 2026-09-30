use std::{sync::Arc, time::Duration};
use beui::merge::{Action, MergeMethod as UiMethod};
use gpui_kit::{Entity, TestAppContext};
use lathe_forge::{CheckStatus, ForgeError, MergeMethod, PullState};
use lathe_pr_view::fixture::{FixtureForge, Write, sample};
use super::*;
fn card(cx: &mut TestAppContext, running: bool) -> (Arc<FixtureForge>, Entity<PullCard>, &mut gpui_kit::VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        beui::init(cx);
        beui::theme::set_appearance(beui::theme::Appearance::Dark, cx);
    });
    let mut data = sample::data(7, "abc", Vec::new());
    if running {
        data.apply(lathe_pr_view::Part::Checks(vec![sample::check("test", CheckStatus::Running, None)]), sample::NOW);
    }
    let forge = Arc::new(FixtureForge::new().with_pull(data));
    let mut made = None;
    let (_root, cx) = cx.add_window_view(|_, cx| {
        let f: Arc<dyn lathe_forge::Forge> = forge.clone();
        let c = cx.new(|cx| PullCard::new(sample::reference(7), f, cx));
        made = Some(c.clone());
        Host(c)
    });
    cx.run_until_parked();
    (forge, made.unwrap(), cx)
}
struct Host(Entity<PullCard>);
impl gpui_kit::Render for Host {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement, Styled};
        gpui_kit::div().w(gpui_kit::px(600.)).child(self.0.clone())
    }
}
/// The card reads the pull request and its checks off the UI thread, and reads them again while
/// checks run.
#[gpui_kit::test]
fn the_card_reads_and_polls_while_checks_run(cx: &mut TestAppContext) {
    let (forge, card, cx) = card(cx, true);
    cx.update(|_, cx| {
        let c = card.read(cx);
        assert_eq!(c.pull.as_ref().map(|p| p.reference.number), Some(7));
        assert_eq!(c.checks.len(), 1);
        assert_eq!(c.next, Some(Duration::from_secs(10)));
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
        assert_eq!(c.pull.as_ref().map(|p| p.state), Some(PullState::Merged));
        assert_eq!(c.next, None, "a merged pull request is not read again");
    });
}
/// Delete branch asks first too.
#[gpui_kit::test]
fn delete_branch_asks_first(cx: &mut TestAppContext) {
    let (forge, card, cx) = card(cx, false);
    cx.update(|_, cx| card.update(cx, |c, cx| c.press(Action::DeleteBranch, cx)));
    assert!(forge.writes().is_empty());
    let words = cx.update(|_, cx| card.read(cx).confirm_words());
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
        assert_eq!(c.pull.as_ref().map(|p| p.state), Some(PullState::Open));
    });
}
