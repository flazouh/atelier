//! A seeded `MemoryMail` for checking the Mail screen before a real mail account is connected. It exists only in a debug build,
//! and only when `ATELIER_DEMO_MAIL` is set: a release build does not have this module, and a debug build without the variable
//! registers nothing.
//!
//! - `1` (or `true`, `yes`): the seeded account, which can do everything.
//! - `readonly`: the same mail, from a provider that lists no call that writes, as Gmail does today.
//! - `offline`, `rate-limited`, `error`, `signed-out`: the seeded account failing in the ways the screen has an answer for.
use atelier_capabilities::{
    Actor, CapError, CapResult, Ref, Subscription,
    mail::{
        Account, Approval, Contact, Draft, DraftPatch, Incoming, MailCapabilities, MailEvent,
        MailOperation, MailProvider, MemoryMail, Message, Mailbox, NewDraft, SearchQuery, Thread,
        ThreadSummary, mailbox_ref,
    },
    tasks::Page,
};

use crate::capability_hub::CapabilityHub;

/// The environment variable that asks for the demo account.
pub const VARIABLE: &str = "ATELIER_DEMO_MAIL";

/// The address of the demo account.
pub const ADDRESS: &str = "demo@example.com";

/// How many threads a page of the demo holds, so the first mailbox has a "Load more".
const PAGE: u32 = 6;

/// The calls that only read. A provider that lists no other is read-only, as Gmail is.
const READS: [MailOperation; 6] = [
    MailOperation::Mailboxes,
    MailOperation::Search,
    MailOperation::Thread,
    MailOperation::Get,
    MailOperation::DownloadAttachment,
    MailOperation::Subscribe,
];

/// How the demo account behaves.
#[derive(Clone, Debug)]
enum Mode {
    Normal,
    ReadOnly,
    /// Every call but the listing of mailboxes (unless `boxes_too`) answers `error`.
    Failing { error: CapError, boxes_too: bool },
}

/// What a value of the variable asks for.
fn asked(value: Option<&str>) -> Option<Demo> {
    let failing = |error: CapError, boxes_too: bool| Mode::Failing { error, boxes_too };
    let mode = match value.map(str::trim)? {
        "1" | "true" | "yes" => Mode::Normal,
        "readonly" => Mode::ReadOnly,
        // The mailboxes are listed and then the connection drops.
        "offline" => failing(CapError::Offline, false),
        "rate-limited" => failing(
            CapError::RateLimited {
                retry_after_ms: 30_000,
            },
            false,
        ),
        "error" => failing(
            CapError::Provider {
                code: "demo".into(),
                message: "the demo provider failed".into(),
            },
            false,
        ),
        "signed-out" => failing(CapError::NotSignedIn, true),
        _ => return None,
    };
    Some(Demo {
        inner: seeded(),
        mode,
    })
}

/// Registers the demo account in `hub` when `value` asks for it. `true` when it did.
pub fn register(hub: &CapabilityHub, value: Option<&str>) -> bool {
    match asked(value) {
        Some(demo) => {
            hub.add_mail(std::sync::Arc::new(demo));
            true
        }
        None => false,
    }
}

/// [`register`] with the variable as the environment has it.
pub fn register_from_env(hub: &CapabilityHub) -> bool {
    register(hub, std::env::var(VARIABLE).ok().as_deref())
}

/// The seeded account behaving as `mode` says.
struct Demo {
    inner: MemoryMail,
    mode: Mode,
}

/// What a call is, for the gate of a mode.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Boxes,
    Read,
    Write,
}

impl Demo {
    fn gate(&self, kind: Kind) -> CapResult<()> {
        match (&self.mode, kind) {
            (Mode::ReadOnly, Kind::Write) => Err(CapError::unsupported("write mail")),
            (Mode::Failing { error, boxes_too }, Kind::Boxes) if *boxes_too => Err(error.clone()),
            (Mode::Failing { error, .. }, Kind::Read | Kind::Write) => Err(error.clone()),
            _ => Ok(()),
        }
    }
}

