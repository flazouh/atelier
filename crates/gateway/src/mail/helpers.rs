use std::sync::Arc;

use atelier_capabilities::{
    Actor, CapError, Ref, Registry,
    mail::{Contact, MailOperation, MailProvider, NewDraft, RefKind, SearchQuery, kind_of},
};
use serde_json::{Value, json};

use super::render::{
    NOTICE, data, draft_json, draft_text, mailbox_line, message_json, message_text, page_json,
    summary_line,
};
use crate::{
    ToolResult,
    shared::{account, choose, clip, failed, heading, limit, neutral, optional, required},
};

/// A tool body gives its result, or the sentence the model reads when the call failed.
type Done = Result<ToolResult, String>;

/// The account a call goes to, as `provider/account`, and its provider.
struct Chosen {
    place: String,
    provider: Arc<dyn MailProvider>,
}

pub(super) fn mailboxes(registry: &Registry, args: &Value) -> Done {
    let chosen = chosen(registry, account(args)?, None)?;
    chosen.needs(MailOperation::Mailboxes, "list mailboxes")?;
    let found = chosen
        .provider
        .mailboxes()
        .map_err(|e| failed("list the mailboxes", &chosen.place, e))?;
    let head = heading(found.len(), "mailbox", "mailboxes", &chosen.place, None, 0);
    let lines: Vec<String> = found.iter().map(mailbox_line).collect();
    let text = if lines.is_empty() {
        head
    } else {
        format!("{head}\n{}", data(&lines.join("\n")))
    };
    let items = found.iter().map(neutral).collect();
    Ok(ToolResult::ok(text, page_json(items, None)))
}

pub(super) fn search(registry: &Registry, args: &Value) -> Done {
    let words = optional(args, "query");
    let mailbox = optional(args, "mailbox");
    if words.is_none() && mailbox.is_none() {
        return Err("Give a query, a mailbox, or both.".into());
    }
    let named = account(args)?;
    let mailbox = mailbox
        .map(|text| parse_ref(&text, RefKind::Mailbox, "mailbox"))
        .transpose()?;
    let from_ref = mailbox.as_ref().map(place_of);
    let chosen = chosen(registry, named, from_ref)?;
    chosen.needs(MailOperation::Search, "search mail")?;
    let query = SearchQuery {
        text: words.unwrap_or_default(),
        mailbox,
        unread: false,
        limit: Some(limit(args)),
        cursor: optional(args, "cursor"),
    };
    let found = chosen
        .provider
        .search(&query)
        .map_err(|e| failed("search the mail", &chosen.place, e))?;
    let (items, left_out) = clip(found.items);
    let head = heading(
        items.len(),
        "thread",
        "threads",
        &chosen.place,
        found.next_cursor.as_deref(),
        left_out,
    );
    let lines: Vec<String> = items.iter().map(summary_line).collect();
    let text = if lines.is_empty() {
        head
    } else {
        format!("{head}\n{}", data(&lines.join("\n")))
    };
    let structured = page_json(
        items.iter().map(neutral).collect(),
        found.next_cursor.as_deref(),
    );
    Ok(ToolResult::ok(text, structured))
}

pub(super) fn thread(registry: &Registry, args: &Value) -> Done {
    let (chosen, reference) = aimed(registry, args, "ref", RefKind::Thread)?;
    chosen.needs(MailOperation::Thread, "read a thread")?;
    let found = chosen
        .provider
        .thread(&reference)
        .map_err(|e| failed("read the thread", &chosen.place, e))?;
    let count = found.messages.len();
    let head = format!(
        "Thread {reference}, {count} message{}.",
        if count == 1 { "" } else { "s" }
    );
    let bodies: Vec<String> = found.messages.iter().map(|m| message_text(m, 0)).collect();
    let text = format!(
        "{head}\n{}",
        data(&format!(
            "Subject: {}\n\n{}",
            found.summary.subject,
            bodies.join("\n\n")
        ))
    );
    let mut thread = neutral(&found.summary);
    thread["messages"] = found.messages.iter().map(|m| message_json(m, 0)).collect();
    Ok(ToolResult::ok(
        text,
        json!({ "thread": thread, "notice": NOTICE }),
    ))
}

pub(super) fn get(registry: &Registry, args: &Value) -> Done {
    let (chosen, reference) = aimed(registry, args, "ref", RefKind::Message)?;
    chosen.needs(MailOperation::Get, "read a message")?;
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let message = chosen
        .provider
        .get(&reference)
        .map_err(|e| failed("read the message", &chosen.place, e))?;
    let text = format!(
        "Message {}.\n{}",
        message.reference,
        data(&message_text(&message, offset))
    );
    Ok(ToolResult::ok(
        text,
        json!({ "message": message_json(&message, offset), "notice": NOTICE }),
    ))
}

