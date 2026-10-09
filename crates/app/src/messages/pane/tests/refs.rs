//! A reference a tool card names opens in the pane.
use atelier_capabilities::Ref;
use gpui_kit::TestAppContext;

use super::{Fake, one, settle};

fn channel_of(
    pane: &gpui_kit::Entity<super::MessagesPane>,
    cx: &mut gpui_kit::VisualTestContext,
) -> Option<Ref> {
    pane.read_with(cx, |p, _| p.open_channel_ref().map(|(_, r)| r.clone()))
}

#[gpui_kit::test]
fn a_channel_reference_opens_the_channel(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &["first"]);
    let (pane, cx) = one(&provider, cx);
    pane.update(cx, |p, cx| p.open_ref(&channel, cx));
    settle(&pane, cx);
    assert_eq!(channel_of(&pane, cx), Some(channel));
}

#[gpui_kit::test]
fn a_message_reference_opens_its_channel(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &["first"]);
    let (pane, cx) = one(&provider, cx);
    let message: Ref = format!("{channel}:1760000000.000001")
        .parse()
        .expect("a message reference");
    pane.update(cx, |p, cx| p.open_ref(&message, cx));
    settle(&pane, cx);
    assert_eq!(channel_of(&pane, cx), Some(channel));
}

#[gpui_kit::test]
fn a_reference_of_an_account_that_is_not_here_changes_nothing(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &["first"]);
    let (pane, cx) = one(&provider, cx);
    let before = channel_of(&pane, cx);
    for text in ["messaging:memory:other:C1", "tasks:memory:acme:C1"] {
        let reference: Ref = text.parse().unwrap();
        pane.update(cx, |p, cx| p.open_ref(&reference, cx));
        settle(&pane, cx);
        assert_eq!(channel_of(&pane, cx), before, "{text}");
    }
    assert!(
        before.is_some() && before == Some(channel),
        "the first channel stays open"
    );
}
