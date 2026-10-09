//! A reference a tool card names opens in the pane.
use atelier_capabilities::{Ref, mail::Role};
use gpui_kit::TestAppContext;

use super::{Fake, ME, one, settle};

fn opened(
    pane: &gpui_kit::Entity<super::MailPane>,
    cx: &mut gpui_kit::VisualTestContext,
) -> Option<Ref> {
    pane.read_with(cx, |p, _| p.open_thread_ref().cloned())
}

#[gpui_kit::test]
fn a_thread_reference_opens_the_thread_with_a_mailbox_behind_it(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello", "plan"]);
    let (pane, cx) = one(&provider, cx);
    let thread = pane.read_with(cx, |p, _| p.rows()[1].reference.clone());
    pane.update_in(cx, |p, window, cx| p.open_ref(&thread, window, cx));
    settle(&pane, cx);
    assert_eq!(opened(&pane, cx), Some(thread));
    assert!(
        pane.read_with(cx, |p, _| p.open_mailbox_ref().is_some()),
        "a mailbox stands behind the thread"
    );
    assert!(
        !pane.read_with(cx, |p, _| p.messages().is_empty()),
        "the thread is read"
    );
}

#[gpui_kit::test]
fn a_mailbox_reference_opens_the_mailbox(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    let (pane, cx) = one(&provider, cx);
    let sent = pane.read_with(cx, |p, _| {
        p.accounts()[0]
            .boxes
            .iter()
            .find(|b| b.role == Role::Sent)
            .expect("a sent mailbox")
            .reference
            .clone()
    });
    pane.update_in(cx, |p, window, cx| p.open_ref(&sent, window, cx));
    settle(&pane, cx);
    assert_eq!(
        pane.read_with(cx, |p, _| p.open_mailbox_ref().map(|(_, m)| m.clone())),
        Some(sent)
    );
}

#[gpui_kit::test]
fn a_reference_of_another_account_or_of_another_kind_changes_nothing(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    let (pane, cx) = one(&provider, cx);
    let before = pane.read_with(cx, |p, _| {
        p.open_mailbox_ref().map(|(at, m)| (at, m.clone()))
    });
    for text in [
        "mail:memory:other@example.org:t:1",
        "mail:memory:me@example.com:m:2",
        "tasks:memory:me@example.com:t:1",
    ] {
        let reference: Ref = text.parse().expect("a reference");
        pane.update_in(cx, |p, window, cx| p.open_ref(&reference, window, cx));
        settle(&pane, cx);
        assert_eq!(opened(&pane, cx), None, "{text}");
    }
    assert_eq!(
        pane.read_with(cx, |p, _| p
            .open_mailbox_ref()
            .map(|(at, m)| (at, m.clone()))),
        before
    );
}

#[gpui_kit::test]
fn the_pane_says_when_it_has_read_the_mailboxes_of_an_account(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["hello"]);
    let (pane, cx) = one(&provider, cx);
    let here: Ref = format!("mail:memory:{ME}:t:1").parse().unwrap();
    let elsewhere: Ref = "mail:memory:other@example.org:t:1".parse().unwrap();
    assert!(pane.read_with(cx, |p, _| p.mailboxes_ready(&here)));
    assert!(
        pane.read_with(cx, |p, _| p.mailboxes_ready(&elsewhere)),
        "an account that is not here has nothing to wait for"
    );
}
