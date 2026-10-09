//! The contract every mail provider passes. A provider's crate calls [`run`] from one test with a function that makes a fresh,
//! empty provider and a way to deliver a message into it. Each check panics with a plain message when the provider breaks
//! it. A check that needs an operation the provider does not list is skipped; [`capabilities_are_honest`] proves the list.
//!
//! The checks are the ones in `docs/capabilities/mail-v1.md`, section 10.
use std::time::Duration;

use super::{
    helpers::kind_of,
    structs::{Approval, Contact, DraftPatch, Incoming, NewDraft, SearchQuery, ThreadSummary},
    traits::MailProvider,
    types::{MailEventKind, MailOperation, RefKind, Role},
};
use crate::{Actor, CapError, Ref};

/// A fresh provider with no mail, and the way to put a message in its inbox. A real provider's test delivers into a fake
/// service.
pub struct Harness {
    pub provider: Box<dyn MailProvider>,
    pub deliver: Box<dyn Fn(&Incoming) + Send + Sync>,
}

pub type Make<'a> = &'a dyn Fn() -> Harness;

const DAY: i64 = 86_400_000;
const T0: i64 = 1_760_000_000_000;

pub fn run(make: Make) {
    the_provider_reads(make);
    mailboxes_have_an_inbox(make);
    delivered_mail_is_found_read_and_ordered(make);
    pages_do_not_repeat_or_skip(make);
    unknown_and_foreign_refs(make);
    flags_change(make);
    archive_leaves_the_inbox(make);
    trash_leaves_the_default_search(make);
    label_and_move_change_the_mailboxes(make);
    drafts_are_patched_by_version(make);
    a_reply_is_a_draft(make);
    an_agent_cannot_send_alone(make);
    the_actor_is_kept(make);
    subscribe_delivers_a_new_message_once(make);
    capabilities_are_honest(make);
}

fn alex() -> Actor {
    Actor::person("alex", "Alex")
}

fn claude() -> Actor {
    Actor::agent("claude", "Claude", "alex")
}

fn me(h: &Harness) -> String {
    h.provider.whoami().expect("whoami").address
}

fn mail(h: &Harness, from: &str, subject: &str, text: &str, date: i64) -> Incoming {
    Incoming::new(from, &me(h), subject, text, date)
}

fn first_thread(h: &Harness, text: &str) -> ThreadSummary {
    let page = h.provider.search(&SearchQuery::text(text)).expect("search");
    page.items
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("search `{text}` finds the delivered message"))
}

fn mailbox(h: &Harness, role: Role) -> Ref {
    h.provider
        .mailboxes()
        .expect("mailboxes")
        .into_iter()
        .find(|m| m.role == role)
        .unwrap_or_else(|| panic!("a mailbox with the role {role:?}"))
        .reference
}

fn in_box(h: &Harness, mailbox: &Ref) -> Vec<ThreadSummary> {
    h.provider
        .search(&SearchQuery {
            mailbox: Some(mailbox.clone()),
            ..SearchQuery::default()
        })
        .expect("search a mailbox")
        .items
}

/// A provider must at least read mail.
pub fn the_provider_reads(make: Make) {
    let h = make();
    for op in [
        MailOperation::Mailboxes,
        MailOperation::Search,
        MailOperation::Thread,
        MailOperation::Get,
    ] {
        assert!(h.provider.can(op), "a provider lists {op:?}");
    }
    if h.provider.can(MailOperation::Send) {
        assert!(
            h.provider.can(MailOperation::CreateDraft),
            "a provider that sends has drafts"
        );
    }
}

/// 1. The mailboxes have an inbox. Each mailbox reference parses back.
pub fn mailboxes_have_an_inbox(make: Make) {
    let h = make();
    let boxes = h.provider.mailboxes().expect("mailboxes");
    assert!(
        boxes.iter().any(|m| m.role == Role::Inbox),
        "there is an inbox"
    );
    for m in &boxes {
        let again: Ref = m
            .reference
            .to_string()
            .parse()
            .expect("a mailbox ref parses");
        assert_eq!(again, m.reference, "a mailbox reference parses back");
        assert_eq!(kind_of(&m.reference), Some(RefKind::Mailbox), "kind b");
    }
}

