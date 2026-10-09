use std::sync::Arc;

use atelier_capabilities::{
    Actor, CapError, Ref, Registry,
    messaging::{ChannelQuery, MessagingProvider, NewMessage, Operation, SearchQuery},
};
use serde_json::{Value, json};

use super::render::{NOTICE, channel_line, data, message_json, message_text, page_json};
use crate::{
    ToolResult,
    shared::{account, choose, clip, failed, heading, limit, neutral, optional, required},
};

/// A tool body gives its result, or the sentence the model reads when the call failed.
type Done = Result<ToolResult, String>;

/// The account a call goes to, as `provider/account`, and its provider.
struct Chosen {
    place: String,
    provider: Arc<dyn MessagingProvider>,
}

/// A ref the call names, with the account it belongs to.
struct Aimed {
    chosen: Chosen,
    reference: Ref,
}

pub(super) fn channels(registry: &Registry, args: &Value) -> Done {
    let chosen = chosen(registry, account(args)?, None)?;
    chosen.needs(Operation::Channels, "list channels")?;
    let query = ChannelQuery {
        text: optional(args, "text"),
        limit: Some(limit(args)),
        cursor: optional(args, "cursor"),
        ..ChannelQuery::default()
    };
    let found = chosen
        .provider
        .channels(&query)
        .map_err(|e| failed("list the channels", &chosen.place, e))?;
    let (items, left_out) = clip(found.items);
    let head = heading(
        items.len(),
        "channel",
        "channels",
        &chosen.place,
        found.next_cursor.as_deref(),
        left_out,
    );
    let lines: Vec<String> = items.iter().map(channel_line).collect();
    let structured = page_json(
        items.iter().map(neutral).collect(),
        found.next_cursor.as_deref(),
    );
    Ok(ToolResult::ok(
        with_lines(head, &lines.join("\n")),
        structured,
    ))
}

pub(super) fn history(registry: &Registry, args: &Value) -> Done {
    let aimed = aimed(registry, args, "channel")?;
    let Aimed { chosen, reference } = &aimed;
    chosen.needs(Operation::History, "read the history of a channel")?;
    let found = chosen
        .provider
        .history(
            reference,
            optional(args, "cursor").as_deref(),
            Some(limit(args)),
        )
        .map_err(|e| failed("read the history", &chosen.place, e))?;
    Ok(messages_page(found.items, found.next_cursor, &chosen.place))
}

pub(super) fn thread(registry: &Registry, args: &Value) -> Done {
    let aimed = aimed(registry, args, "message")?;
    let Aimed { chosen, reference } = &aimed;
    chosen.needs(Operation::Thread, "read a thread")?;
    let found = chosen
        .provider
        .thread(reference, optional(args, "cursor").as_deref())
        .map_err(|e| failed("read the thread", &chosen.place, e))?;
    Ok(messages_page(found.items, found.next_cursor, &chosen.place))
}

pub(super) fn search(registry: &Registry, args: &Value) -> Done {
    let words = required(args, "query")?;
    // `channel` is optional here, and a ref in it also tells which account to search.
    let (chosen, channel) = match optional(args, "channel") {
        Some(_) => {
            let Aimed { chosen, reference } = aimed(registry, args, "channel")?;
            (chosen, Some(reference))
        }
        None => (self::chosen(registry, account(args)?, None)?, None),
    };
    chosen.needs(Operation::Search, "search messages")?;
    let query = SearchQuery {
        text: words,
        channel,
        from: None,
        limit: Some(limit(args)),
        cursor: optional(args, "cursor"),
    };
    let found = chosen
        .provider
        .search(&query)
        .map_err(|e| failed("search the messages", &chosen.place, e))?;
    Ok(messages_page(found.items, found.next_cursor, &chosen.place))
}

