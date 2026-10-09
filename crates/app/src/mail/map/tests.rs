use atelier_capabilities::mail::{
    Attachment, Contact, Flags, Mailbox, MailboxKind, Message, Role, ThreadSummary, attachment_ref,
    mailbox_ref, message_ref, thread_ref,
};

use super::*;

fn mailbox(id: &str, name: &str, role: Role) -> Mailbox {
    Mailbox {
        reference: mailbox_ref("memory", "me@x.test", id),
        name: name.into(),
        role,
        kind: MailboxKind::System,
        unread: 0,
        total: None,
        color: None,
        raw: None,
    }
}

fn message(text: &str) -> Message {
    Message {
        reference: message_ref("memory", "me@x.test", "1"),
        thread: thread_ref("memory", "me@x.test", "1"),
        from: Contact::named("Ana", "ana@x.test"),
        to: vec![Contact::new("me@x.test")],
        cc: vec![],
        bcc: vec![],
        reply_to: None,
        subject: "Hello".into(),
        snippet: String::new(),
        text: text.into(),
        html: None,
        attachments: vec![],
        date: 0,
        flags: Flags::default(),
        mailboxes: vec![],
        headers: Default::default(),
        raw: None,
    }
}

fn summary(subject: &str, participants: Vec<Contact>) -> ThreadSummary {
    ThreadSummary {
        reference: thread_ref("memory", "me@x.test", "1"),
        subject: subject.into(),
        snippet: "  the\n snippet ".into(),
        participants,
        message_count: 3,
        unread: 1,
        starred: true,
        has_attachments: true,
        mailboxes: vec![],
        last_at: 0,
        version: "7".into(),
    }
}

#[test]
fn the_sidebar_lists_the_roles_in_a_fixed_order_and_the_rest_as_the_provider_has_them() {
    let rows = box_rows_of(&[
        mailbox("z", "Zeta", Role::Custom),
        mailbox("a", "Archive", Role::Archive),
        mailbox("s", "Sent", Role::Sent),
        mailbox("i", "Inbox", Role::Inbox),
        mailbox("y", "Alpha", Role::Custom),
        mailbox("t", "Trash", Role::Trash),
        mailbox("d", "Drafts", Role::Drafts),
        mailbox("p", "Spam", Role::Spam),
    ]);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "Inbox", "Sent", "Drafts", "Trash", "Spam", "Archive", "Zeta", "Alpha"
        ]
    );
}

#[test]
fn a_row_names_the_others_and_never_the_account() {
    let people = vec![
        Contact::named("Me", "me@x.test"),
        Contact::named("Ana", "ana@x.test"),
        Contact::new("ben@x.test"),
    ];
    assert_eq!(who_of(&people, "ME@x.test"), "Ana, ben@x.test");
    assert_eq!(
        who_of(&[Contact::new("me@x.test")], "me@x.test"),
        "me@x.test",
        "a note to oneself names oneself"
    );
    let many: Vec<Contact> = (0..6)
        .map(|n| Contact::named(format!("P{n}"), format!("p{n}@x.test")))
        .collect();
    assert_eq!(who_of(&many, "me@x.test"), "P0, P1, P2 and 3 more");
}

#[test]
fn a_thread_row_holds_what_the_list_draws() {
    let row = thread_row_of(
        &summary(
            "Re:   plan\u{202E}\u{0007}",
            vec![Contact::named("Ana", "ana@x.test")],
        ),
        "me@x.test",
        0,
    );
    assert_eq!(
        row.subject, "Re: plan",
        "one line, with no override and no control character"
    );
    assert_eq!(row.snippet, "the snippet");
    assert!(row.unread && row.starred && row.attachment);
    assert_eq!(row.count, 3);
    assert_eq!(row.version, "7");
    let none = thread_row_of(&summary("   ", vec![]), "me@x.test", 0);
    assert_eq!(none.subject, "(no subject)");
}

#[test]
fn a_body_stays_text_and_loses_what_can_make_it_lie() {
    let text = "<b>bold</b> & <script>x()</script>\r\nsecond\u{202E}line\u{0000}";
    let view = message_view_of(&message(text), "me@x.test");
    assert_eq!(
        view.text, "<b>bold</b> & <script>x()</script>\nsecondline",
        "tags stay as the characters they are; only lines, tabs and printable text remain"
    );
    assert_eq!(view.cut, None);
}