/// 2. A delivered message is found, read as a thread oldest first, and `get` gives the same message.
pub fn delivered_mail_is_found_read_and_ordered(make: Make) {
    let h = make();
    (h.deliver)(&mail(&h, "bob@example.com", "Lunch", "Noon on Friday?", T0));
    let found = first_thread(&h, "lunch");
    assert_eq!(found.subject, "Lunch", "the subject");
    assert_eq!(found.message_count, 1, "one message");
    assert_eq!(found.unread, 1, "a delivered message is unread");
    let mut second = mail(&h, "alex@example.com", "Re: Lunch", "Yes, noon.", T0 + 1000);
    second.thread = Some(found.reference.clone());
    (h.deliver)(&second);
    let thread = h.provider.thread(&found.reference).expect("thread");
    assert_eq!(thread.messages.len(), 2, "two messages in the thread");
    assert_eq!(thread.summary.message_count, 2, "the summary counts both");
    assert!(
        thread.messages[0].date <= thread.messages[1].date,
        "oldest first (a tie is allowed: some services show the minute only)"
    );
    assert_eq!(thread.messages[0].text, "Noon on Friday?", "the plain text");
    assert!(thread.messages[0].date > 100_000_000_000, "milliseconds");
    let read = h
        .provider
        .get(&thread.messages[0].reference)
        .expect("get a message of the thread");
    assert_eq!(read, thread.messages[0], "get gives what the thread has");
    assert_eq!(read.thread, found.reference, "a message knows its thread");
    assert_eq!(
        h.provider
            .search(&SearchQuery::text("noon friday"))
            .expect("search")
            .items
            .len(),
        1,
        "all the words match, once per thread"
    );
}

/// 3. A list is stable across pages: no thread twice, none missed, newest first.
pub fn pages_do_not_repeat_or_skip(make: Make) {
    let h = make();
    for n in 0..5 {
        (h.deliver)(&mail(
            &h,
            "bob@example.com",
            &format!("Topic {n}"),
            "body",
            T0 + n * DAY,
        ));
    }
    let mut seen: Vec<String> = Vec::new();
    let mut cursor = None;
    for _ in 0..10 {
        let page = h
            .provider
            .search(&SearchQuery {
                limit: Some(2),
                cursor: cursor.clone(),
                ..SearchQuery::default()
            })
            .expect("a page");
        assert!(page.items.len() <= 2, "a page respects the limit");
        seen.extend(page.items.iter().map(|t| t.subject.clone()));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        seen,
        ["Topic 4", "Topic 3", "Topic 2", "Topic 1", "Topic 0"],
        "every thread once, newest first"
    );
}

/// 4. An unknown message is `NotFound`. A reference of another account or capability is `Invalid`.
pub fn unknown_and_foreign_refs(make: Make) {
    let h = make();
    let p = &h.provider;
    let own = |id: &str| -> Ref {
        format!("mail:{}:{}:{id}", p.provider(), p.account())
            .parse()
            .unwrap()
    };
    assert!(
        matches!(p.get(&own("m:nope-9")), Err(CapError::NotFound { .. })),
        "an unknown message is not found"
    );
    assert!(
        matches!(p.thread(&own("t:nope-9")), Err(CapError::NotFound { .. })),
        "an unknown thread is not found"
    );
    let other: Ref = format!("mail:{}:other@example.org:t:1", p.provider())
        .parse()
        .unwrap();
    assert!(
        matches!(p.thread(&other), Err(CapError::Invalid { .. })),
        "a thread of another account is invalid"
    );
    let task: Ref = format!("tasks:{}:{}:t:1", p.provider(), p.account())
        .parse()
        .unwrap();
    assert!(
        matches!(p.thread(&task), Err(CapError::Invalid { .. })),
        "a reference of another capability is invalid"
    );
    assert!(
        matches!(p.get(&own("t:1")), Err(CapError::Invalid { .. })),
        "a thread reference is not a message"
    );
}