impl MailProvider for Demo {
    fn provider(&self) -> &str {
        self.inner.provider()
    }

    fn account(&self) -> &str {
        self.inner.account()
    }

    fn capabilities(&self) -> MailCapabilities {
        let mut caps = self.inner.capabilities();
        caps.limits.page_max = Some(PAGE);
        if matches!(self.mode, Mode::ReadOnly) {
            caps.operations.retain(|o| READS.contains(o));
        }
        caps
    }

    fn whoami(&self) -> CapResult<Account> {
        self.inner.whoami()
    }

    fn mailboxes(&self) -> CapResult<Vec<Mailbox>> {
        self.gate(Kind::Boxes)?;
        self.inner.mailboxes()
    }

    fn search(&self, query: &SearchQuery) -> CapResult<Page<ThreadSummary>> {
        self.gate(Kind::Read)?;
        self.inner.search(query)
    }

    fn thread(&self, thread: &Ref) -> CapResult<Thread> {
        self.gate(Kind::Read)?;
        self.inner.thread(thread)
    }

    fn get(&self, message: &Ref) -> CapResult<Message> {
        self.gate(Kind::Read)?;
        self.inner.get(message)
    }

    fn draft(&self, draft: &Ref) -> CapResult<Draft> {
        self.gate(Kind::Read)?;
        self.inner.draft(draft)
    }

    fn mark_read(&self, target: &Ref, read: bool, by: &Actor) -> CapResult<()> {
        self.gate(Kind::Write)?;
        self.inner.mark_read(target, read, by)
    }

    fn star(&self, target: &Ref, starred: bool, by: &Actor) -> CapResult<()> {
        self.gate(Kind::Write)?;
        self.inner.star(target, starred, by)
    }

    fn archive(&self, target: &Ref, by: &Actor) -> CapResult<()> {
        self.gate(Kind::Write)?;
        self.inner.archive(target, by)
    }

    fn label(&self, target: &Ref, add: &[Ref], remove: &[Ref], by: &Actor) -> CapResult<()> {
        self.gate(Kind::Write)?;
        self.inner.label(target, add, remove, by)
    }

    fn move_to(&self, target: &Ref, mailbox: &Ref, by: &Actor) -> CapResult<()> {
        self.gate(Kind::Write)?;
        self.inner.move_to(target, mailbox, by)
    }

    fn trash(&self, target: &Ref, by: &Actor) -> CapResult<()> {
        self.gate(Kind::Write)?;
        self.inner.trash(target, by)
    }

    fn create_draft(&self, new: &NewDraft, by: &Actor) -> CapResult<Draft> {
        self.gate(Kind::Write)?;
        self.inner.create_draft(new, by)
    }

    fn update_draft(
        &self,
        draft: &Ref,
        patch: &DraftPatch,
        version: &str,
        by: &Actor,
    ) -> CapResult<Draft> {
        self.gate(Kind::Write)?;
        self.inner.update_draft(draft, patch, version, by)
    }

    fn send(
        &self,
        draft: &Ref,
        version: &str,
        by: &Actor,
        approval: Option<&Approval>,
    ) -> CapResult<Message> {
        self.gate(Kind::Write)?;
        self.inner.send(draft, version, by, approval)
    }

    fn download_attachment(&self, attachment: &Ref) -> CapResult<Vec<u8>> {
        self.gate(Kind::Read)?;
        self.inner.download_attachment(attachment)
    }

    fn subscribe(&self) -> CapResult<Subscription<MailEvent>> {
        self.gate(Kind::Read)?;
        self.inner.subscribe()
    }
}

