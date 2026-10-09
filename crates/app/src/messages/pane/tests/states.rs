//! What the pane shows for each way a call fails, for no account, one and two, and for what a message holds.
use std::sync::Arc;

use atelier_capabilities::{
    Actor, CapError,
    messaging::{Attachment, AttachmentKind, ChannelKind, Formatting, MessagingProvider, NewMessage, Operation},
};
use gpui_kit::TestAppContext;

use super::{Fake, POLL, Problem, composer_text, heard_settings, lines, one, open, press, settle, texts_of, type_and_send};
use crate::messages::map::Body;

fn text(words: &str) -> String {
    words.to_string()
}

#[gpui_kit::test]
fn offline_is_a_banner_with_retry_and_retry_reads_again(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["hello"]);
    provider.fail(Operation::History, CapError::Offline);
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("messages-banner").is_some(), "a banner");
    assert!(cx.debug_bounds("messages-retry").is_some(), "with Retry");
    assert!(lines(&pane, cx).is_empty());
    provider.heal(Operation::History);
    press("messages-retry", &pane, cx);
    assert_eq!(lines(&pane, cx), ["hello"], "Retry read it again");
    assert!(cx.debug_bounds("messages-banner").is_none(), "and the banner went");
}

#[gpui_kit::test]
fn offline_while_reading_the_channels_is_the_same_banner(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["hello"]);
    provider.fail(Operation::Channels, CapError::Offline);
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("messages-banner").is_some() && cx.debug_bounds("messages-retry").is_some());
    provider.heal(Operation::Channels);
    press("messages-retry", &pane, cx);
    assert_eq!(lines(&pane, cx), ["hello"], "the channels are read again and the first one opens");
    assert!(cx.debug_bounds("messages-banner").is_none());
}

#[gpui_kit::test]
fn not_signed_in_is_an_empty_state_with_a_button_that_opens_settings(cx: &mut TestAppContext) {
    for failing in [Operation::Channels, Operation::History] {
        let (provider, _) = Fake::seeded("acme", &["hello"]);
        provider.fail(failing, CapError::NotSignedIn);
        let (pane, cx) = one(&provider, cx);
        assert!(cx.debug_bounds("messages-signed-out").is_some(), "{failing:?}: an empty state");
        assert!(cx.debug_bounds("messages-composer").is_none(), "{failing:?}: no composer");
        let heard = heard_settings(&pane, cx);
        press("messages-sign-in", &pane, cx);
        assert!(heard.get(), "{failing:?}: the button asks the app for Settings");
    }
}

#[gpui_kit::test]
fn rate_limited_is_a_banner_with_the_wait_and_no_retry(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["hello"]);
    provider.fail(Operation::History, CapError::RateLimited { retry_after_ms: 2500 });
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("messages-banner").is_some());
    assert!(cx.debug_bounds("messages-retry").is_none(), "a wait is waited out");
    assert!(pane.read_with(cx, |p, _| p.effective_problem()) == Some(Problem::Wait(2500)));
}

#[gpui_kit::test]
fn a_call_that_is_not_offered_loses_its_control(cx: &mut TestAppContext) {
    // The provider lists `send` and answers `Unsupported`: the composer goes, after the first try.
    let (provider, _) = Fake::seeded("acme", &["hello"]);
    provider.fail(Operation::Send, CapError::unsupported("send a message"));
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("messages-composer").is_some());
    type_and_send("hi", &pane, cx);
    assert!(cx.debug_bounds("messages-composer").is_none(), "the control is hidden");
    // The same for the thread link.
    let (provider, channel) = Fake::seeded("acme", &["root"]);
    let root = provider.memory().history(&channel, None, None).unwrap().items.remove(0);
    provider.memory().send(&NewMessage::reply(&root.reference, &channel, "reply"), &Actor::person("ana", "Ana")).unwrap();
    provider.fail(Operation::Thread, CapError::unsupported("read a thread"));
    let (pane, cx) = one(&provider, cx);
    press("message-replies-0", &pane, cx);
    assert_eq!(lines(&pane, cx), ["root"], "back in the channel");
    assert!(cx.debug_bounds("message-replies-0").is_none(), "the link is gone");
}