/// 5. `mark_read` and `star` change the message and the thread summary.
pub fn flags_change(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::MarkRead) && !h.provider.can(MailOperation::Star) {
        return;
    }
    (h.deliver)(&mail(&h, "bob@example.com", "Flags", "text", T0));
    let thread = first_thread(&h, "flags");
    let message = h.provider.thread(&thread.reference).unwrap().messages[0]
        .reference
        .clone();
    if h.provider.can(MailOperation::MarkRead) {
        h.provider
            .mark_read(&thread.reference, true, &alex())
            .expect("mark a thread read");
        assert!(h.provider.get(&message).unwrap().flags.read, "read");
        assert_eq!(first_thread(&h, "flags").unread, 0, "no unread left");
        h.provider
            .mark_read(&message, false, &alex())
            .expect("mark a message unread");
        assert_eq!(first_thread(&h, "flags").unread, 1, "unread again");
    }
    if h.provider.can(MailOperation::Star) {
        h.provider.star(&message, true, &alex()).expect("star");
        assert!(h.provider.get(&message).unwrap().flags.starred, "starred");
        assert!(first_thread(&h, "flags").starred, "the thread shows it");
        h.provider.star(&thread.reference, false, &alex()).unwrap();
        assert!(!first_thread(&h, "flags").starred, "unstarred");
    }
}

/// 6. `archive` takes a thread out of the inbox and keeps it findable.
pub fn archive_leaves_the_inbox(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::Archive) {
        return;
    }
    (h.deliver)(&mail(&h, "bob@example.com", "Archive me", "text", T0));
    let inbox = mailbox(&h, Role::Inbox);
    let thread = first_thread(&h, "archive");
    assert_eq!(in_box(&h, &inbox).len(), 1, "it is in the inbox");
    h.provider
        .archive(&thread.reference, &alex())
        .expect("archive");
    assert!(in_box(&h, &inbox).is_empty(), "it left the inbox");
    assert_eq!(
        first_thread(&h, "archive").reference,
        thread.reference,
        "it is still found"
    );
}

/// 7. `trash` takes a thread out of the default search and into the trash.
pub fn trash_leaves_the_default_search(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::Trash) {
        return;
    }
    (h.deliver)(&mail(&h, "bob@example.com", "Bin me", "text", T0));
    let thread = first_thread(&h, "bin");
    h.provider.trash(&thread.reference, &alex()).expect("trash");
    assert!(
        h.provider
            .search(&SearchQuery::text("bin"))
            .unwrap()
            .items
            .is_empty(),
        "trash is out of the default search"
    );
    let trash = mailbox(&h, Role::Trash);
    assert_eq!(in_box(&h, &trash).len(), 1, "it is in the trash");
    let inbox = mailbox(&h, Role::Inbox);
    if h.provider.can(MailOperation::Move) {
        h.provider
            .move_to(&thread.reference, &inbox, &alex())
            .expect("move back");
        assert_eq!(in_box(&h, &inbox).len(), 1, "trash is reversible");
    }
}

