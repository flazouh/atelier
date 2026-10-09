use std::{cell::Cell, rc::Rc, sync::Arc};

use atelier_capabilities::{
    Actor, CapError, Ref,
    messaging::{ChannelKind, Feature, Formatting, MemoryMessaging, MessagingProvider, NewMessage, Operation},
};
use gpui_kit::{Entity, Focusable, ListOffset, ListState, TestAppContext, VisualTestContext, px, size};

use super::*;
use crate::messages::map::{Body, Line};

mod fake;
mod states;

use fake::Fake;

fn texts_of(lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .map(|l| match &l.body {
            Body::Plain(t) | Body::Markdown(t) => t.to_string(),
        })
        .collect()
}

/// A pane over `providers`, drawn 800 by 700, once every reading is done.
fn open(providers: Vec<Arc<dyn MessagingProvider>>, cx: &mut TestAppContext) -> (Entity<MessagesPane>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::init(cx);
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (pane, cx) = cx.add_window_view(move |window, cx| {
        let mut pane = MessagesPane::new(window, cx);
        pane.set_providers(providers, cx);
        pane
    });
    cx.simulate_resize(size(px(800.), px(700.)));
    settle(&pane, cx);
    (pane, cx)
}

fn one<'a>(provider: &Arc<Fake>, cx: &'a mut TestAppContext) -> (Entity<MessagesPane>, &'a mut VisualTestContext) {
    open(vec![provider.clone() as Arc<dyn MessagingProvider>], cx)
}

fn settle(pane: &Entity<MessagesPane>, cx: &mut VisualTestContext) {
    for _ in 0..6 {
        cx.run_until_parked();
        pane.update(cx, |_, cx| cx.notify());
    }
}

fn lines(pane: &Entity<MessagesPane>, cx: &mut VisualTestContext) -> Vec<String> {
    pane.read_with(cx, |p, _| texts_of(p.lines()))
}

fn press(name: &'static str, pane: &Entity<MessagesPane>, cx: &mut VisualTestContext) {
    let at = cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} is drawn")).center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    settle(pane, cx);
}

/// Types `text` in the composer shown and presses Enter, as the reader does.
fn type_and_send(text: &str, pane: &Entity<MessagesPane>, cx: &mut VisualTestContext) {
    let handle = cx.update(|_, cx| pane.read(cx).focus_handle(cx));
    cx.update(|window, cx| handle.focus(window, cx));
    cx.simulate_input(text);
    cx.simulate_keystrokes("enter");
    settle(pane, cx);
}

fn composer_text(pane: &Entity<MessagesPane>, cx: &mut VisualTestContext) -> String {
    let composer = pane.read_with(cx, |p, _| p.composer().clone());
    cx.update(|_, cx| composer.read(cx).text(cx).to_string())
}

fn heard_settings(pane: &Entity<MessagesPane>, cx: &mut VisualTestContext) -> Rc<Cell<bool>> {
    let heard = Rc::new(Cell::new(false));
    let told = heard.clone();
    cx.update(|_, cx| cx.subscribe(pane, move |_, _: &MessagesEvent, _| told.set(true)).detach());
    heard
}

#[test]
fn each_error_has_one_answer() {
    assert_eq!(react(&CapError::Offline), Reaction::Raise(Problem::Offline));
    assert_eq!(react(&CapError::NotSignedIn), Reaction::Raise(Problem::SignedOut));
    assert_eq!(react(&CapError::RateLimited { retry_after_ms: 1500 }), Reaction::Raise(Problem::Wait(1500)));
    assert_eq!(react(&CapError::unsupported("send")), Reaction::Hide);
    for other in [
        CapError::not_found("x"),
        CapError::invalid("text"),
        CapError::Storage { message: "disk".into() },
        CapError::Provider { code: "E1".into(), message: "boom".into() },
    ] {
        assert!(matches!(react(&other), Reaction::Line(words) if words == other.to_string()), "{other:?} is one line");
    }
}

#[gpui_kit::test]
fn the_history_reads_oldest_at_the_top_and_stays_at_the_newest(cx: &mut TestAppContext) {
    let names: Vec<String> = (0..30).map(|n| format!("message {n:02}")).collect();
    let (provider, _) = Fake::seeded("acme", &names.iter().map(String::as_str).collect::<Vec<_>>());
    let (pane, cx) = one(&provider, cx);
    let shown = lines(&pane, cx);
    assert_eq!(shown, names, "oldest first, newest last");
    let list = pane.read_with(cx, |p, _| p.list_state().clone());
    assert!(list.is_following_tail(), "the list follows the end");
    assert_ne!(list.is_scrolled_to_end(), Some(false), "it is at the end");
    assert_eq!(pane.read_with(cx, |p, _| p.accounts()[0].rows.len()), 1);
    assert!(cx.debug_bounds("message-0").is_none() || cx.debug_bounds("message-29").is_some(), "the newest is on screen");
}

