//! Tests against the real mailbox. They are ignored, and they only read. Set `ATELIER_GMAIL_COMMAND` to the tool:
//! `local:<path>` or `ssh:<host>:<path>` (the Mac that holds the browser login). Run with
//! `cargo test -p atelier-gmail -- --ignored`.
//!
//! Reading a thread opens it in the browser, and Gmail then marks it read. So `a_thread_can_be_read` runs only when
//! `ATELIER_GMAIL_READ_THREAD=1` is set too.
use std::sync::Arc;

use atelier_capabilities::mail::{MailProvider, Role, SearchQuery};

use crate::{CliRunner, GmailMail};

fn live() -> Option<GmailMail> {
    let spec = std::env::var("ATELIER_GMAIL_COMMAND").ok()?;
    let runner = match spec.split_once(':')? {
        ("local", path) => CliRunner::local(path),
        ("ssh", rest) => {
            let (host, path) = rest.split_once(':')?;
            CliRunner::over_ssh(host, path)
        }
        _ => return None,
    };
    Some(GmailMail::connect(Arc::new(runner)).expect("connect to the signed-in Gmail"))
}

fn newest(p: &GmailMail, limit: u32) -> Vec<atelier_capabilities::mail::ThreadSummary> {
    p.search(&SearchQuery {
        text: "in:inbox".into(),
        limit: Some(limit),
        ..SearchQuery::default()
    })
    .unwrap()
    .items
}

#[test]
#[ignore = "needs a signed-in browser behind gmailcli"]
fn the_account_and_the_mailboxes_are_read() {
    let Some(p) = live() else { return };
    assert_eq!(p.whoami().unwrap().address, p.account());
    let boxes = p.mailboxes().unwrap();
    assert!(boxes.iter().any(|m| m.role == Role::Inbox), "{boxes:?}");
}

#[test]
#[ignore = "needs a signed-in browser behind gmailcli"]
fn the_inbox_is_searched() {
    let Some(p) = live() else { return };
    let items = newest(&p, 3);
    assert!(items.len() <= 3);
    assert!(items.iter().all(|t| !t.reference.id.is_empty()));
}

#[test]
#[ignore = "marks the thread read in Gmail; also needs ATELIER_GMAIL_READ_THREAD=1"]
fn a_thread_can_be_read() {
    let Some(p) = live() else { return };
    if std::env::var("ATELIER_GMAIL_READ_THREAD").as_deref() != Ok("1") {
        return;
    }
    let first = newest(&p, 1)
        .into_iter()
        .next()
        .expect("a thread in the inbox");
    assert!(!p.thread(&first.reference).unwrap().messages.is_empty());
}