/// 8. `label` and `move_to` change `mailboxes`.
pub fn label_and_move_change_the_mailboxes(make: Make) {
    let h = make();
    (h.deliver)(&mail(&h, "bob@example.com", "Labels", "text", T0));
    let thread = first_thread(&h, "labels");
    let message = h.provider.thread(&thread.reference).unwrap().messages[0]
        .reference
        .clone();
    if h.provider.can(MailOperation::Label)
        && let Some(label) = h
            .provider
            .mailboxes()
            .unwrap()
            .into_iter()
            .find(|m| m.role == Role::Custom)
    {
        let label = label.reference;
        h.provider
            .label(&message, std::slice::from_ref(&label), &[], &alex())
            .expect("label");
        assert!(h.provider.get(&message).unwrap().mailboxes.contains(&label));
        assert_eq!(in_box(&h, &label).len(), 1, "the label lists it");
        h.provider
            .label(
                &thread.reference,
                &[],
                std::slice::from_ref(&label),
                &alex(),
            )
            .expect("unlabel a thread");
        assert!(!h.provider.get(&message).unwrap().mailboxes.contains(&label));
    }
    if h.provider.can(MailOperation::Move) {
        let spam = mailbox(&h, Role::Spam);
        h.provider.move_to(&message, &spam, &alex()).expect("move");
        let boxes = h.provider.get(&message).unwrap().mailboxes;
        assert!(boxes.contains(&spam), "it is in spam");
        assert!(
            !boxes.contains(&mailbox(&h, Role::Inbox)),
            "and not in the inbox"
        );
    }
}

/// 9. A draft is created and read back, and an update changes only the fields in the patch.
pub fn drafts_are_patched_by_version(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::CreateDraft) {
        return;
    }
    let p = &h.provider;
    let made = p
        .create_draft(
            &NewDraft {
                to: vec![Contact::new("bob@example.com")],
                subject: "Plan".into(),
                text: "First text".into(),
                ..NewDraft::default()
            },
            &alex(),
        )
        .expect("create a draft");
    assert_eq!(p.draft(&made.reference).expect("read it"), made);
    assert!(
        matches!(
            p.create_draft(
                &NewDraft {
                    to: vec![Contact::new("not an address")],
                    ..NewDraft::default()
                },
                &alex()
            ),
            Err(CapError::Invalid { .. })
        ),
        "an address needs an @"
    );
    if !p.can(MailOperation::UpdateDraft) {
        return;
    }
    let after = p
        .update_draft(
            &made.reference,
            &DraftPatch {
                subject: Some("New plan".into()),
                ..DraftPatch::default()
            },
            &made.version,
            &alex(),
        )
        .expect("update");
    assert_eq!(
        (after.subject.as_str(), after.text.as_str(), after.to.len()),
        ("New plan", "First text", 1),
        "only the subject changed"
    );
    assert_ne!(after.version, made.version, "a change makes a new version");
    match p.update_draft(
        &made.reference,
        &DraftPatch::default(),
        &made.version,
        &alex(),
    ) {
        Err(CapError::Conflict { current }) => assert_eq!(
            current["subject"], "New plan",
            "a conflict carries the draft as it is"
        ),
        other => panic!("an old version is a conflict, got {other:?}"),
    }
    let same = p
        .update_draft(
            &made.reference,
            &DraftPatch::default(),
            &after.version,
            &alex(),
        )
        .expect("an empty patch");
    assert_eq!(same.version, after.version, "no change keeps the version");
}

/// 10. A reply is a draft with `Re:` once, the right recipients and `in_reply_to`. Nothing is sent.
pub fn a_reply_is_a_draft(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::Reply) {
        return;
    }
    (h.deliver)(&mail(&h, "bob@example.com", "Lunch", "Noon?", T0));
    let thread = first_thread(&h, "lunch");
    let message = h.provider.thread(&thread.reference).unwrap().messages[0]
        .reference
        .clone();
    let draft = h
        .provider
        .reply(&message, false, "Sounds good", &claude())
        .expect("reply");
    assert_eq!(draft.subject, "Re: Lunch");
    assert_eq!(draft.to, [Contact::new("bob@example.com")], "to the sender");
    assert_eq!(draft.in_reply_to.as_ref(), Some(&message));
    assert_eq!(draft.thread.as_ref(), Some(&thread.reference));
    assert_eq!(draft.text, "Sounds good");
    assert!(
        in_box(&h, &mailbox(&h, Role::Sent)).is_empty(),
        "a reply sends nothing"
    );
    assert!(
        matches!(
            h.provider.reply(&message, false, "  ", &claude()),
            Err(CapError::Invalid { .. })
        ),
        "an empty reply is invalid"
    );
    let mut again = mail(&h, "bob@example.com", "Re: Lunch", "Or 1pm?", T0 + 1000);
    again.thread = Some(thread.reference.clone());
    (h.deliver)(&again);
    let last = h
        .provider
        .thread(&thread.reference)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .reference
        .clone();
    assert_eq!(
        h.provider
            .reply(&last, false, "1pm", &claude())
            .unwrap()
            .subject,
        "Re: Lunch",
        "one Re: only"
    );
}

