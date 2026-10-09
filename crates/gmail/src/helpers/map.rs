use atelier_capabilities::mail::{
    Attachment, Contact, Flags, Mailbox, MailboxKind, Message, Role, Thread, ThreadSummary,
    attachment_ref, mailbox_ref, message_ref, thread_ref,
};

use super::date::parse_display_date;
use crate::{
    structs::{Row, WireLabel, WireMessage, WireThread},
    types::PROVIDER,
};

/// A thread id is what Gmail prints: letters and digits. Anything else is not one, and is never sent to the tool.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// A label of the navigation pane as a mailbox. `gmailcli` gives the name and the unread count, no id, so the name is the id.
/// Gmail's own boxes get a role; Starred, Important and the others are `custom` but `system`.
pub(crate) fn label_mailbox(account: &str, label: &WireLabel) -> Mailbox {
    let (role, kind) = match label.name.to_lowercase().as_str() {
        "inbox" => (Role::Inbox, MailboxKind::System),
        "sent" | "sent mail" => (Role::Sent, MailboxKind::System),
        "drafts" => (Role::Drafts, MailboxKind::System),
        "trash" | "bin" => (Role::Trash, MailboxKind::System),
        "spam" | "junk" => (Role::Spam, MailboxKind::System),
        "all mail" => (Role::Archive, MailboxKind::System),
        "starred" | "snoozed" | "important" | "chats" | "scheduled" | "categories" => {
            (Role::Custom, MailboxKind::System)
        }
        _ => (Role::Custom, MailboxKind::User),
    };
    Mailbox {
        reference: mailbox_ref(PROVIDER, account, &label.name),
        name: label.name.clone(),
        role,
        kind,
        unread: label.unread,
        total: None,
        color: None,
        raw: serde_json::to_value(label).ok(),
    }
}

fn one_line(text: &str, limit: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect()
}

fn contact(address: &str, name: &str) -> Contact {
    if name.is_empty() {
        Contact::new(address)
    } else {
        Contact::named(name, address)
    }
}

fn addresses(list: &str) -> Vec<Contact> {
    list.split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(Contact::new)
        .collect()
}

fn date_of(text: &str) -> i64 {
    parse_display_date(text).unwrap_or(0)
}

/// What a search row tells of a thread. The unread count is 1 or 0 (the row says only whether the thread is unread), the
/// message count is 1 (it is at least that), and nothing says which mailboxes the thread is in.
pub(crate) fn row_summary(account: &str, row: &Row) -> ThreadSummary {
    let last_at = date_of(&row.date);
    ThreadSummary {
        reference: thread_ref(PROVIDER, account, &row.thread_id),
        subject: row.subject.clone(),
        snippet: row.snippet.clone(),
        participants: vec![contact(&row.from, &row.from_name)],
        message_count: 1,
        unread: u32::from(row.unread),
        starred: false,
        has_attachments: row.attachments,
        mailboxes: vec![],
        last_at,
        version: format!("{}|{}", u8::from(row.unread), row.date),
    }
}

fn mime_of(filename: &str) -> &'static str {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ics" => "text/calendar",
        _ => "application/octet-stream",
    }
}

fn message_of(
    account: &str,
    thread: &str,
    index: usize,
    first_file: usize,
    m: &WireMessage,
) -> Message {
    let attachments = m
        .attachments
        .iter()
        .enumerate()
        .map(|(at, name)| Attachment {
            reference: attachment_ref(PROVIDER, account, &format!("{thread}.{}", first_file + at)),
            filename: name.clone(),
            mime: mime_of(name).into(),
            size: None,
            inline: false,
        })
        .collect();
    let mut headers = std::collections::BTreeMap::new();
    if !m.date.is_empty() {
        headers.insert("date".to_string(), m.date.clone());
    }
    Message {
        reference: message_ref(PROVIDER, account, &format!("{thread}.{index}")),
        thread: thread_ref(PROVIDER, account, thread),
        from: contact(&m.from, &m.from_name),
        to: addresses(&m.to),
        cc: vec![],
        bcc: vec![],
        reply_to: None,
        subject: String::new(),
        snippet: one_line(&m.body, 120),
        text: m.body.clone(),
        html: None,
        attachments,
        date: date_of(&m.date),
        // Reading a thread through the browser marks it read in Gmail.
        flags: Flags {
            read: true,
            starred: false,
        },
        mailboxes: vec![],
        headers,
        raw: serde_json::to_value(m).ok(),
    }
}

/// A thread read through `gmailcli`. A thread that does not exist comes back from the tool as one message with no sender
/// (the text of the page it landed on), so a thread in which no message has a sender is not found.
pub(crate) fn thread_from_wire(account: &str, id: &str, wire: &WireThread) -> Option<Thread> {
    if wire.messages.iter().all(|m| m.from.is_empty()) {
        return None;
    }
    let mut first_file = 0;
    let mut messages = Vec::with_capacity(wire.messages.len());
    for (index, m) in wire.messages.iter().enumerate() {
        let mut message = message_of(account, id, index, first_file, m);
        message.subject = wire.subject.clone();
        first_file += m.attachments.len();
        messages.push(message);
    }
    let mut participants: Vec<Contact> = Vec::new();
    for c in messages
        .iter()
        .flat_map(|m| std::iter::once(&m.from).chain(&m.to))
    {
        if !participants
            .iter()
            .any(|p| p.address.eq_ignore_ascii_case(&c.address))
        {
            participants.push(c.clone());
        }
    }
    let last_at = messages.iter().map(|m| m.date).max().unwrap_or(0);
    let summary = ThreadSummary {
        reference: thread_ref(PROVIDER, account, id),
        subject: wire.subject.clone(),
        snippet: messages
            .last()
            .map(|m| m.snippet.clone())
            .unwrap_or_default(),
        participants,
        message_count: messages.len() as u32,
        unread: 0,
        starred: false,
        has_attachments: first_file > 0,
        mailboxes: vec![],
        last_at,
        version: format!("{}|{last_at}", messages.len()),
    };
    Some(Thread { summary, messages })
}

/// The file name of the `flat`th attachment of a thread, counting through its messages in order.
pub(crate) fn attachment_at(thread: &Thread, flat: usize) -> Option<String> {
    thread
        .messages
        .iter()
        .flat_map(|m| m.attachments.iter())
        .nth(flat)
        .map(|a| a.filename.clone())
}

/// The name under which `gmailcli attachments -download` writes a file: a slash or a backslash becomes a hyphen.
pub(crate) fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\'], "-")
}