#[gpui_kit::test]
fn a_new_message_that_arrives_through_subscribe_shows_without_a_reload(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &["first"]);
    let (pane, cx) = one(&provider, cx);
    assert_eq!(lines(&pane, cx), ["first"]);
    let ana = Actor::person("ana", "Ana");
    provider.memory().send(&NewMessage::to(&channel, "from Ana, just now"), &ana).unwrap();
    // The pane looks at its subscription on a timer; nothing else asks for the new message.
    cx.executor().advance_clock(POLL * 2);
    settle(&pane, cx);
    assert_eq!(lines(&pane, cx), ["first", "from Ana, just now"]);
    assert_eq!(provider.calls(Operation::Subscribe), 1);
}

#[gpui_kit::test]
fn a_message_in_another_channel_shows_as_unread_from_what_the_provider_says(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["first"]);
    let other = provider.memory().add_channel("design", ChannelKind::Private);
    let (pane, cx) = one(&provider, cx);
    let unread = |pane: &Entity<MessagesPane>, cx: &mut VisualTestContext| pane.read_with(cx, |p, _| p.accounts()[0].rows.iter().map(|r| r.unread).collect::<Vec<_>>());
    assert_eq!(unread(&pane, cx), [false, false]);
    provider.memory().send(&NewMessage::to(&other, "psst"), &Actor::person("ana", "Ana")).unwrap();
    provider.memory().set_unread(&other, 1).unwrap();
    cx.executor().advance_clock(POLL * 2);
    settle(&pane, cx);
    assert_eq!(unread(&pane, cx), [false, true], "the provider's count shows, and the open channel is not it");
    // Opening the channel reads it: the provider is told, and the dot goes.
    pane.update(cx, |p, cx| p.open_channel(0, &other, cx));
    settle(&pane, cx);
    assert_eq!(unread(&pane, cx), [false, false]);
    assert_eq!(provider.calls(Operation::MarkRead), 1);
}

#[gpui_kit::test]
fn a_channel_with_no_unread_is_not_marked_read(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["first"]);
    let (_pane, _cx) = one(&provider, cx);
    assert_eq!(provider.calls(Operation::MarkRead), 0, "nothing asked for it");
}

#[gpui_kit::test]
fn the_composer_is_there_only_when_the_provider_lists_send(cx: &mut TestAppContext) {
    let (with, _) = Fake::seeded("acme", &["hi"]);
    let (pane, cx) = one(&with, cx);
    assert!(cx.debug_bounds("messages-composer").is_some(), "send is listed");
    assert!(pane.read_with(cx, |p, _| p.composer_shown()));

    let (without, _) = Fake::seeded("acme", &["hi"]);
    without.without(Operation::Send);
    let (pane, cx) = one(&without, cx);
    assert_eq!(lines(&pane, cx), ["hi"], "the history shows");
    assert!(cx.debug_bounds("messages-composer").is_none(), "no send: no composer");
}

#[gpui_kit::test]
fn the_thread_link_is_there_only_where_the_provider_has_threads(cx: &mut TestAppContext) {
    let (with, channel) = Fake::seeded("acme", &["root"]);
    let root = with.memory().history(&channel, None, None).unwrap().items.remove(0);
    with.memory().send(&NewMessage::reply(&root.reference, &channel, "a reply"), &Actor::person("ana", "Ana")).unwrap();
    let (_pane, cx) = one(&with, cx);
    assert!(cx.debug_bounds("message-replies-0").is_some(), "a root with a reply has the link");

    for off in [Some(Feature::Threads), None] {
        let (without, channel) = Fake::seeded("acme", &["root"]);
        let root = without.memory().history(&channel, None, None).unwrap().items.remove(0);
        without.memory().send(&NewMessage::reply(&root.reference, &channel, "a reply"), &Actor::person("ana", "Ana")).unwrap();
        match off {
            Some(feature) => without.without_feature(feature),
            None => without.without(Operation::Thread),
        }
        let (_pane, cx) = one(&without, cx);
        assert!(cx.debug_bounds("message-0").is_some(), "the message shows");
        assert!(cx.debug_bounds("message-replies-0").is_none(), "no threads, no link ({off:?})");
    }
}