pub(super) fn create_draft(registry: &Registry, args: &Value, by: &Actor) -> Done {
    let named = account(args)?;
    let replying_to = optional(args, "in_reply_to")
        .map(|text| parse_ref(&text, RefKind::Message, "in_reply_to"))
        .transpose()?;
    let from_ref = replying_to.as_ref().map(place_of);
    let chosen = chosen(registry, named, from_ref)?;
    chosen.needs(MailOperation::CreateDraft, "create a draft")?;
    let to = recipients(args)?;
    let new = NewDraft {
        to,
        subject: required(args, "subject")?,
        text: required(args, "body")?,
        in_reply_to: replying_to,
        ..NewDraft::default()
    };
    let draft = chosen
        .provider
        .create_draft(&new, by)
        .map_err(|e| failed("create the draft", &chosen.place, e))?;
    let text = format!(
        "Created draft {}. Nothing was sent: the person reads the draft in Atelier and clicks Send, or does not.\n{}",
        draft.reference,
        data(&draft_text(&draft))
    );
    Ok(ToolResult::ok(
        text,
        json!({ "draft": draft_json(&draft), "notice": NOTICE }),
    ))
}

/// The addresses of `to`, each as a contact. `Ana <ana@example.com>` keeps the name.
fn recipients(args: &Value) -> Result<Vec<Contact>, String> {
    let list = args["to"]
        .as_array()
        .filter(|list| !list.is_empty())
        .ok_or("The argument to is required: a list with at least one address.")?;
    list.iter()
        .map(|entry| {
            let text = entry.as_str().map(str::trim).unwrap_or_default();
            parse_contact(text).ok_or_else(|| {
                format!("\"{text}\" is not an address. Use ana@example.com or \"Ana <ana@example.com>\".")
            })
        })
        .collect()
}

fn parse_contact(text: &str) -> Option<Contact> {
    let plain = |address: &str| {
        let ok = address.split_once('@').is_some_and(|(local, host)| {
            !local.is_empty() && host.contains('.') && !address.contains(char::is_whitespace)
        });
        ok.then(|| address.to_string())
    };
    if let Some((name, rest)) = text.rsplit_once('<')
        && let Some(address) = rest.strip_suffix('>')
    {
        let address = plain(address.trim())?;
        let name = name.trim().trim_matches('"').trim();
        return Some(if name.is_empty() {
            Contact::new(address)
        } else {
            Contact::named(name, address)
        });
    }
    plain(text).map(Contact::new)
}

fn place_of(reference: &Ref) -> (String, String) {
    (reference.provider.clone(), reference.account.clone())
}

fn chosen(
    registry: &Registry,
    named: Option<(String, String)>,
    from_ref: Option<(String, String)>,
) -> Result<Chosen, String> {
    let all = registry
        .all_mail()
        .into_iter()
        .map(|p| (p.provider().to_string(), p.account().to_string(), p))
        .collect();
    let (place, provider) = choose("mail", all, named, from_ref)?;
    Ok(Chosen { place, provider })
}

impl Chosen {
    /// The call is for an account that cannot do it: say so, and name the account.
    fn needs(&self, operation: MailOperation, feature: &str) -> Result<(), String> {
        if self.provider.can(operation) {
            Ok(())
        } else {
            Err(failed(feature, &self.place, CapError::unsupported(feature)))
        }
    }
}

/// `text` as a mail ref of the kind asked for. Mail ids hold a kind letter, so there are no bare ids.
fn parse_ref(text: &str, kind: RefKind, key: &str) -> Result<Ref, String> {
    let reference: Ref = text.parse().map_err(|_| {
        format!("{key} must be a mail ref, as an earlier call gave it. {text} is not one.")
    })?;
    if kind_of(&reference) == Some(kind) {
        Ok(reference)
    } else {
        let name = match kind {
            RefKind::Mailbox => "mailbox",
            RefKind::Thread => "thread",
            RefKind::Message => "message",
            RefKind::Draft => "draft",
            RefKind::Attachment => "attachment",
        };
        Err(format!(
            "{key} must be the ref of a {name}, and {text} is not."
        ))
    }
}

fn aimed(
    registry: &Registry,
    args: &Value,
    key: &str,
    kind: RefKind,
) -> Result<(Chosen, Ref), String> {
    let reference = parse_ref(&required(args, key)?, kind, key)?;
    let chosen = chosen(registry, account(args)?, Some(place_of(&reference)))?;
    Ok((chosen, reference))
}
