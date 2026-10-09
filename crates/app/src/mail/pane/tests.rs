use std::{cell::Cell, rc::Rc, sync::Arc};

use atelier_capabilities::{
    Actor, CapError, Ref,
    mail::{Incoming, MailOperation, MailProvider, Role, SearchQuery, mailbox_ref},
};
use gpui_kit::{Entity, TestAppContext, VisualTestContext, px, size};

use super::*;

mod compose;
mod fake;
mod states;

use fake::Fake;

const ME: &str = "me@example.com";

/// A pane over `providers`, drawn 1000 by 700, once every reading is done.
fn open(
    providers: Vec<Arc<dyn MailProvider>>,
    cx: &mut TestAppContext,
) -> (Entity<MailPane>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        atelier_ui::init(cx);
        atelier_ui::theme::set_appearance(atelier_ui::theme::Appearance::Light, cx);
        cx.set_reduce_motion(true);
    });
    let (pane, cx) = cx.add_window_view(move |window, cx| {
        let mut pane = MailPane::new(Actor::person("me", "me"), window, cx);
        pane.set_providers(providers, cx);
        pane
    });
    cx.simulate_resize(size(px(1000.), px(700.)));
    settle(&pane, cx);
    (pane, cx)
}

fn one<'a>(
    provider: &Arc<Fake>,
    cx: &'a mut TestAppContext,
) -> (Entity<MailPane>, &'a mut VisualTestContext) {
    open(vec![provider.clone() as Arc<dyn MailProvider>], cx)
}

fn settle(pane: &Entity<MailPane>, cx: &mut VisualTestContext) {
    for _ in 0..6 {
        cx.run_until_parked();
        pane.update(cx, |_, cx| cx.notify());
    }
}

fn subjects(pane: &Entity<MailPane>, cx: &mut VisualTestContext) -> Vec<String> {
    pane.read_with(cx, |p, _| {
        p.rows().iter().map(|r| r.subject.to_string()).collect()
    })
}

fn unread(pane: &Entity<MailPane>, cx: &mut VisualTestContext) -> Vec<bool> {
    pane.read_with(cx, |p, _| p.rows().iter().map(|r| r.unread).collect())
}

fn press(name: &'static str, pane: &Entity<MailPane>, cx: &mut VisualTestContext) {
    let at = cx
        .debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} is drawn"))
        .center();
    cx.simulate_click(at, gpui_kit::Modifiers::default());
    settle(pane, cx);
}

/// A name for the control socket and the tests, which keep theirs for as long as the process lives.
fn named(name: String) -> &'static str {
    Box::leak(name.into_boxed_str())
}

/// Opens the thread of row `n`, as a press on it does.
fn open_row(n: usize, pane: &Entity<MailPane>, cx: &mut VisualTestContext) {
    press(named(format!("mail-thread-{n}")), pane, cx);
}

fn inbox(pane: &Entity<MailPane>, cx: &mut VisualTestContext) -> Ref {
    pane.read_with(cx, |p, _| {
        p.accounts()[0]
            .boxes
            .iter()
            .find(|b| b.role == Role::Inbox)
            .expect("an inbox")
            .reference
            .clone()
    })
}

fn heard_settings(pane: &Entity<MailPane>, cx: &mut VisualTestContext) -> Rc<Cell<bool>> {
    let heard = Rc::new(Cell::new(false));
    let told = heard.clone();
    cx.update(|_, cx| {
        cx.subscribe(pane, move |_, _: &MailPaneEvent, _| told.set(true))
            .detach()
    });
    heard
}

#[test]
fn each_error_has_one_answer() {
    assert_eq!(react(&CapError::Offline), Reaction::Raise(Problem::Offline));
    assert_eq!(
        react(&CapError::NotSignedIn),
        Reaction::Raise(Problem::SignedOut)
    );
    assert_eq!(
        react(&CapError::RateLimited {
            retry_after_ms: 1500
        }),
        Reaction::Raise(Problem::Wait(1500))
    );
    assert_eq!(react(&CapError::unsupported("star")), Reaction::Hide);
    for other in [
        CapError::not_found("x"),
        CapError::invalid("text"),
        CapError::Storage {
            message: "disk".into(),
        },
        CapError::Provider {
            code: "E1".into(),
            message: "boom".into(),
        },
    ] {
        assert!(
            matches!(react(&other), Reaction::Line(words) if words == other.to_string()),
            "{other:?} is one line"
        );
    }
}

