use super::{
    structs::{Contact, Message},
    types::RefKind,
};
use crate::{Actor, ActorKind, CapError, CapResult, Ref};

fn make(kind: RefKind, provider: &str, account: &str, id: &str) -> Ref {
    Ref {
        capability: "mail".into(),
        provider: provider.into(),
        account: account.into(),
        id: format!("{}:{id}", kind.letter()),
    }
}

pub fn mailbox_ref(provider: &str, account: &str, id: &str) -> Ref {
    make(RefKind::Mailbox, provider, account, id)
}

pub fn thread_ref(provider: &str, account: &str, id: &str) -> Ref {
    make(RefKind::Thread, provider, account, id)
}

pub fn message_ref(provider: &str, account: &str, id: &str) -> Ref {
    make(RefKind::Message, provider, account, id)
}

pub fn draft_ref(provider: &str, account: &str, id: &str) -> Ref {
    make(RefKind::Draft, provider, account, id)
}

pub fn attachment_ref(provider: &str, account: &str, id: &str) -> Ref {
    make(RefKind::Attachment, provider, account, id)
}

/// What a mail reference points at, or `None` for a reference of another capability or one with no kind letter.
pub fn kind_of(reference: &Ref) -> Option<RefKind> {
    if reference.capability != "mail" {
        return None;
    }
    let mut chars = reference.id.chars();
    let letter = chars.next()?;
    (chars.next() == Some(':'))
        .then(|| RefKind::from_letter(letter))
        .flatten()
}

/// The id without its kind letter: `1a0b.2` for `m:1a0b.2`.
pub fn local_id(reference: &Ref) -> &str {
    match reference.id.split_once(':') {
        Some((_, rest)) => rest,
        None => &reference.id,
    }
}

/// Puts text from a sender inside the fence that tells an agent it is data and not an instruction. A `<untrusted` or
/// `</untrusted` inside the text is broken, so the sender cannot close the fence or open a second one.
pub fn fence(source: &str, text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut safe = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(found) = lower[at..].find('<') {
        let start = at + found;
        safe.push_str(&text[at..start]);
        let rest = &lower[start..];
        if rest.starts_with("<untrusted") || rest.starts_with("</untrusted") {
            safe.push_str("&lt;");
        } else {
            safe.push('<');
        }
        at = start + 1;
    }
    safe.push_str(&text[at..]);
    format!(
        "<untrusted source=\"{}\">{safe}</untrusted>",
        source.replace('"', "&quot;")
    )
}

/// `Re: subject`, with one `Re:` only.
pub fn reply_subject(subject: &str) -> String {
    let trimmed = subject.trim();
    if trimmed.to_ascii_lowercase().starts_with("re:") {
        trimmed.to_string()
    } else {
        format!("Re: {trimmed}")
    }
}

fn same_address(a: &Contact, b: &Contact) -> bool {
    a.address.eq_ignore_ascii_case(&b.address)
}

fn push_new(list: &mut Vec<Contact>, contact: &Contact, me: &str) {
    if !contact.address.eq_ignore_ascii_case(me) && !list.iter().any(|c| same_address(c, contact)) {
        list.push(contact.clone());
    }
}

/// Who a reply goes to, as `(to, cc)`. A reply goes to `reply_to`, or to the sender. A reply to a message that `me` sent goes
/// to its recipients. With `all`, the other recipients come too. `me` is never a recipient.
pub fn reply_recipients(message: &Message, me: &str, all: bool) -> (Vec<Contact>, Vec<Contact>) {
    let mut to = Vec::new();
    let mut cc = Vec::new();
    if message.from.address.eq_ignore_ascii_case(me) {
        for c in &message.to {
            push_new(&mut to, c, me);
        }
    } else {
        push_new(
            &mut to,
            message.reply_to.as_ref().unwrap_or(&message.from),
            me,
        );
    }
    if all {
        for c in &message.to {
            push_new(&mut to, c, me);
        }
        for c in &message.cc {
            if !to.iter().any(|t| same_address(t, c)) {
                push_new(&mut cc, c, me);
            }
        }
    }
    (to, cc)
}

/// The rule of section 8.2 of the spec. A person sends by clicking, so the call needs no approval. An agent needs the click of
/// the person it works for, on the version of the draft that is being sent. Every provider calls this before it sends.
pub fn check_send_approval(
    by: &Actor,
    draft_version: &str,
    approval: Option<&super::structs::Approval>,
) -> CapResult<()> {
    if by.kind == ActorKind::Person {
        return Ok(());
    }
    let refuse = |message: &str| CapError::Provider {
        code: "approval_required".into(),
        message: message.into(),
    };
    let Some(approval) = approval else {
        return Err(refuse("a person must click Send"));
    };
    if approval.person.kind != ActorKind::Person
        || by.on_behalf_of.as_deref() != Some(approval.person.id.as_str())
    {
        return Err(refuse(
            "the click is not from the person the agent works for",
        ));
    }
    if approval.version != draft_version {
        return Err(refuse("the draft changed after the click"));
    }
    Ok(())
}

/// The plain text of an HTML body. It drops scripts, styles and comments, turns block tags into line breaks, removes the other
/// tags and decodes the common entities.
pub fn html_to_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut at = 0;
    while at < html.len() {
        let rest = &html[at..];
        if !rest.starts_with('<') {
            let Some(c) = rest.chars().next() else { break };
            out.push(c);
            at += c.len_utf8();
            continue;
        }
        if lower[at..].starts_with("<!--") {
            at = lower[at..].find("-->").map_or(html.len(), |p| at + p + 3);
            continue;
        }
        let name: String = lower[at + 1..]
            .trim_start_matches('/')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if name.is_empty() {
            out.push('<');
            at += 1;
            continue;
        }
        let tag_end = rest.find('>').map_or(html.len(), |p| at + p + 1);
        if (name == "script" || name == "style") && !lower[at + 1..].starts_with('/') {
            let close = format!("</{name}");
            let after = lower[at..].find(&close).map_or(html.len(), |p| at + p);
            at = lower[after..]
                .find('>')
                .map_or(html.len(), |p| after + p + 1);
            continue;
        }
        if matches!(
            name.as_str(),
            "br" | "p"
                | "div"
                | "li"
                | "tr"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "blockquote"
        ) {
            out.push('\n');
        }
        at = tag_end;
    }
    tidy(&decode(&out))
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let decoded = tail
            .find(';')
            .filter(|end| *end <= 9)
            .and_then(|end| entity(&tail[1..end]).map(|c| (c, end + 1)));
        match decoded {
            Some((c, used)) => {
                out.push(c);
                rest = &tail[used..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// Trims each line, and keeps at most one blank line in a row.
fn tidy(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}