#[gpui_kit::test]
fn any_other_error_is_one_line(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["hello"]);
    let error = CapError::Provider { code: "E42".into(), message: "boom".into() };
    provider.fail(Operation::History, error.clone());
    let (pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("messages-banner").is_none() && cx.debug_bounds("messages-signed-out").is_none());
    assert!(cx.debug_bounds("messages-line").is_some(), "the line stands in the middle");
    assert!(lines(&pane, cx).is_empty());
    // And an error reading an older page is a line under the header, with the messages still there.
    let names: Vec<String> = (0..8).map(|n| format!("m{n}")).collect();
    let (provider, _) = Fake::over(atelier_capabilities::messaging::MemoryMessaging::new("acme").with_page_max(3), &names.iter().map(String::as_str).collect::<Vec<_>>());
    let (pane, cx) = one(&provider, cx);
    provider.fail(Operation::History, error);
    press("messages-load-older", &pane, cx);
    assert!(pane.read_with(cx, |p, _| p.said().is_some_and(|s| s.contains("E42: boom"))), "{:?}", pane.read_with(cx, |p, _| p.said().map(text)));
    assert_eq!(lines(&pane, cx), ["m5", "m6", "m7"], "what was read stays");
}

#[gpui_kit::test]
fn a_failed_send_keeps_the_text_and_says_why(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &["hello"]);
    let (pane, cx) = one(&provider, cx);
    provider.fail(Operation::Send, CapError::Offline);
    type_and_send("an important reply", &pane, cx);
    assert_eq!(composer_text(&pane, cx), "an important reply", "the text is back in the box");
    assert!(pane.read_with(cx, |p, _| p.said().is_some_and(|s| s.contains("Could not send") && s.contains("no connection"))));
    assert!(cx.debug_bounds("messages-said").is_some(), "the reason is on screen");
    assert_eq!(provider.memory().history(&channel, None, None).unwrap().items.len(), 1, "nothing went out");
    provider.heal(Operation::Send);
    cx.simulate_keystrokes("enter");
    settle(&pane, cx);
    assert_eq!(composer_text(&pane, cx), "", "sent, so the box is empty");
    assert_eq!(lines(&pane, cx), ["hello", "an important reply"]);
}

#[gpui_kit::test]
fn the_pane_never_sends_by_itself(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["hello"]);
    let (pane, cx) = one(&provider, cx);
    provider.memory().send(&NewMessage::to(&pane.read_with(cx, |p, _| p.open_channel_ref().unwrap().1.clone()), "from Ana"), &Actor::person("ana", "Ana")).unwrap();
    cx.executor().advance_clock(POLL * 2);
    settle(&pane, cx);
    assert_eq!(provider.calls(Operation::Send), 0, "reading, loading, and subscribing send nothing");
}

#[gpui_kit::test]
fn what_the_reader_sends_goes_as_the_person(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &[]);
    let (pane, cx) = one(&provider, cx);
    type_and_send("hello there", &pane, cx);
    let sent = provider.memory().history(&channel, None, None).unwrap().items.remove(0);
    assert_eq!(sent.author, provider.memory().whoami().unwrap());
    assert_eq!(sent.origin, None, "the person's own words have no agent origin");
}

#[gpui_kit::test]
fn the_screen_is_built_for_no_account_one_and_two(cx: &mut TestAppContext) {
    // None: an empty state that says so, with a way to Settings.
    let (pane, cx) = open(Vec::new(), cx);
    assert!(cx.debug_bounds("messages-empty").is_some());
    assert!(cx.debug_bounds("messages-composer").is_none());
    let heard = heard_settings(&pane, cx);
    press("messages-open-settings", &pane, cx);
    assert!(heard.get());

    // One: its first channel is open.
    let (first, _) = Fake::seeded("acme", &["from acme"]);
    let (pane, cx) = open(vec![first.clone() as Arc<dyn MessagingProvider>], cx);
    assert_eq!(pane.read_with(cx, |p, _| p.accounts().len()), 1);
    assert_eq!(lines(&pane, cx), ["from acme"]);

    // Two: both are listed, in order, and each shows its own history.
    let (second, second_channel) = Fake::seeded("beta", &["from beta"]);
    let (pane, cx) = open(vec![second.clone() as Arc<dyn MessagingProvider>, first.clone() as Arc<dyn MessagingProvider>], cx);
    let accounts: Vec<String> = pane.read_with(cx, |p, _| p.accounts().iter().map(|a| a.choice.account.clone()).collect());
    assert_eq!(accounts, ["acme", "beta"], "the registry's order");
    assert_eq!(lines(&pane, cx), ["from acme"]);
    pane.update(cx, |p, cx| p.open_channel(1, &second_channel, cx));
    settle(&pane, cx);
    assert_eq!(lines(&pane, cx), ["from beta"]);
    assert_eq!(pane.read_with(cx, |p, _| p.open_channel_ref().map(|(at, _)| at)), Some(1));
    // One account that fails does not take the other with it.
    first.fail(Operation::Channels, CapError::Offline);
    pane.update(cx, |p, cx| p.reload(cx));
    settle(&pane, cx);
    assert_eq!(lines(&pane, cx), ["from beta"]);
}

