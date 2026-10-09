use std::{sync::Arc, time::Duration};

use atelier_capabilities::{
    CapError, Ref,
    mail::{
        Contact, Incoming, MailEventKind, MailOperation, MailProvider, MailboxKind, Role,
        SearchQuery,
        contract::{self, Harness},
        mailbox_ref,
    },
};

use super::fake::{FakeGmail, display};
use crate::{GmailMail, RunFailure, Runner};

const ME: &str = "alex@gmail.com";
const T0: i64 = 1_760_000_000_000;

fn gmail(fake: &Arc<FakeGmail>) -> GmailMail {
    GmailMail::new(ME, fake.clone()).with_poll_interval(Duration::from_millis(20))
}

fn setup() -> (Arc<FakeGmail>, GmailMail) {
    let fake = Arc::new(FakeGmail::new(ME));
    let provider = gmail(&fake);
    (fake, provider)
}

fn mail(subject: &str, text: &str, date: i64) -> Incoming {
    Incoming::new("bob@example.com", ME, subject, text, date)
}

#[test]
fn the_gmail_provider_passes_the_contract_over_a_fake_runner() {
    contract::run(&|| {
        let (fake, provider) = setup();
        Harness {
            provider: Box::new(provider),
            deliver: Box::new(move |incoming| fake.deliver(incoming)),
        }
    });
}

#[test]
fn the_capabilities_list_only_what_gmailcli_can_do() {
    let (_, p) = setup();
    let caps = p.capabilities();
    assert_eq!(
        caps.operations,
        [
            MailOperation::Mailboxes,
            MailOperation::Search,
            MailOperation::Thread,
            MailOperation::Get,
            MailOperation::Subscribe,
            MailOperation::DownloadAttachment,
        ]
    );
    assert_eq!(serde_json::to_value(caps.search_syntax).unwrap(), "gmail");
    assert_eq!(caps.limits.page_max, Some(50));
    let no_files = GmailMail::new(ME, Arc::new(FakeGmail::new(ME).without_files()));
    assert!(!no_files.can(MailOperation::DownloadAttachment));
    let probe: Ref = format!("mail:gmail:{ME}:a:1.0").parse().unwrap();
    assert!(matches!(
        no_files.download_attachment(&probe),
        Err(CapError::Unsupported { .. })
    ));
}