#[gpui_kit::test]
fn a_thread_opens_in_the_same_pane_and_a_reply_goes_to_the_thread(cx: &mut TestAppContext) {
    let (provider, channel) = Fake::seeded("acme", &["the root", "another"]);
    let root = provider.memory().history(&channel, None, None).unwrap().items.into_iter().find(|m| m.text == "the root").unwrap();
    provider.memory().send(&NewMessage::reply(&root.reference, &channel, "first reply"), &Actor::person("ana", "Ana")).unwrap();
    let (pane, cx) = one(&provider, cx);
    assert_eq!(lines(&pane, cx), ["the root", "another"]);
    press("message-replies-0", &pane, cx);
    assert_eq!(lines(&pane, cx), ["the root", "first reply"], "the root first, then its replies");
    assert!(cx.debug_bounds("messages-back").is_some(), "Back is there");
    assert!(cx.debug_bounds("message-replies-0").is_none(), "no link to the thread you are in");
    type_and_send("my reply", &pane, cx);
    assert_eq!(lines(&pane, cx), ["the root", "first reply", "my reply"]);
    let held = provider.memory().thread(&root.reference, None).unwrap().items;
    assert_eq!(held.last().unwrap().parent.as_ref(), Some(&root.reference), "it is a reply");
    assert!(!provider.memory().history(&channel, None, None).unwrap().items.iter().any(|m| m.text == "my reply"), "not a message of the channel");
    press("messages-back", &pane, cx);
    assert_eq!(lines(&pane, cx), ["the root", "another"]);
    assert!(pane.read_with(cx, |p, _| p.lines().iter().find(|l| l.replies > 0).map(|l| l.replies)) == Some(2), "the count grew");
}

#[gpui_kit::test]
fn a_page_with_a_cursor_has_a_load_older_row_that_reads_the_page_before(cx: &mut TestAppContext) {
    let names: Vec<String> = (0..8).map(|n| format!("m{n}")).collect();
    let (provider, _) = Fake::over(MemoryMessaging::new("acme").with_page_max(3), &names.iter().map(String::as_str).collect::<Vec<_>>());
    let (pane, cx) = one(&provider, cx);
    assert_eq!(lines(&pane, cx), ["m5", "m6", "m7"], "the newest page");
    assert!(cx.debug_bounds("messages-load-older").is_some());
    press("messages-load-older", &pane, cx);
    assert_eq!(lines(&pane, cx), ["m2", "m3", "m4", "m5", "m6", "m7"], "the older page goes above, in order");
    press("messages-load-older", &pane, cx);
    assert_eq!(lines(&pane, cx), names, "all eight, once each");
    assert!(cx.debug_bounds("messages-load-older").is_none(), "no cursor, no row");
}

#[gpui_kit::test]
fn a_page_with_no_cursor_has_no_load_older_row(cx: &mut TestAppContext) {
    let (provider, _) = Fake::seeded("acme", &["one", "two"]);
    let (_pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("messages-load-older").is_none());
}

#[test]
fn an_older_page_is_added_above_without_moving_what_the_reader_is_looking_at() {
    fn line(n: usize) -> Line {
        let reference: Ref = format!("messaging:memory:acme:C1:{n}").parse().unwrap();
        let message = atelier_capabilities::messaging::Message {
            reference: reference.clone(),
            channel: "messaging:memory:acme:C1".parse().unwrap(),
            parent: None,
            text: format!("m{n}"),
            author: Actor::person("me", "Me"),
            created_at: 0,
            edited_at: None,
            attachments: Vec::new(),
            reactions: Vec::new(),
            reply_count: 0,
            origin: None,
            raw: None,
        };
        crate::messages::map::line_of(&message, Formatting::Rich, 0)
    }
    let list = ListState::new(0, gpui_kit::ListAlignment::Bottom, px(100.));
    let mut shown = Rc::new(Vec::new());
    apply(&mut shown, &list, (5..10).map(line).collect());
    assert_eq!(list.item_count(), 5);
    list.scroll_to(ListOffset { item_ix: 2, offset_in_item: px(10.) });
    apply(&mut shown, &list, (2..10).map(line).collect());
    assert_eq!(list.item_count(), 8);
    assert_eq!(list.logical_scroll_top().item_ix, 5, "the row that was third is still the one on top, three rows lower");
    assert_eq!(list.logical_scroll_top().offset_in_item, px(10.));
    // The same lines again change nothing.
    apply(&mut shown, &list, (2..10).map(line).collect());
    assert_eq!(list.item_count(), 8);
    // A row that changed in place is one row of the list.
    let mut changed: Vec<Line> = (2..10).map(line).collect();
    changed[3].replies = 4;
    apply(&mut shown, &list, changed);
    assert_eq!(list.item_count(), 8);
    assert_eq!(shown[3].replies, 4);
}