#[gpui_kit::test]
fn the_same_providers_are_not_read_again(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["hello"]);
    let (pane, cx) = one(&provider, cx);
    assert_eq!(provider.calls(Operation::Channels), 1);
    pane.update(cx, |p, cx| p.set_providers(vec![provider.clone() as Arc<dyn MessagingProvider>], cx));
    settle(&pane, cx);
    assert_eq!(provider.calls(Operation::Channels), 1, "an answer that has not changed asks for nothing");
    assert_eq!(lines(&pane, cx), ["hello"]);
}

#[gpui_kit::test]
fn text_is_drawn_as_text_and_never_run_as_markup(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &[]);
    let ana = Actor::person("ana", "Ana");
    for hostile in ["<script>alert(1)</script> hi", "[click](javascript:alert(1))", "![x](https://evil.test/p.png)", "# loud <b>bold</b>"] {
        provider.memory().send(&NewMessage::to(&channel, hostile), &ana).unwrap();
    }
    let (pane, cx) = one(&provider, cx);
    let shown = lines(&pane, cx);
    assert_eq!(shown.len(), 4);
    for body in &shown {
        // A tag may be in the text, but only with a backslash before it: that is a character, not a tag.
        assert!(!body.replace("\\<", "").contains('<') && !body.contains("javascript:") && !body.contains("!["), "{body}");
    }
    assert!(shown[0].contains(r"\<script>"), "the tag stays as written, in a form that is text: {}", shown[0]);
    // A provider that keeps no formatting shows every character as typed.
    provider.keeping(Formatting::Plain);
    pane.update(cx, |p, cx| p.reload(cx));
    settle(&pane, cx);
    let plain = pane.read_with(cx, |p, _| p.lines().iter().map(|l| matches!(l.body, Body::Plain(_))).all(|plain| plain));
    assert!(plain);
    assert_eq!(texts_of(&pane.read_with(cx, |p, _| p.lines().to_vec()))[0], "<script>alert(1)</script> hi");
}

#[gpui_kit::test]
fn an_agents_message_shows_whose_agent_sent_it_and_a_message_shows_its_chips(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &[]);
    let memory = provider.memory();
    let agent = Actor::agent("claude", "Claude", "me");
    memory.send(&NewMessage::to(&channel, "tests pass"), &agent).unwrap();
    let report = memory.send(&NewMessage::to(&channel, "the report"), &Actor::person("ana", "Ana")).unwrap();
    memory
        .attach(&report.reference, Attachment { id: "1".into(), name: "report.pdf".into(), mime: None, size: Some(4096), url: "https://x.test/r.pdf".into(), kind: AttachmentKind::File })
        .unwrap();
    memory.react(&report.reference, "thumbsup", true, &Actor::person("ben", "Ben")).unwrap();
    let (pane, cx) = one(&provider, cx);
    assert_eq!(lines(&pane, cx), ["tests pass", "the report"]);
    assert!(cx.debug_bounds("message-origin-0").is_some(), "the agent's message names its origin");
    assert!(cx.debug_bounds("message-origin-1").is_none(), "a person's does not");
    assert!(cx.debug_bounds("message-file-1-0").is_some(), "the file");
    assert!(cx.debug_bounds("message-reaction-1-0").is_some(), "the reaction");
    let _ = ChannelKind::Public;
}
