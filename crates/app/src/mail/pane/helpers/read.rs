use atelier_capabilities::{
    Actor, CapError, CapResult, Ref,
    mail::{
        Contact, Draft, DraftPatch, MailCapabilities, MailProvider, Message, NewDraft, SearchQuery,
        Thread, reply_recipients, reply_subject,
    },
};

use super::super::types::{PAGE_FALLBACK, PAGE_MOST, Problem, Reaction, THREAD_PAGES};
use crate::mail::map::{BoxRow, ThreadRow, box_rows_of, thread_row_of};

/// How the screen answers an error: offline and a wait are banners over what stays, signed out is an empty state, a call
/// that is not offered loses its control, and anything else is one line.
pub fn react(error: &CapError) -> Reaction {
    match error {
        CapError::Offline => Reaction::Raise(Problem::Offline),
        CapError::RateLimited { retry_after_ms } => Reaction::Raise(Problem::Wait(*retry_after_ms)),
        CapError::NotSignedIn => Reaction::Raise(Problem::SignedOut),
        CapError::Unsupported { .. } => Reaction::Hide,
        other => Reaction::Line(other.to_string()),
    }
}

/// How many threads a page asks for: what the provider allows, within what a screen wants.
fn page_of(caps: &MailCapabilities) -> u32 {
    caps.limits
        .page_max
        .unwrap_or(PAGE_FALLBACK)
        .clamp(1, PAGE_MOST)
}

/// What reading an account's mailboxes brings.
pub struct Boxes {
    pub caps: MailCapabilities,
    pub rows: Vec<BoxRow>,
}

/// The mailboxes of the account, and what it can do. Blocks.
pub fn read_boxes(provider: &dyn MailProvider) -> CapResult<Boxes> {
    let caps = provider.capabilities();
    let rows = box_rows_of(&provider.mailboxes()?);
    Ok(Boxes { caps, rows })
}

/// What one reading of a mailbox's threads brings: the rows, newest first, and the cursor of the page after them.
pub struct Fetched {
    pub rows: Vec<ThreadRow>,
    pub next: Option<String>,
}

fn query_of(
    caps: &MailCapabilities,
    mailbox: &Ref,
    text: &str,
    cursor: Option<String>,
) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        mailbox: Some(mailbox.clone()),
        unread: false,
        limit: Some(page_of(caps)),
        cursor,
    }
}

/// The first `pages` pages of a mailbox's threads for `text`. Blocks. `now` is in seconds, for the age of each.
pub fn read_threads(
    provider: &dyn MailProvider,
    mailbox: &Ref,
    text: &str,
    pages: usize,
    now: u64,
) -> CapResult<Fetched> {
    let caps = provider.capabilities();
    let (mut cursor, mut rows) = (None::<String>, Vec::new());
    for _ in 0..pages.clamp(1, THREAD_PAGES) {
        let page = provider.search(&query_of(&caps, mailbox, text, cursor.take()))?;
        rows.extend(
            page.items
                .iter()
                .map(|t| thread_row_of(t, provider.account(), now)),
        );
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(Fetched { rows, next: cursor })
}

/// The page that follows `cursor`. Blocks.
pub fn read_more(
    provider: &dyn MailProvider,
    mailbox: &Ref,
    text: &str,
    cursor: &str,
    now: u64,
) -> CapResult<Fetched> {
    let caps = provider.capabilities();
    let page = provider.search(&query_of(&caps, mailbox, text, Some(cursor.to_string())))?;
    Ok(Fetched {
        rows: page
            .items
            .iter()
            .map(|t| thread_row_of(t, provider.account(), now))
            .collect(),
        next: page.next_cursor,
    })
}

/// A thread with its messages, oldest first. Blocks.
pub fn read_thread(provider: &dyn MailProvider, thread: &Ref) -> CapResult<Thread> {
    provider.thread(thread)
}

/// What writing the reader's reply brings: the draft as the provider now holds it, and, when the reader asked to send, what the
/// send answered. A draft that was saved and not sent stays shown, at its new version.
pub struct Outcome {
    pub draft: Draft,
    pub sent: Option<CapResult<Message>>,
}

/// Saves `text` as a draft that answers `last`, then sends it when `send` is set. The draft is made, or changed from the version
/// held, and only that version is sent: the one this call has just saved and the box showed. Blocks.
pub fn write_draft(
    provider: &dyn MailProvider,
    by: &Actor,
    held: Option<&Draft>,
    last: &Message,
    text: &str,
    send: bool,
) -> CapResult<Outcome> {
    let draft = match held {
        // Nothing changed since the last save: that version is what the box shows.
        Some(held) if held.text == text => held.clone(),
        Some(held) => provider.update_draft(
            &held.reference,
            &DraftPatch {
                text: Some(text.to_string()),
                ..DraftPatch::default()
            },
            &held.version,
            by,
        )?,
        None => {
            let (to, cc) = reply_recipients(last, provider.account(), false);
            provider.create_draft(
                &NewDraft {
                    to,
                    cc,
                    bcc: Vec::<Contact>::new(),
                    subject: reply_subject(&last.subject),
                    text: text.to_string(),
                    in_reply_to: Some(last.reference.clone()),
                },
                by,
            )?
        }
    };
    let sent = send.then(|| provider.send(&draft.reference, &draft.version, by, None));
    Ok(Outcome { draft, sent })
}
