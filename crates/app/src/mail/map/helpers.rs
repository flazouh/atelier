use atelier_capabilities::mail::{Contact, Mailbox, Message, Role, ThreadSummary};
use atelier_ui::sidebar_model::since;

use super::{
    structs::{Attached, BoxRow, MessageView, ThreadRow},
    types::{BODY_LIMIT, LINKS_MOST},
};

/// The order of the roles in the sidebar; a custom box follows them, in the provider's order.
const ORDER: [Role; 7] = [
    Role::Inbox,
    Role::Sent,
    Role::Drafts,
    Role::Trash,
    Role::Spam,
    Role::Archive,
    Role::Custom,
];

/// The mailboxes as the sidebar lists them: inbox, sent, drafts, trash, spam, archive, then the rest.
pub fn box_rows_of(boxes: &[Mailbox]) -> Vec<BoxRow> {
    let mut rows: Vec<&Mailbox> = boxes.iter().collect();
    // A stable sort: the boxes of one role keep the provider's order.
    rows.sort_by_key(|b| ORDER.iter().position(|r| *r == b.role));
    rows.into_iter()
        .map(|b| BoxRow {
            reference: b.reference.clone(),
            name: clean_line(&b.name).into(),
            role: b.role,
            kind: b.kind,
            unread: b.unread,
        })
        .collect()
}

/// A line of text that a sender wrote, safe to draw: one line, with no control character and no bidirectional override, which
/// can make an address read as another.
pub(super) fn clean_line(text: &str) -> String {
    text.chars()
        .filter(|c| !is_override(*c))
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A body that a sender wrote, safe to draw: lines stay, a tab becomes spaces, other control characters and overrides go.
pub(super) fn clean_body(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| !is_override(*c) && (!c.is_control() || matches!(c, '\n' | '\t')))
        .collect::<String>()
        // A tab draws as whatever the font has for it; four spaces draw the same everywhere.
        .replace('\t', "    ")
}

/// The marks that change the direction of the text after them (U+202A to U+202E, U+2066 to U+2069), and the zero-width and
/// byte-order marks that hide in an address.
fn is_override(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}'..='\u{200F}' | '\u{FEFF}')
}

fn contact_words(contact: &Contact) -> String {
    match contact.name.as_deref().map(clean_line).filter(|n| !n.is_empty()) {
        Some(name) => name,
        None => clean_line(&contact.address),
    }
}

/// The people of a thread other than the account's own, as a short line: `Ana, Ben and 2 more`.
pub fn who_of(participants: &[Contact], me: &str) -> String {
    let others: Vec<&Contact> = participants
        .iter()
        .filter(|c| !c.address.eq_ignore_ascii_case(me))
        .collect();
    let named: Vec<String> = others.iter().take(3).map(|c| contact_words(c)).collect();
    match (others.len(), named.is_empty()) {
        (_, true) => clean_line(me),
        (n, _) if n > 3 => format!("{} and {} more", named.join(", "), n - 3),
        _ => named.join(", "),
    }
}

/// `summary` as a row of the middle list. `now` is in seconds, for its age.
pub fn thread_row_of(summary: &ThreadSummary, me: &str, now: u64) -> ThreadRow {
    ThreadRow {
        reference: summary.reference.clone(),
        who: who_of(&summary.participants, me).into(),
        subject: match clean_line(&summary.subject) {
            s if s.is_empty() => "(no subject)".to_string(),
            s => s,
        }
        .into(),
        snippet: clean_line(&summary.snippet).into(),
        time: since(now, u64::try_from(summary.last_at / 1000).unwrap_or(0)).into(),
        unread: summary.unread > 0,
        starred: summary.starred,
        attachment: summary.has_attachments,
        count: summary.message_count,
        version: summary.version.clone(),
    }
}

/// `message` as the reading pane draws it. `me` is the account's address.
pub fn message_view_of(message: &Message, me: &str) -> MessageView {
    let text = clean_body(&message.text);
    let list = |contacts: &[Contact]| {
        contacts
            .iter()
            .map(contact_words)
            .collect::<Vec<_>>()
            .join(", ")
    };
    MessageView {
        reference: message.reference.clone(),
        from_name: message
            .from
            .name
            .as_deref()
            .map(clean_line)
            .filter(|n| !n.is_empty())
            .map(Into::into),
        from_address: clean_line(&message.from.address).into(),
        to: format!("To {}", list(&message.to)).into(),
        cc: (!message.cc.is_empty()).then(|| format!("Cc {}", list(&message.cc)).into()),
        time: date_words(message.date).into(),
        unread: !message.flags.read,
        mine: message.from.address.eq_ignore_ascii_case(me),
        cut: cut_at(&text, BODY_LIMIT).map(Into::into),
        links: links_of(&text).into_iter().map(Into::into).collect(),
        text: text.into(),
        attachments: message
            .attachments
            .iter()
            .filter(|a| !a.inline)
            .map(|a| Attached {
                name: clean_line(&a.filename).into(),
                detail: a
                    .size
                    .map_or_else(|| clean_line(&a.mime), size_words)
                    .into(),
            })
            .collect(),
    }
}

/// The first `limit` characters of `text`, ended where a line or a word ends when one is near, or `None` when the text is not
/// longer than that.
pub(super) fn cut_at(text: &str, limit: usize) -> Option<String> {
    let (end, _) = text.char_indices().nth(limit)?;
    let head = &text[..end];
    // A break within the last tenth of the cut keeps the last word whole.
    let near = head
        .rfind(['\n', ' '])
        .filter(|at| head[*at..].chars().count() <= limit / 10);
    Some(head[..near.unwrap_or(end)].trim_end().to_string())
}

/// The web addresses in `text`, in the order they come, each once, at most [`LINKS_MOST`]. A scheme other than http and https is
/// not an address the screen opens, so it is not listed.
pub fn links_of(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '(' | ')')) {
        let lower = word.to_ascii_lowercase();
        if !(lower.starts_with("https://") || lower.starts_with("http://")) {
            continue;
        }
        let trimmed = word.trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', ']', '}']);
        // A bare scheme is no address.
        let rest = trimmed.split_once("://").map_or("", |(_, rest)| rest);
        if rest.is_empty() || found.iter().any(|f| f == trimmed) {
            continue;
        }
        found.push(trimmed.to_string());
        if found.len() == LINKS_MOST {
            break;
        }
    }
    found
}

/// `2 KB`, `1.5 MB`: one unit, and a decimal only where it says something.
pub fn size_words(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let (mut size, mut unit) = (bytes as f64, 0);
    while size >= 1024. && unit + 1 < UNITS.len() {
        size /= 1024.;
        unit += 1;
    }
    match size.fract() {
        f if f < 0.05 => format!("{size:.0} {}", UNITS[unit]),
        _ => format!("{size:.1} {}", UNITS[unit]),
    }
}

/// A message's date: `9 Oct 2026, 14:03 UTC`. The provider gives UTC and the app knows no other zone, so the zone is said.
pub fn date_words(millis: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let seconds = millis.div_euclid(1000);
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // The days since 1970-01-01 as a date of the civil calendar (Howard Hinnant's algorithm).
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{day} {} {year}, {:02}:{:02} UTC",
        MONTHS[(month - 1) as usize],
        rest / 3600,
        rest % 3600 / 60
    )
}
