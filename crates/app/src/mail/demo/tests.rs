use atelier_capabilities::{
    Actor, CapError,
    mail::{MailOperation, MailProvider, Role, SearchQuery},
};

use super::*;
use crate::capability_hub::CapabilityHub;
use crate::mail::map::BODY_LIMIT;

fn hub() -> CapabilityHub {
    CapabilityHub::new(Actor::person("alex", "Alex"), None, false)
}

#[test]
fn nothing_is_registered_without_the_variable() {
    for value in [None, Some(""), Some("0"), Some("no"), Some("false"), Some("readonly-ish")] {
        let hub = hub();
        assert!(!register(&hub, value), "{value:?} asks for nothing");
        assert!(
            hub.mail_providers().is_empty(),
            "{value:?} leaves the registry empty"
        );
    }
}

/// A release build has no demo at all: the module, and the call that registers it, stand behind `debug_assertions`. A debug test
/// cannot compile a release build, so it reads the two places the gate is written.
#[test]
fn a_release_build_has_no_demo() {
    let module = include_str!("../../mail.rs");
    assert!(
        module.contains("#[cfg(debug_assertions)]\npub mod demo;"),
        "the module is a debug-only module"
    );
    let main = include_str!("../../main.rs");
    let call = main
        .find("mail::demo::register_from_env")
        .expect("main registers the demo");
    assert!(
        main[..call].trim_end().ends_with("#[cfg(debug_assertions)]\n        let _ ="),
        "the call is a debug-only call"
    );
}

#[test]
fn the_variable_registers_one_seeded_account() {
    for value in ["1", "true", "yes"] {
        let hub = hub();
        assert!(register(&hub, Some(value)));
        let providers = hub.mail_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!((providers[0].provider(), providers[0].account()), ("memory", ADDRESS));
        assert!(
            MailOperation::ALL.iter().all(|o| providers[0].can(*o)),
            "the seeded account can do everything"
        );
    }
}

#[test]
fn the_read_only_demo_lists_no_call_that_writes() {
    let hub = hub();
    assert!(register(&hub, Some("readonly")));
    let provider = hub.mail_providers().remove(0);
    let caps = provider.capabilities();
    assert!(!caps.operations.is_empty());
    assert!(caps.operations.iter().all(|o| READS.contains(o)), "{:?}", caps.operations);
    for write in [
        MailOperation::MarkRead,
        MailOperation::Star,
        MailOperation::Archive,
        MailOperation::Label,
        MailOperation::Move,
        MailOperation::Trash,
        MailOperation::CreateDraft,
        MailOperation::UpdateDraft,
        MailOperation::Send,
    ] {
        assert!(!provider.can(write), "{write:?} is not listed");
    }
    // A call that is not listed is refused, whatever the screen does.
    let thread = provider.search(&SearchQuery::default()).unwrap().items[0].reference.clone();
    assert!(matches!(
        provider.star(&thread, true, &Actor::person("me", "me")),
        Err(CapError::Unsupported { .. })
    ));
}

#[test]
fn a_failing_demo_answers_the_error_the_screen_has_a_state_for() {
    for (value, error, boxes_fail) in [
        ("offline", CapError::Offline, false),
        ("rate-limited", CapError::RateLimited { retry_after_ms: 30_000 }, false),
        (
            "error",
            CapError::Provider { code: "demo".into(), message: "the demo provider failed".into() },
            false,
        ),
        ("signed-out", CapError::NotSignedIn, true),
    ] {
        let hub = hub();
        assert!(register(&hub, Some(value)), "{value}");
        let provider = hub.mail_providers().remove(0);
        let boxes = provider.mailboxes();
        assert_eq!(boxes.is_err(), boxes_fail, "{value}: the mailboxes");
        assert_eq!(provider.search(&SearchQuery::default()).unwrap_err(), error, "{value}: the threads");
        let me = Actor::person("me", "me");
        let any = atelier_capabilities::mail::thread_ref("memory", ADDRESS, "1");
        assert_eq!(provider.star(&any, true, &me).unwrap_err(), error, "{value}: a change");
        assert_eq!(provider.subscribe().err(), Some(error), "{value}: the subscription");
    }
}

#[test]
fn the_seed_has_something_of_each_thing_the_screen_draws() {
    let demo = seeded();
    let boxes = demo.mailboxes().unwrap();
    let find = |role: Role| boxes.iter().find(|b| b.role == role).expect("a mailbox of each role").reference.clone();
    let all = |mailbox: &Ref| {
        let mut threads = Vec::new();
        let mut cursor = None;
        loop {
            let page = demo
                .search(&SearchQuery {
                    mailbox: Some(mailbox.clone()),
                    cursor: cursor.clone(),
                    ..SearchQuery::default()
                })
                .unwrap();
            threads.extend(page.items);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return threads;
            }
        }
    };
    let inbox = all(&find(Role::Inbox));
    assert!((10..=14).contains(&inbox.len()), "about ten threads, got {}", inbox.len());
    assert!(inbox.iter().any(|t| t.unread > 0), "some are unread");
    assert!(inbox.iter().any(|t| t.starred), "one is starred");
    assert!(inbox.iter().any(|t| t.has_attachments), "one has a file");
    assert!(inbox.iter().any(|t| t.message_count >= 3), "one has several messages");
    let long = inbox
        .iter()
        .map(|t| demo.thread(&t.reference).unwrap())
        .flat_map(|t| t.messages)
        .any(|m| m.text.chars().count() > BODY_LIMIT);
    assert!(long, "one body is long enough to be cut");
    for role in [Role::Sent, Role::Drafts, Role::Archive, Role::Trash, Role::Spam] {
        assert!(!all(&find(role)).is_empty(), "{role:?} has a thread");
    }
    assert!(boxes.iter().any(|b| b.role == Role::Custom), "a label");
    let first_page = demo.search(&SearchQuery { mailbox: Some(find(Role::Inbox)), limit: Some(PAGE), ..SearchQuery::default() }).unwrap();
    assert!(first_page.next_cursor.is_some(), "the inbox pages, so Load more shows");
    assert_eq!(MailProvider::capabilities(&Demo { inner: seeded(), mode: Mode::Normal }).limits.page_max, Some(PAGE));
}