pub(super) fn send(registry: &Registry, args: &Value, by: &Actor) -> Done {
    let Aimed { chosen, reference } = aimed(registry, args, "channel")?;
    chosen.needs(Operation::Send, "send messages")?;
    let text = required(args, "text")?;
    let in_thread_of = optional(args, "in_thread_of")
        .map(|given| ref_in(&chosen, &given, "in_thread_of"))
        .transpose()?;
    let new = NewMessage {
        channel: reference.clone(),
        text,
        in_thread_of,
    };
    let sent = chosen
        .provider
        .send(&new, by)
        .map_err(|e| failed("send the message", &chosen.place, e))?;
    let head = match &new.in_thread_of {
        Some(root) => format!(
            "Sent {} as a reply in the thread of {root}.",
            sent.reference
        ),
        None => format!("Sent {} to {reference}.", sent.reference),
    };
    let text = format!("{head}\n{}", data(&message_text(&sent)));
    Ok(ToolResult::ok(
        text,
        json!({ "message": message_json(&sent), "notice": NOTICE }),
    ))
}

/// A page of messages: the data, and the text to read.
fn messages_page(
    items: Vec<atelier_capabilities::messaging::Message>,
    next_cursor: Option<String>,
    place: &str,
) -> ToolResult {
    let (items, left_out) = clip(items);
    let head = heading(
        items.len(),
        "message",
        "messages",
        place,
        next_cursor.as_deref(),
        left_out,
    );
    let body: Vec<String> = items.iter().map(message_text).collect();
    let structured = page_json(
        items.iter().map(message_json).collect(),
        next_cursor.as_deref(),
    );
    ToolResult::ok(with_lines(head, &body.join("\n\n")), structured)
}

/// The heading, then the lines inside the data markers. An empty page has no block.
fn with_lines(head: String, lines: &str) -> String {
    if lines.is_empty() {
        head
    } else {
        format!("{head}\n{}", data(lines))
    }
}

fn chosen(
    registry: &Registry,
    named: Option<(String, String)>,
    from_ref: Option<(String, String)>,
) -> Result<Chosen, String> {
    let all = registry
        .all_messaging()
        .into_iter()
        .map(|p| (p.provider().to_string(), p.account().to_string(), p))
        .collect();
    let (place, provider) = choose("messaging", all, named, from_ref)?;
    Ok(Chosen { place, provider })
}

impl Chosen {
    /// The call is for an account that cannot do it: say so, and name the account.
    fn needs(&self, operation: Operation, feature: &str) -> Result<(), String> {
        if self.provider.can(operation) {
            Ok(())
        } else {
            Err(failed(feature, &self.place, CapError::unsupported(feature)))
        }
    }
}

/// The ref in argument `key` and the account it is in. A bare id works when the account is clear from the call.
fn aimed(registry: &Registry, args: &Value, key: &str) -> Result<Aimed, String> {
    let text = required(args, key)?;
    let named = account(args)?;
    if let Ok(reference) = text.parse::<Ref>() {
        if reference.capability != "messaging" {
            return Err(format!("{text} is not a messaging reference."));
        }
        let from_ref = Some((reference.provider.clone(), reference.account.clone()));
        let chosen = chosen(registry, named, from_ref)?;
        return Ok(Aimed { chosen, reference });
    }
    let chosen = chosen(registry, named, None)?;
    let reference = ref_in(&chosen, &text, key)?;
    Ok(Aimed { chosen, reference })
}

/// `text` as a ref of the account `chosen`: a full ref must be of that account, a bare id is made into one.
fn ref_in(chosen: &Chosen, text: &str, key: &str) -> Result<Ref, String> {
    if let Ok(reference) = text.parse::<Ref>() {
        let same = reference.capability == "messaging"
            && reference.provider == chosen.provider.provider()
            && reference.account == chosen.provider.account();
        return if same {
            Ok(reference)
        } else {
            Err(format!(
                "{key} must be a ref of {}, and {text} is not.",
                chosen.place
            ))
        };
    }
    Ref::new(
        "messaging",
        chosen.provider.provider(),
        chosen.provider.account(),
        text,
    )
    .map_err(|_| format!("{text} is not a messaging reference. Use the ref a call gave you."))
}
