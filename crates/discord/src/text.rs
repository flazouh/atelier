//! Text between the markdown subset of the spec and what Discord shows.
//!
//! Discord reads almost the same subset (`**bold**`, `*italic*`, `~~strike~~`, code, `> quote`, `-` lists). The
//! differences are three. A message from a person does not show `[text](url)` as a link, so it goes out as `text (url)`.
//! A mention is `<@id>` in Discord and a link to a reference in the subset. And a text must never ping a crowd.
use atelier_capabilities::Ref;

use crate::helpers::DM;

/// A zero-width space. After the `@` of `@everyone`, it keeps Discord from reading a mention and changes nothing a
/// person sees.
const ZWSP: char = '\u{200b}';

fn is_id(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// `[label](target)` at the start of `chars`: the label, the target and how many characters it took.
fn link_at(chars: &[char]) -> Option<(String, String, usize)> {
    let close = chars.iter().position(|c| *c == ']')?;
    if chars.get(close + 1) != Some(&'(') || close == 0 {
        return None;
    }
    let end = chars[close + 2..].iter().position(|c| *c == ')')?;
    let label: String = chars[1..close].iter().collect();
    let target: String = chars[close + 2..close + 2 + end].iter().collect();
    if label.contains('\n') || target.contains(char::is_whitespace) || target.is_empty() {
        return None;
    }
    Some((label, target, close + 2 + end + 1))
}

/// A link of the subset, in Discord's words. A reference to a person or a channel of Discord becomes a mention, a
/// reference to a message becomes its address, and a web address stays beside its label.
fn link_to_discord(label: &str, target: &str) -> Option<String> {
    if let Ok(r) = target.parse::<Ref>() {
        if r.capability != "messaging" || r.provider != "discord" {
            return None;
        }
        if let Some(user) = r.id.strip_prefix("user/") {
            return is_id(user).then(|| format!("<@{user}>"));
        }
        return match r.id.split_once(':') {
            None if is_id(&r.id) => Some(format!("<#{}>", r.id)),
            Some((channel, message)) if is_id(channel) && is_id(message) => {
                let server = if r.account == DM { "@me" } else { &r.account };
                Some(format!(
                    "{label} (https://discord.com/channels/{server}/{channel}/{message})"
                ))
            }
            _ => None,
        };
    }
    if target.starts_with("https://") || target.starts_with("http://") {
        let clean: String = target.chars().filter(|c| !matches!(c, '<' | '>')).collect();
        return Some(if label == target {
            clean
        } else {
            format!("{label} ({clean})")
        });
    }
    None
}

/// Whether `rest` starts with `@everyone` or `@here`, in any case. Discord may read either anywhere in a word, so the
/// check does not look at what comes before or after.
fn starts_broadcast(rest: &[char]) -> bool {
    let word = |w: &str| {
        let w: Vec<char> = w.chars().collect();
        rest.len() > w.len()
            && rest[1..=w.len()]
                .iter()
                .zip(&w)
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    };
    rest.first() == Some(&'@') && (word("everyone") || word("here"))
}

fn convert(text: &str, out: &mut String) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let rest = &chars[i..];
        if rest[0] == '`'
            && let Some(end) = rest[1..].iter().position(|c| *c == '`')
        {
            // Inside code Discord shows the text as it is and sends no mention, so the code stays as written.
            out.extend(&rest[..end + 2]);
            i += end + 2;
            continue;
        }
        if rest[0] == '['
            && let Some((label, target, len)) = link_at(rest)
            && let Some(text) = link_to_discord(&label, &target)
        {
            out.push_str(&text);
            i += len;
            continue;
        }
        if starts_broadcast(rest) {
            out.push('@');
            out.push(ZWSP);
        } else if rest[0] == '<' && rest.get(1) == Some(&'@') {
            // `<@id>`, `<@!id>` and `<@&role>` are mentions. A mention goes out only as a link to a reference.
            out.push('<');
            out.push(ZWSP);
        } else {
            out.push(rest[0]);
        }
        i += 1;
    }
}

/// Turns the markdown subset of the spec into the text to send to Discord. A text that its sender wrote cannot ping
/// `@everyone`, `@here` or a role, and cannot mention a person unless it holds a link to that person's reference.
/// Fenced code is copied as it is. (`discordcli` also tells Discord to parse no mention, so this is the second lock.)
pub fn to_discord(text: &str) -> String {
    let parts: Vec<&str> = text.split("```").collect();
    let mut out = String::new();
    for (n, part) in parts.iter().enumerate() {
        if n > 0 {
            out.push_str("```");
        }
        // A fence that never closes is not code: Discord shows it as plain text.
        if n % 2 == 1 && n + 1 < parts.len() {
            out.push_str(part);
        } else {
            convert(part, &mut out);
        }
    }
    out
}

/// What a `<...>` of Discord text means in the subset, if it is one we know. `account` is the account of the channel the
/// text came from.
fn angle_to_subset(inner: &str, account: &str) -> Option<String> {
    if let Some(id) = inner.strip_prefix("@&") {
        return is_id(id).then(|| "@role".to_string());
    }
    if let Some(id) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
        return is_id(id).then(|| format!("[@{id}](messaging:discord:{account}:user/{id})"));
    }
    if let Some(id) = inner.strip_prefix('#') {
        return is_id(id).then(|| format!("[#{id}](messaging:discord:{account}:{id})"));
    }
    // A custom emoji: `<:name:id>` or `<a:name:id>`.
    let emoji = inner
        .strip_prefix("a:")
        .or_else(|| inner.strip_prefix(':'))?;
    let (name, id) = emoji.split_once(':')?;
    (is_id(id) && !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .then(|| format!(":{name}:"))
}

/// Turns the text of a Discord message into the markdown subset: a mention becomes a link to a reference, a custom
/// emoji becomes `:name:`, and `<https://x>` becomes the address. The rest is kept as it is.
pub fn from_discord(text: &str, account: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<'
            && let Some(len) = chars[i + 1..].iter().take(80).position(|c| *c == '>')
        {
            let inner: String = chars[i + 1..i + 1 + len].iter().collect();
            let known = angle_to_subset(&inner, account).or_else(|| {
                (inner.starts_with("https://") || inner.starts_with("http://"))
                    .then(|| inner.clone())
                    .filter(|u| !u.contains(char::is_whitespace))
            });
            if let Some(known) = known {
                out.push_str(&known);
                i += len + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}