#[gpui_kit::test]
fn the_inbox_opens_first_with_its_threads_newest_first(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["oldest", "middle", "newest"]);
    let (pane, cx) = one(&provider, cx);
    assert_eq!(subjects(&pane, cx), ["newest", "middle", "oldest"]);
    let (title, shown) = pane.read_with(cx, |p, _| {
        (p.title().to_string(), p.open_mailbox_ref().is_some())
    });
    assert_eq!(title, "Inbox");
    assert!(shown);
    let order: Vec<Role> = pane.read_with(cx, |p, _| {
        p.accounts()[0].boxes.iter().map(|b| b.role).collect()
    });
    assert_eq!(
        order,
        [
            Role::Inbox,
            Role::Sent,
            Role::Drafts,
            Role::Trash,
            Role::Spam,
            Role::Archive,
            Role::Custom
        ],
        "the sidebar's order"
    );
    assert!(
        cx.debug_bounds("mail-thread-0").is_some() && cx.debug_bounds("mail-thread-2").is_some()
    );
    assert_eq!(
        provider.calls(MailOperation::Thread),
        0,
        "nothing is opened until the reader asks"
    );
}

#[gpui_kit::test]
fn an_unread_thread_has_its_dot_and_a_read_one_has_none(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["old", "new"]);
    let thread = provider
        .memory()
        .search(&SearchQuery::default())
        .unwrap()
        .items
        .remove(1);
    provider
        .memory()
        .mark_read(&thread.reference, true, &Actor::person("me", "me"))
        .unwrap();
    let (pane, cx) = one(&provider, cx);
    assert_eq!(unread(&pane, cx), [true, false]);
    assert!(
        cx.debug_bounds("mail-unread-0").is_some(),
        "the unread thread has its dot"
    );
    assert!(
        cx.debug_bounds("mail-unread-1").is_none(),
        "the read one has none"
    );
    assert_eq!(
        pane.read_with(cx, |p, _| p.accounts()[0].boxes[0].unread),
        1,
        "the mailbox counts what the provider says"
    );
}

#[gpui_kit::test]
fn a_page_ends_with_load_more_and_each_press_reads_the_next_page_once(cx: &mut TestAppContext) {
    let names: Vec<String> = (0..7).map(|n| format!("thread {n}")).collect();
    let provider = Fake::seeded(ME, &names.iter().map(String::as_str).collect::<Vec<_>>());
    provider.page_max(3);
    let (pane, cx) = one(&provider, cx);
    assert_eq!(subjects(&pane, cx).len(), 3, "a page of three");
    assert!(
        cx.debug_bounds("mail-load-more").is_some(),
        "and a row to read more"
    );
    press("mail-load-more", &pane, cx);
    assert_eq!(subjects(&pane, cx).len(), 6);
    press("mail-load-more", &pane, cx);
    let all = subjects(&pane, cx);
    assert_eq!(all.len(), 7);
    let mut sorted = all.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 7, "no thread twice: {all:?}");
    assert!(
        cx.debug_bounds("mail-load-more").is_none(),
        "no more to read, no row"
    );
}

#[gpui_kit::test]
fn a_message_that_arrives_through_subscribe_adds_its_thread_without_a_reload(
    cx: &mut TestAppContext,
) {
    let provider = Fake::seeded(ME, &["first"]);
    let (pane, cx) = one(&provider, cx);
    assert_eq!(subjects(&pane, cx), ["first"]);
    provider
        .memory()
        .receive(&Incoming::new(
            "ben@example.com",
            ME,
            "just now",
            "hello",
            1_790_000_900_000,
        ))
        .unwrap();
    // The pane looks at its subscription on a timer; nothing else asks for the new thread.
    cx.executor().advance_clock(POLL * 2);
    settle(&pane, cx);
    assert_eq!(subjects(&pane, cx), ["just now", "first"]);
    assert_eq!(provider.calls(MailOperation::Subscribe), 1);
    assert_eq!(
        pane.read_with(cx, |p, _| p.accounts()[0].boxes[0].unread),
        2,
        "and the count of the mailbox follows"
    );
}