#[test]
fn a_long_body_is_cut_at_the_limit_with_the_whole_kept() {
    let word = "word ";
    let text = word.repeat(2_000);
    let view = message_view_of(&message(&text), "me@x.test");
    let cut = view.cut.expect("a body of 10000 characters is cut");
    assert!(cut.chars().count() <= BODY_LIMIT);
    assert!(cut.ends_with("word"), "the last word is whole: {cut:?}");
    assert_eq!(
        view.text.chars().count(),
        text.chars().count(),
        "the whole is kept for Show all"
    );
    assert_eq!(cut_at("short", BODY_LIMIT), None);
    assert_eq!(
        cut_at("héllo wörld", 4).as_deref(),
        Some("héll"),
        "a cut falls on a character, not a byte"
    );
}

#[test]
fn the_links_of_a_body_are_listed_as_text_and_only_the_web_ones() {
    let text = "see https://example.com/a?b=1, and (http://x.test/p). \
                Not javascript:alert(1) nor ftp://x.test nor https:// alone. \
                Again https://example.com/a?b=1 and <https://y.test/q>";
    assert_eq!(
        links_of(text),
        [
            "https://example.com/a?b=1",
            "http://x.test/p",
            "https://y.test/q"
        ]
    );
    let many = (0..20)
        .map(|n| format!("https://x.test/{n} "))
        .collect::<String>();
    assert_eq!(links_of(&many).len(), LINKS_MOST);
}

#[test]
fn a_message_view_says_who_wrote_to_whom_and_what_it_carries() {
    let mut m = message("hi");
    m.cc = vec![Contact::new("cc@x.test")];
    m.flags = Flags {
        read: false,
        starred: false,
    };
    m.attachments = vec![
        Attachment {
            reference: attachment_ref("memory", "me@x.test", "1.0"),
            filename: "plan.pdf".into(),
            mime: "application/pdf".into(),
            size: Some(2048),
            inline: false,
        },
        Attachment {
            reference: attachment_ref("memory", "me@x.test", "1.1"),
            filename: "logo.png".into(),
            mime: "image/png".into(),
            size: None,
            inline: true,
        },
        Attachment {
            reference: attachment_ref("memory", "me@x.test", "1.2"),
            filename: "notes".into(),
            mime: "text/plain".into(),
            size: None,
            inline: false,
        },
    ];
    let view = message_view_of(&m, "me@x.test");
    assert_eq!(view.from_name.as_deref(), Some("Ana"));
    assert_eq!(view.from_address, "ana@x.test");
    assert_eq!(view.to, "To me@x.test");
    assert_eq!(view.cc.as_deref(), Some("Cc cc@x.test"));
    assert!(view.unread && !view.mine);
    let files: Vec<(&str, &str)> = view
        .attachments
        .iter()
        .map(|a| (a.name.as_ref(), a.detail.as_ref()))
        .collect();
    assert_eq!(
        files,
        [("plan.pdf", "2 KB"), ("notes", "text/plain")],
        "an inline image is part of the body, not a chip"
    );
}

#[test]
fn sizes_and_dates_read_as_words() {
    assert_eq!(size_words(512), "512 B");
    assert_eq!(size_words(1536), "1.5 KB");
    assert_eq!(size_words(5 * 1024 * 1024), "5 MB");
    assert_eq!(date_words(0), "1 Jan 1970, 00:00 UTC");
    // 2026-10-09 14:03:00 UTC
    assert_eq!(date_words(1_791_554_580_000), "9 Oct 2026, 14:03 UTC");
    // A leap day.
    assert_eq!(
        date_words(951_782_400_000 + 3_600_000),
        "29 Feb 2000, 01:00 UTC"
    );
    assert_eq!(date_words(-1000), "31 Dec 1969, 23:59 UTC");
}

#[test]
fn cleaning_keeps_words_and_drops_the_rest() {
    assert_eq!(clean_line("a\tb\nc  d"), "a b c d");
    assert_eq!(clean_body("a\tb\r\nc\u{200B}d"), "a    b\ncd");
}