/// 11. An agent cannot send without the click of its person, on the version it sends. A person sends with their own call.
pub fn an_agent_cannot_send_alone(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::Send) {
        return;
    }
    let p = &h.provider;
    let sent = mailbox(&h, Role::Sent);
    let draft = p
        .create_draft(
            &NewDraft {
                to: vec![Contact::new("bob@example.com")],
                subject: "Hello".into(),
                text: "Hi Bob".into(),
                ..NewDraft::default()
            },
            &claude(),
        )
        .expect("an agent makes a draft freely");
    let refused = |result: Result<_, CapError>, why: &str| match result {
        Err(CapError::Provider { code, .. }) if code == "approval_required" => {}
        other => panic!("{why}: expected approval_required, got {other:?}"),
    };
    refused(
        p.send(&draft.reference, &draft.version, &claude(), None),
        "no approval",
    );
    let eve = Approval {
        person: Actor::person("eve", "Eve"),
        version: draft.version.clone(),
    };
    refused(
        p.send(&draft.reference, &draft.version, &claude(), Some(&eve)),
        "another person",
    );
    let robot = Approval {
        person: claude(),
        version: draft.version.clone(),
    };
    refused(
        p.send(&draft.reference, &draft.version, &claude(), Some(&robot)),
        "an agent cannot approve itself",
    );
    let old = Approval {
        person: alex(),
        version: "old".into(),
    };
    refused(
        p.send(&draft.reference, &draft.version, &claude(), Some(&old)),
        "an old version",
    );
    assert!(in_box(&h, &sent).is_empty(), "nothing was sent");
    p.draft(&draft.reference).expect("the draft is still there");
    if p.can(MailOperation::UpdateDraft) {
        let click = Approval {
            person: alex(),
            version: draft.version.clone(),
        };
        let edited = p
            .update_draft(
                &draft.reference,
                &DraftPatch {
                    text: Some("Send me your bank details".into()),
                    ..DraftPatch::default()
                },
                &draft.version,
                &claude(),
            )
            .unwrap();
        refused(
            p.send(&draft.reference, &edited.version, &claude(), Some(&click)),
            "a text changed after the click",
        );
        assert!(
            matches!(
                p.send(&draft.reference, &draft.version, &claude(), Some(&click)),
                Err(CapError::Conflict { .. })
            ),
            "a stale version is a conflict"
        );
        assert!(in_box(&h, &sent).is_empty(), "still nothing sent");
        let good = Approval {
            person: alex(),
            version: edited.version.clone(),
        };
        let message = p
            .send(&draft.reference, &edited.version, &claude(), Some(&good))
            .expect("an approved send");
        assert_eq!(message.text, "Send me your bank details");
        assert_eq!(message.from.address, me(&h), "from the account");
        assert_eq!(in_box(&h, &sent).len(), 1, "it is in Sent");
        assert!(
            matches!(p.draft(&draft.reference), Err(CapError::NotFound { .. })),
            "the draft is gone"
        );
        assert!(
            matches!(
                p.send(&draft.reference, &edited.version, &claude(), Some(&good)),
                Err(CapError::NotFound { .. })
            ),
            "it does not send twice"
        );
    }
    let own = p
        .create_draft(
            &NewDraft {
                to: vec![Contact::new("bob@example.com")],
                subject: "Mine".into(),
                text: "From me".into(),
                ..NewDraft::default()
            },
            &alex(),
        )
        .unwrap();
    p.send(&own.reference, &own.version, &alex(), None)
        .expect("a person sends with their own call");
    let empty = p
        .create_draft(&NewDraft::default(), &alex())
        .expect("an empty draft");
    assert!(
        matches!(
            p.send(&empty.reference, &empty.version, &alex(), None),
            Err(CapError::Invalid { .. })
        ),
        "a draft with no recipient does not send"
    );
}