#[gpui_kit::test]
fn opening_a_thread_shows_its_messages_oldest_first_and_marks_nothing_read(
    cx: &mut TestAppContext,
) {
    let provider = Fake::seeded(ME, &["plan"]);
    let first = provider
        .memory()
        .search(&SearchQuery::default())
        .unwrap()
        .items
        .remove(0);
    provider
        .memory()
        .receive(&Incoming {
            thread: Some(first.reference.clone()),
            ..Incoming::new(
                "ben@example.com",
                ME,
                "Re: plan",
                "second message",
                1_790_000_060_000,
            )
        })
        .unwrap();
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    let bodies: Vec<String> = pane.read_with(cx, |p, _| {
        p.messages().iter().map(|m| m.text.to_string()).collect()
    });
    assert_eq!(
        bodies,
        ["The body of plan", "second message"],
        "oldest first"
    );
    assert!(
        cx.debug_bounds("mail-message-0").is_some() && cx.debug_bounds("mail-message-1").is_some()
    );
    assert_eq!(
        provider.calls(MailOperation::MarkRead),
        0,
        "the pane never marks a thread read on its own"
    );
    assert_eq!(
        unread(&pane, cx),
        [true],
        "it stays unread, as the provider says"
    );
}

#[gpui_kit::test]
fn a_provider_that_marks_a_thread_read_by_giving_it_shows_it_read(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    provider.marks_read_on_read();
    let (pane, cx) = one(&provider, cx);
    assert_eq!(unread(&pane, cx), [true]);
    assert_eq!(
        pane.read_with(cx, |p, _| p.accounts()[0].boxes[0].unread),
        1
    );
    open_row(0, &pane, cx);
    assert_eq!(
        unread(&pane, cx),
        [false],
        "the row follows what the provider says"
    );
    assert_eq!(
        pane.read_with(cx, |p, _| p.accounts()[0].boxes[0].unread),
        0,
        "and so does the count of the mailbox"
    );
    assert_eq!(
        provider.calls(MailOperation::MarkRead),
        0,
        "the pane asked for nothing"
    );
}