#[test]
fn a_search_asks_for_one_row_more_than_the_page_and_pages_by_offset() {
    let (fake, p) = setup();
    for n in 0..5 {
        fake.deliver(&mail(&format!("Topic {n}"), "body", T0 + n * 86_400_000));
    }
    let page = p
        .search(&SearchQuery {
            text: "body".into(),
            limit: Some(2),
            cursor: Some("2".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!(
        fake.calls().last().unwrap(),
        &["search", "body", "-n", "5", "-json"]
    );
    assert_eq!(
        page.items
            .iter()
            .map(|t| t.subject.as_str())
            .collect::<Vec<_>>(),
        ["Topic 2", "Topic 1"]
    );
    assert_eq!(page.next_cursor.as_deref(), Some("4"));
    let last = p
        .search(&SearchQuery {
            text: "body".into(),
            limit: Some(2),
            cursor: Some("4".into()),
            ..SearchQuery::default()
        })
        .unwrap();
    assert_eq!((last.items.len(), last.next_cursor), (1, None));
    assert!(matches!(
        p.search(&SearchQuery {
            cursor: Some("x".into()),
            ..SearchQuery::default()
        }),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn nothing_beyond_the_visible_page_is_asked_for() {
    let (fake, p) = setup();
    p.search(&SearchQuery {
        limit: Some(100),
        cursor: Some("40".into()),
        ..SearchQuery::default()
    })
    .unwrap();
    assert_eq!(
        fake.calls().last().unwrap()[3],
        "50",
        "the page is 50 threads"
    );
}

#[test]
fn a_search_summary_says_what_a_row_knows_and_no_more() {
    let (fake, p) = setup();
    let mut incoming = Incoming::new("bob@example.com", ME, "Invoice", "See file.", T0);
    incoming.from = Contact::named("Bob", "bob@example.com");
    incoming.attachments = vec![("inv.pdf".into(), "application/pdf".into(), vec![1])];
    fake.deliver(&incoming);
    let t = &p.search(&SearchQuery::text("invoice")).unwrap().items[0];
    assert_eq!(
        t.reference
            .to_string()
            .split(':')
            .take(4)
            .collect::<Vec<_>>()
            .join(":"),
        "mail:gmail:alex@gmail.com:t"
    );
    assert_eq!(t.participants, [Contact::named("Bob", "bob@example.com")]);
    assert_eq!((t.message_count, t.unread, t.starred), (1, 1, false));
    assert!(t.has_attachments && t.mailboxes.is_empty());
    assert_eq!(t.last_at, T0 - T0 % 60_000, "the minute Gmail shows");
}

#[test]
fn a_thread_reads_every_message_with_its_files_and_marks_nothing_it_does_not_know() {
    let (fake, p) = setup();
    let mut first = mail("Files", "First.", T0);
    first.attachments = vec![("a.pdf".into(), "application/pdf".into(), b"AAA".to_vec())];
    fake.deliver(&first);
    let found = p.search(&SearchQuery::text("files")).unwrap().items[0].clone();
    let mut second = mail("Re: Files", "Second.", T0 + 120_000);
    second.thread = Some(found.reference.clone());
    second.attachments = vec![
        ("b.png".into(), "image/png".into(), b"BBB".to_vec()),
        ("c/d.txt".into(), "text/plain".into(), b"CCC".to_vec()),
    ];
    fake.deliver(&second);
    let thread = p.thread(&found.reference).unwrap();
    assert_eq!(thread.summary.subject, "Files");
    assert_eq!(thread.summary.message_count, 2);
    assert_eq!(thread.summary.last_at, thread.messages[1].date);
    let m = &thread.messages[1];
    assert_eq!(
        m.subject, "Files",
        "a message has the subject of its thread"
    );
    assert_eq!(
        m.reference.id,
        format!("m:{}.1", found.reference.id.trim_start_matches("t:"))
    );
    assert_eq!(m.headers["date"], display(T0 + 120_000));
    assert!(m.flags.read && !m.flags.starred && m.mailboxes.is_empty());
    assert_eq!(m.to, [Contact::new(ME)]);
    assert_eq!(
        m.attachments
            .iter()
            .map(|a| (a.filename.as_str(), a.mime.as_str()))
            .collect::<Vec<_>>(),
        [("b.png", "image/png"), ("c/d.txt", "text/plain")]
    );
    assert!(
        m.attachments[0].reference.id.ends_with(".1"),
        "files count through the thread"
    );
    assert_eq!(
        p.download_attachment(&thread.messages[0].attachments[0].reference)
            .unwrap(),
        b"AAA"
    );
    assert_eq!(
        p.download_attachment(&m.attachments[1].reference).unwrap(),
        b"CCC"
    );
    assert!(
        matches!(
            p.download_attachment(
                &format!(
                    "mail:gmail:{ME}:a:{}.9",
                    found.reference.id.trim_start_matches("t:")
                )
                .parse()
                .unwrap()
            ),
            Err(CapError::NotFound { .. })
        ),
        "there is no tenth file"
    );
    assert_eq!(fake.files_left(), 0, "the scratch directory is removed");
}

#[test]
fn an_id_that_is_not_a_thread_is_not_found_and_never_sent_to_the_tool() {
    let (fake, p) = setup();
    for id in ["../etc/passwd", "a b", "t;rm -rf", ""] {
        let r: Ref = format!("mail:gmail:{ME}:t:{id}").parse().unwrap();
        assert!(
            matches!(p.thread(&r), Err(CapError::NotFound { .. })),
            "{id}"
        );
    }
    assert!(fake.calls().is_empty());
    let page: Ref = format!("mail:gmail:{ME}:t:1a0b3ab523a5d891")
        .parse()
        .unwrap();
    assert!(
        matches!(p.thread(&page), Err(CapError::NotFound { .. })),
        "the junk the tool prints for an unknown id is not a thread"
    );
    let bad: Ref = format!("mail:gmail:{ME}:m:nodot").parse().unwrap();
    assert!(matches!(p.get(&bad), Err(CapError::NotFound { .. })));
    let past: Ref = format!("mail:gmail:{ME}:m:1a0b3ab523a5d891.4")
        .parse()
        .unwrap();
    fake.deliver(&mail("One", "x", T0));
    assert!(matches!(p.get(&past), Err(CapError::NotFound { .. })));
}

#[test]
fn the_mailboxes_are_the_labels_with_roles() {
    let (_, p) = setup();
    let boxes = p.mailboxes().unwrap();
    let find = |name: &str| boxes.iter().find(|m| m.name == name).unwrap();
    assert_eq!(find("Inbox").role, Role::Inbox);
    assert_eq!(find("Inbox").unread, 1);
    assert_eq!(find("All Mail").role, Role::Archive);
    assert_eq!(
        (find("Starred").role, find("Starred").kind),
        (Role::Custom, MailboxKind::System)
    );
    let work = find("Work/Projects");
    assert_eq!((work.role, work.kind), (Role::Custom, MailboxKind::User));
    assert_eq!(work.reference, mailbox_ref("gmail", ME, "Work/Projects"));
    assert!(work.raw.is_some());
}

#[test]
fn a_search_of_a_mailbox_or_unread_is_a_gmail_query() {
    let (fake, p) = setup();
    let ask = |query: SearchQuery| {
        p.search(&query).unwrap();
        fake.calls().last().unwrap()[1].clone()
    };
    assert_eq!(
        ask(SearchQuery::default()),
        "in:anywhere -in:trash -in:spam"
    );
    assert_eq!(
        ask(SearchQuery::text("-from:bob")),
        "in:anywhere -in:trash -in:spam -from:bob",
        "gmailcli would read a leading dash as a flag"
    );
    assert_eq!(
        ask(SearchQuery {
            text: "from:bob".into(),
            unread: true,
            mailbox: Some(mailbox_ref("gmail", ME, "Work/Projects")),
            ..SearchQuery::default()
        }),
        "is:unread label:work-projects from:bob"
    );
    assert_eq!(
        ask(SearchQuery {
            mailbox: Some(mailbox_ref("gmail", ME, "Trash")),
            ..SearchQuery::default()
        }),
        "in:trash"
    );
    assert!(matches!(
        p.search(&SearchQuery {
            mailbox: Some(mailbox_ref("gmail", "other@gmail.com", "Inbox")),
            ..SearchQuery::default()
        }),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn a_failure_of_the_tool_is_the_error_the_screen_knows() {
    let (fake, p) = setup();
    let fail = |text: &str| {
        fake.fail_with(Some(FakeGmail::exit(text)));
        p.search(&SearchQuery::text("x")).unwrap_err()
    };
    assert_eq!(
        fail(
            "error: the browser is not signed in to Gmail. Open mail.google.com in the browser, sign in, then retry"
        ),
        CapError::NotSignedIn
    );
    assert!(matches!(
        fail("error: the browser call failed: 429 Too Many Requests"),
        CapError::RateLimited {
            retry_after_ms: 60_000
        }
    ));
    assert_eq!(
        fail("ssh: Could not resolve hostname mac: Name or service not known"),
        CapError::Offline
    );
    assert!(matches!(
        fail("error: Gmail's page layout did not match what this tool expects."),
        CapError::Provider { code, .. } if code == "layout_changed"
    ));
    fake.fail_with(None);
    assert!(p.search(&SearchQuery::text("x")).is_ok());
}

struct Canned(String);

impl Runner for Canned {
    fn run(&self, _args: &[String]) -> Result<String, RunFailure> {
        Ok(self.0.clone())
    }
}

#[test]
fn output_that_is_not_json_is_a_provider_error() {
    let p = GmailMail::new(ME, Arc::new(Canned("Inbox (3) Compose".into())));
    assert!(matches!(
        p.mailboxes(),
        Err(CapError::Provider { code, .. }) if code == "bad_output"
    ));
}

#[test]
fn whoami_checks_the_account_the_browser_is_signed_in_to() {
    let (_, p) = setup();
    let account = p.whoami().unwrap();
    assert_eq!(account.address, ME);
    assert_eq!(
        account.reference.to_string(),
        "mail:gmail:alex@gmail.com:account"
    );
    let other = GmailMail::new("someone@else.com", Arc::new(FakeGmail::new(ME)));
    assert!(matches!(
        other.whoami(),
        Err(CapError::Provider { code, .. }) if code == "account_mismatch"
    ));
    let signed_out = GmailMail::new(ME, Arc::new(Canned(r#"{"email":"","unread":0}"#.into())));
    assert_eq!(signed_out.whoami().unwrap_err(), CapError::NotSignedIn);
}

#[test]
fn connect_learns_the_address_from_the_browser() {
    let fake = Arc::new(FakeGmail::new("Someone@Gmail.com"));
    let p = GmailMail::connect(fake).unwrap();
    assert_eq!(p.account(), "Someone@Gmail.com");
    assert_eq!(p.whoami().unwrap().address, "Someone@Gmail.com");
    let signed_out = Arc::new(Canned(r#"{"email":"","unread":0}"#.into()));
    assert_eq!(
        GmailMail::connect(signed_out).unwrap_err(),
        CapError::NotSignedIn
    );
}

#[test]
fn a_reference_matches_the_account_without_regard_to_case() {
    let (fake, p) = setup();
    fake.deliver(&mail("Case", "x", T0));
    let id = p.search(&SearchQuery::text("case")).unwrap().items[0]
        .reference
        .id
        .clone();
    let loud: Ref = format!("mail:gmail:ALEX@gmail.com:{id}").parse().unwrap();
    assert!(p.thread(&loud).is_ok());
}

#[test]
fn the_poll_tells_new_mail_once_and_not_the_mail_that_was_there() {
    let (fake, p) = setup();
    fake.deliver(&mail("Old", "x", T0));
    let sub = p.subscribe().unwrap();
    assert!(
        sub.recv_timeout(Duration::from_millis(150)).is_err(),
        "mail that was there before is not news"
    );
    fake.deliver(&mail("Fresh", "y", T0 + 600_000));
    let event = sub.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        (event.kind, event.thread.subject.as_str()),
        (MailEventKind::NewMessage, "Fresh")
    );
    let old = p.search(&SearchQuery::text("old")).unwrap().items[0]
        .reference
        .clone();
    let mut more = mail("Re: Old", "z", T0 + 1_200_000);
    more.thread = Some(old);
    fake.deliver(&more);
    let event = sub.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        (event.kind, event.thread.subject.as_str()),
        (MailEventKind::NewMessage, "Old")
    );
    assert!(sub.recv_timeout(Duration::from_millis(150)).is_err());
}

#[test]
fn a_failed_poll_is_skipped_and_the_next_one_goes_on() {
    let (fake, p) = setup();
    let sub = p.subscribe().unwrap();
    fake.fail_with(Some(FakeGmail::exit(
        "error: the browser did not answer within 3m0s",
    )));
    std::thread::sleep(Duration::from_millis(80));
    fake.fail_with(None);
    fake.deliver(&mail("Late", "x", T0));
    assert_eq!(
        sub.recv_timeout(Duration::from_secs(5))
            .unwrap()
            .thread
            .subject,
        "Late"
    );
}

#[test]
fn dropping_the_subscription_ends_the_poll() {
    let (fake, p) = setup();
    let sub = p.subscribe().unwrap();
    std::thread::sleep(Duration::from_millis(80));
    drop(sub);
    std::thread::sleep(Duration::from_millis(100));
    let before = fake.calls().len();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(fake.calls().len(), before, "no call after the stop");
}
