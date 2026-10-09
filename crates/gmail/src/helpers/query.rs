use atelier_capabilities::mail::{SearchQuery, local_id};

/// The Gmail search text for one of Gmail's own mailboxes, or for a label. Gmail writes a label's name in lower case with
/// hyphens for spaces and slashes.
fn mailbox_term(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "inbox" => "in:inbox".into(),
        "sent" | "sent mail" => "in:sent".into(),
        "drafts" => "in:drafts".into(),
        "trash" | "bin" => "in:trash".into(),
        "spam" | "junk" => "in:spam".into(),
        "all mail" => "in:anywhere".into(),
        "starred" => "is:starred".into(),
        "important" => "is:important".into(),
        other => format!(
            "label:{}",
            other
                .split(|c: char| c.is_whitespace() || c == '/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        ),
    }
}

/// What Gmail's own default search shows: everything but trash and spam. It opens with a positive term, because `gmailcli`
/// reads a query that starts with a dash as one of its own flags.
const EVERYTHING: &str = "in:anywhere -in:trash -in:spam";

/// What `gmailcli search` takes. `gmailcli` refuses an empty query, so a search with nothing in it asks for what Gmail's own
/// default search shows. A query that would start with a dash gets the same opening.
pub(crate) fn build_query(query: &SearchQuery) -> String {
    let mut parts: Vec<String> = Vec::new();
    if query.unread {
        parts.push("is:unread".into());
    }
    if let Some(mailbox) = &query.mailbox {
        parts.push(mailbox_term(local_id(mailbox)));
    }
    let text = query.text.trim();
    if !text.is_empty() {
        parts.push(text.to_string());
    }
    if parts.first().is_none_or(|first| first.starts_with('-')) {
        parts.insert(0, EVERYTHING.into());
    }
    parts.join(" ")
}