#[gpui_kit::test]
fn the_reader_marks_a_thread_read_and_unread(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["plan"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    press("mail-mark-read", &pane, cx);
    assert_eq!(provider.calls(MailOperation::MarkRead), 1);
    assert_eq!(unread(&pane, cx), [false]);
    assert_eq!(
        pane.read_with(cx, |p, _| p.accounts()[0].boxes[0].unread),
        0
    );
    press("mail-mark-read", &pane, cx);
    assert_eq!(provider.calls(MailOperation::MarkRead), 2);
    assert_eq!(
        unread(&pane, cx),
        [true],
        "the same button marks it unread again"
    );
}

#[gpui_kit::test]
fn star_archive_trash_and_move_change_the_provider_and_the_screen(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["one", "two", "three"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    press("mail-star", &pane, cx);
    assert_eq!(provider.calls(MailOperation::Star), 1);
    assert!(
        cx.debug_bounds("mail-star-0").is_some(),
        "the row has its star"
    );
    press("mail-archive", &pane, cx);
    assert_eq!(
        subjects(&pane, cx),
        ["two", "one"],
        "an archived thread leaves the mailbox"
    );
    assert!(
        pane.read_with(cx, |p, _| p.open_thread_ref().is_none()),
        "and the reading pane closes"
    );
    open_row(0, &pane, cx);
    press("mail-trash", &pane, cx);
    assert_eq!(subjects(&pane, cx), ["one"]);
    // Move: the menu lists the other mailboxes, and the first of them is Sent.
    open_row(0, &pane, cx);
    press("mail-move", &pane, cx);
    let others = pane.read_with(cx, |p, _| p.accounts()[0].boxes.len() - 1);
    assert!(
        cx.debug_bounds(named(format!("mail-move-{}", others - 1)))
            .is_some()
    );
    assert!(
        cx.debug_bounds(named(format!("mail-move-{others}")))
            .is_none(),
        "the open mailbox is not a target"
    );
    press("mail-move-0", &pane, cx);
    assert_eq!(provider.calls(MailOperation::Move), 1);
    assert!(
        subjects(&pane, cx).is_empty(),
        "the moved thread left the inbox"
    );
    assert_eq!(provider.calls(MailOperation::Archive), 1);
    assert_eq!(provider.calls(MailOperation::Trash), 1);
}

#[gpui_kit::test]
fn a_label_is_added_and_taken_off_from_its_menu(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["one"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    press("mail-label", &pane, cx);
    press("mail-label-0", &pane, cx);
    assert_eq!(provider.calls(MailOperation::Label), 1);
    let label = mailbox_ref("memory", ME, "label-receipts");
    let thread = provider
        .memory()
        .search(&SearchQuery::default())
        .unwrap()
        .items
        .remove(0);
    assert!(
        thread.mailboxes.contains(&label),
        "the thread has the label now"
    );
    press("mail-label", &pane, cx);
    press("mail-label-0", &pane, cx);
    let thread = provider
        .memory()
        .search(&SearchQuery::default())
        .unwrap()
        .items
        .remove(0);
    assert!(
        !thread.mailboxes.contains(&label),
        "the same row takes it off"
    );
}

#[gpui_kit::test]
fn a_press_on_another_mailbox_opens_it_and_a_search_reads_it_with_the_words(
    cx: &mut TestAppContext,
) {
    let provider = Fake::seeded(ME, &["apples", "pears", "apple pie"]);
    let (pane, cx) = one(&provider, cx);
    assert!(
        cx.debug_bounds("mail-search-field").is_some(),
        "the provider lists search"
    );
    let input = pane.read_with(cx, |p, _| p.search_input().clone());
    pane.update_in(cx, |_, window, cx| {
        input.update(cx, |i, cx| {
            i.set_value("apple", window, cx);
            i.focus(window, cx);
        })
    });
    cx.simulate_keystrokes("enter");
    settle(&pane, cx);
    assert_eq!(subjects(&pane, cx), ["apple pie", "apples"]);
    let last = provider.queries().pop().unwrap();
    assert_eq!(
        last.text, "apple",
        "the words go to the provider as they are"
    );
    assert_eq!(last.mailbox, Some(inbox(&pane, cx)));
    // Words that match nothing say so.
    pane.update_in(cx, |_, window, cx| {
        input.update(cx, |i, cx| i.set_value("zebra", window, cx))
    });
    cx.simulate_keystrokes("enter");
    settle(&pane, cx);
    assert!(subjects(&pane, cx).is_empty());
    assert!(
        cx.debug_bounds("mail-line").is_some(),
        "a line says nothing matched"
    );
}

#[gpui_kit::test]
fn a_provider_with_no_search_has_no_search_field(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["one"]);
    provider.without(MailOperation::Search);
    let (_pane, cx) = one(&provider, cx);
    assert!(cx.debug_bounds("mail-search-field").is_none());
    assert!(
        cx.debug_bounds("mail-search").is_none(),
        "and nothing in its place"
    );
}

#[gpui_kit::test]
fn the_actions_are_the_ones_the_provider_lists(cx: &mut TestAppContext) {
    let all = [
        "mail-star",
        "mail-mark-read",
        "mail-archive",
        "mail-trash",
        "mail-move",
        "mail-label",
    ];
    let provider = Fake::seeded(ME, &["one"]);
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    for name in all {
        assert!(cx.debug_bounds(name).is_some(), "{name} is listed");
    }
    for (left_out, button) in [
        (MailOperation::Star, "mail-star"),
        (MailOperation::MarkRead, "mail-mark-read"),
        (MailOperation::Archive, "mail-archive"),
        (MailOperation::Trash, "mail-trash"),
        (MailOperation::Move, "mail-move"),
        (MailOperation::Label, "mail-label"),
    ] {
        let provider = Fake::seeded(ME, &["one"]);
        provider.without(left_out);
        let (pane, cx) = one(&provider, cx);
        open_row(0, &pane, cx);
        assert!(
            cx.debug_bounds(button).is_none(),
            "no {left_out:?}, no {button}"
        );
        for other in all.iter().filter(|n| **n != button) {
            assert!(cx.debug_bounds(other).is_some(), "{other} stays");
        }
    }
}

#[gpui_kit::test]
fn a_read_only_provider_shows_no_action_and_no_composer(cx: &mut TestAppContext) {
    let provider = Fake::seeded(ME, &["one"]);
    provider.read_only();
    let (pane, cx) = one(&provider, cx);
    open_row(0, &pane, cx);
    assert!(
        cx.debug_bounds("mail-message-0").is_some(),
        "the thread reads"
    );
    for name in [
        "mail-actions",
        "mail-star",
        "mail-mark-read",
        "mail-archive",
        "mail-trash",
        "mail-move",
        "mail-label",
        "mail-composer",
        "mail-save-draft",
        "mail-send",
    ] {
        assert!(cx.debug_bounds(name).is_none(), "{name} is not drawn");
    }
    assert!(
        cx.debug_bounds("mail-subject").is_some(),
        "the head is there with the subject"
    );
}