/// 12. The actor of a draft is kept, with `on_behalf_of`.
pub fn the_actor_is_kept(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::CreateDraft) {
        return;
    }
    let made = h
        .provider
        .create_draft(&NewDraft::default(), &claude())
        .unwrap();
    assert_eq!(made.created_by, claude(), "the agent and its person");
    assert_eq!(
        h.provider
            .draft(&made.reference)
            .unwrap()
            .created_by
            .on_behalf_of,
        Some("alex".into())
    );
}

/// 13. `subscribe` delivers a new message once.
pub fn subscribe_delivers_a_new_message_once(make: Make) {
    let h = make();
    if !h.provider.can(MailOperation::Subscribe) {
        return;
    }
    let sub = h.provider.subscribe().expect("subscribe");
    (h.deliver)(&mail(&h, "bob@example.com", "News", "fresh", T0));
    let event = sub
        .recv_timeout(Duration::from_secs(10))
        .expect("an event for the new message");
    assert_eq!(event.kind, MailEventKind::NewMessage);
    assert_eq!(event.thread.subject, "News");
    assert!(
        sub.recv_timeout(Duration::from_millis(300)).is_err(),
        "the message is told once"
    );
}

/// 14. `capabilities` is honest: each operation that is not listed returns `Unsupported`.
pub fn capabilities_are_honest(make: Make) {
    let h = make();
    let p = &h.provider;
    let caps = p.capabilities();
    let reference = |kind: RefKind| -> Ref {
        format!(
            "mail:{}:{}:{}:probe",
            p.provider(),
            p.account(),
            kind.letter()
        )
        .parse()
        .unwrap()
    };
    let (thread, message, draft, mailbox, attachment) = (
        reference(RefKind::Thread),
        reference(RefKind::Message),
        reference(RefKind::Draft),
        reference(RefKind::Mailbox),
        reference(RefKind::Attachment),
    );
    for op in MailOperation::ALL {
        if caps.can(op) {
            continue;
        }
        let result = match op {
            MailOperation::Mailboxes => p.mailboxes().map(drop),
            MailOperation::Search => p.search(&SearchQuery::default()).map(drop),
            MailOperation::Thread => p.thread(&thread).map(drop),
            MailOperation::Get => p.get(&message).map(drop),
            MailOperation::MarkRead => p.mark_read(&thread, true, &alex()),
            MailOperation::Star => p.star(&thread, true, &alex()),
            MailOperation::Archive => p.archive(&thread, &alex()),
            MailOperation::Label => p.label(&thread, std::slice::from_ref(&mailbox), &[], &alex()),
            MailOperation::Move => p.move_to(&thread, &mailbox, &alex()),
            MailOperation::Trash => p.trash(&thread, &alex()),
            MailOperation::CreateDraft => p.create_draft(&NewDraft::default(), &alex()).map(drop),
            MailOperation::UpdateDraft => p
                .update_draft(&draft, &DraftPatch::default(), "1", &alex())
                .map(drop),
            MailOperation::Reply => p.reply(&message, false, "text", &alex()).map(drop),
            MailOperation::Send => p.send(&draft, "1", &alex(), None).map(drop),
            MailOperation::DownloadAttachment => p.download_attachment(&attachment).map(drop),
            MailOperation::Subscribe => p.subscribe().map(drop),
        };
        assert!(
            matches!(result, Err(CapError::Unsupported { .. })),
            "{op:?} is not listed, so it returns Unsupported, got {result:?}"
        );
    }
}