/// A body long enough to be cut: a mailing list's digest, with two addresses in it.
fn digest() -> String {
    let mut text = String::from(
        "Issue 41. Read it online at https://example.com/digest?issue=41 or leave the list at https://example.com/unsubscribe.\n\n",
    );
    for n in 1..=60 {
        text.push_str(&format!(
            "{n}. The gateway gives each agent session one door to the app's tools. Mail, tasks and messages come through it, \
             and a tool that writes asks the person first. Read item {n} of this digest for the details of how that works.\n\n"
        ));
    }
    text
}

/// An account of about ten threads in the inbox, with a sent box, a draft, an archive, a trash, a spam box and one label: a
/// thread with three messages, an unread one, a starred one, one with a file, one with a body long enough to be cut, one with
/// markup in it that must stay text, and links. The page holds six, so the inbox has a "Load more".
pub fn seeded() -> MemoryMail {
    const HOUR: i64 = 3_600_000;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let me = Actor::person("demo", "Demo");
    let mail = MemoryMail::new(ADDRESS);
    let mut at = now - 50 * HOUR;
    let mut step = |hours: i64| {
        at += hours * HOUR;
        at
    };
    let got = |from: Contact, subject: &str, text: &str, date: i64| Incoming {
        from,
        ..Incoming::new("", ADDRESS, subject, text, date)
    };
    let ana = || Contact::named("Ana Costa", "ana@example.com");
    let read = |message: &Message| {
        mail.mark_read(&message.reference, true, &me).expect("a flag")
    };

    // An old thread of three messages, the last one unread.
    let first = mail
        .receive(&got(
            ana(),
            "Release 0.1.6 is cut",
            "The release branch is cut.\nCan you run the smoke tests before Friday?\n\nAna",
            step(1),
        ))
        .expect("a message");
    read(&first);
    let reply = mail
        .receive(&Incoming {
            from: Contact::new(ADDRESS),
            to: vec![ana()],
            thread: Some(first.thread.clone()),
            ..got(Contact::new(ADDRESS), "Re: Release 0.1.6 is cut", "Yes, on it. I will report tonight.", step(1))
        })
        .expect("a message");
    read(&reply);

    mail.receive(&got(
        Contact::named("Carol Diaz", "carol@example.com"),
        "Lunch on Friday?",
        "A few of us are going to the noodle place at noon. Join us?",
        step(5),
    ))
    .expect("a message");

    let invoice = mail
        .receive(&Incoming {
            attachments: vec![("invoice-2026-09.pdf".into(), "application/pdf".into(), vec![0; 184_320])],
            ..got(
                Contact::named("Billing", "billing@example.com"),
                "Your invoice for September",
                "Your invoice is attached. The amount is due in 14 days.\nQuestions? Reply to this message.",
                step(6),
            )
        })
        .expect("a message");
    read(&invoice);
    let receipts = mailbox_ref("memory", ADDRESS, "label-receipts");
    mail.label(&invoice.thread, &[receipts], &[], &me).expect("a label");

    let ben = mail
        .receive(&got(
            Contact::named("Ben Okafor", "ben@example.com"),
            "Design review notes",
            "Notes from the review:\n- the sidebar spacing is right at every zoom\n- the thread list needs a clearer unread mark\n- ship it after Ana's pass",
            step(8),
        ))
        .expect("a message");
    mail.star(&ben.thread, true, &me).expect("a star");

    let markup = mail
        .receive(&got(
            Contact::named("Security", "security@example.net"),
            "Account notice <b>please read</b>",
            "<b>Dear user</b>, <a href=\"https://example.net/login\">sign in</a> now.\n<script>alert(1)</script>\n[a link](javascript:alert(1)) and ![an image](https://example.net/pixel.gif)",
            step(4),
        ))
        .expect("a message");
    read(&markup);

    mail.receive(&got(
        Contact::named("Weekly Digest", "digest@example.org"),
        "The weekly digest, issue 41",
        &digest(),
        step(3),
    ))
    .expect("a message");

    let dan = mail
        .receive(&got(
            Contact::named("Dan Evers", "dan@example.com"),
            "Quarterly planning",
            "I put the three options in the doc. Which one do you prefer?",
            step(12),
        ))
        .expect("a message");
    read(&dan);
    let more = mail
        .receive(&Incoming {
            thread: Some(dan.thread.clone()),
            ..got(Contact::named("Dan Evers", "dan@example.com"), "Re: Quarterly planning", "Never mind, I decided. Option two.", step(1))
        })
        .expect("a message");
    read(&more);

    let pull = mail
        .receive(&got(
            Contact::named("GitHub", "notifications@example.com"),
            "[atelier] Pull request 455 is ready for review",
            "Ana asked for your review.\nView it: https://example.com/atelier/pull/455\nYou are receiving this because you were asked.",
            step(2),
        ))
        .expect("a message");
    read(&pull);

    let eve = mail
        .receive(&got(
            Contact::named("Eve Lindqvist", "eve@example.com"),
            "Re: the contract",
            "The signed copy is in the shared folder. Thanks for the quick turn.",
            step(5),
        ))
        .expect("a message");
    read(&eve);
    mail.star(&eve.thread, true, &me).expect("a star");

    // The first thread gets its third message, which is unread.
    mail.receive(&Incoming {
        thread: Some(first.thread.clone()),
        ..got(ana(), "Re: Release 0.1.6 is cut", "Also: please run the Linux build, it failed on my side.", step(0))
    })
    .expect("a message");

    let welcome = mail
        .receive(&got(
            Contact::named("The Atelier team", "hello@example.com"),
            "Welcome to Atelier",
            "Thanks for trying Atelier. Mail, tasks and chat live in one window.",
            step(1),
        ))
        .expect("a message");
    read(&welcome);

    // The archive, the trash and the spam box have a thread each, so every mailbox has something to open.
    let boxed = |role: &str, from: &str, subject: &str, text: &str| {
        let message = mail
            .receive(&got(Contact::new(from), subject, text, now - 300 * HOUR))
            .expect("a message");
        mail.mark_read(&message.reference, true, &me).expect("a flag");
        mail.move_to(&message.thread, &mailbox_ref("memory", ADDRESS, role), &me)
            .expect("a move");
    };
    boxed("archive", "ops@example.com", "Last year's report", "The report is done and filed.");
    boxed("trash", "promo@example.com", "A sale you did not ask for", "Everything is half price today only.");
    boxed("spam", "prize@example.net", "You won", "Click to claim your prize at https://example.net/claim");

    // What the account sent, and a draft it has not.
    let sent = |to: Contact, subject: &str, text: &str, date: i64| {
        let message = mail
            .receive(&Incoming {
                from: Contact::new(ADDRESS),
                to: vec![to],
                ..got(Contact::new(ADDRESS), subject, text, date)
            })
            .expect("a message");
        mail.mark_read(&message.reference, true, &me).expect("a flag");
        mail.move_to(&message.thread, &mailbox_ref("memory", ADDRESS, "sent"), &me)
            .expect("a move");
    };
    sent(Contact::named("Ben Okafor", "ben@example.com"), "Re: Design review notes", "Thanks. I will send my pass on Thursday.", now - 20 * HOUR);
    sent(Contact::named("Dan Evers", "dan@example.com"), "Quarterly planning", "Option two works for me.", now - 30 * HOUR);
    let draft = mail
        .receive(&Incoming {
            from: Contact::new(ADDRESS),
            to: vec![ana()],
            ..got(Contact::new(ADDRESS), "Smoke test results", "Draft: all green except the Linux build, which I am still looking at.", now - 2 * HOUR)
        })
        .expect("a message");
    mail.mark_read(&draft.reference, true, &me).expect("a flag");
    mail.move_to(&draft.thread, &mailbox_ref("memory", ADDRESS, "drafts"), &me)
        .expect("a move");
    mail
}

#[cfg(test)]
mod tests;
