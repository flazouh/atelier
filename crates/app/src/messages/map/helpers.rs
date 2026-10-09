use atelier_capabilities::{
    ActorKind,
    messaging::{Attachment, AttachmentKind, Channel, ChannelKind, Formatting, Message},
};
use atelier_ui::{project_badge::fallback_color, sidebar_model::since};

use super::{
    structs::{Chip, File, Line, Row},
    types::{Body, Group},
};

/// The first letter or digit of `name`, in capitals; `?` for a name with neither.
pub fn letter_of(name: &str) -> String {
    name.chars().find(|c| c.is_alphanumeric()).map_or_else(|| "?".to_string(), |c| c.to_uppercase().collect())
}

/// `message` as a line. `now` is in seconds, for its age. `formatting` is what the provider keeps of the markdown subset.
pub fn line_of(message: &Message, formatting: Formatting, now: u64) -> Line {
    let body = match formatting {
        Formatting::Plain => Body::Plain(message.text.clone().into()),
        Formatting::Basic | Formatting::Rich => Body::Markdown(safe_markdown(&message.text).into()),
    };
    Line {
        reference: message.reference.clone(),
        parent: message.parent.clone(),
        author: message.author.name.clone().into(),
        letter: letter_of(&message.author.name).into(),
        color: fallback_color(&message.author.id),
        agent: message.author.kind == ActorKind::Agent,
        origin: message.origin.clone().map(Into::into),
        time: since(now, u64::try_from(message.created_at / 1000).unwrap_or(0)).into(),
        created_at: message.created_at,
        edited: message.edited_at.is_some(),
        body,
        replies: message.reply_count,
        reactions: message.reactions.iter().map(|r| Chip { name: r.name.clone().into(), count: r.count, mine: r.me }).collect(),
        files: message.attachments.iter().map(file_of).collect(),
    }
}

fn file_of(attachment: &Attachment) -> File {
    let kind = match attachment.kind {
        AttachmentKind::File => "file",
        AttachmentKind::Image => "image",
        AttachmentKind::Link => "link",
        AttachmentKind::Other => "attachment",
    };
    File { name: attachment.name.clone().into(), detail: attachment.size.map_or_else(|| kind.to_string(), size_words).into() }
}

/// `2 KB`, `1.5 MB`: one unit, and a decimal only where it says something.
pub(super) fn size_words(bytes: u64) -> String {
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

/// `channel` as a row of the sidebar.
pub fn row_of(channel: &Channel) -> Row {
    let group = match channel.kind {
        ChannelKind::Public | ChannelKind::Private => Group::Channels,
        ChannelKind::Dm | ChannelKind::GroupDm => Group::Direct,
    };
    Row {
        reference: channel.reference.clone(),
        name: channel.name.clone().into(),
        kind: channel.kind,
        group,
        unread: channel.unread.is_some_and(|n| n > 0),
    }
}

/// Text that a person or a service wrote, made safe to draw as markdown: a tag is text, an image is its words, and a link
/// that does not lead to the web (a script, a file, a mention) is its words. Code is left as it is.
///
/// The markdown view draws what it is given, so what a message may do is decided here and nowhere else.
pub fn safe_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        // A line that opens and closes a fence by itself (three ticks, code, three ticks) is inline code.
        let fence = line.trim_start().starts_with("```") && line.matches("```").count() % 2 == 1;
        if fence {
            in_fence = !in_fence;
            out.push_str(line);
        } else if in_fence {
            out.push_str(line);
        } else {
            out.push_str(&safe_line(line));
        }
    }
    out
}

/// One line outside a fence: its backtick spans are code, the rest is made safe.
fn safe_line(line: &str) -> String {
    let line = match line.strip_prefix('#') {
        Some(rest) => format!("\\#{rest}"),
        None => line.to_string(),
    };
    line.split('`')
        .enumerate()
        .map(|(n, part)| if n % 2 == 0 { safe_text(part) } else { part.to_string() })
        .collect::<Vec<_>>()
        .join("`")
}

/// Text with no backtick in it: tags, images and links.
fn safe_text(text: &str) -> String {
    let escaped = text.replace('<', "\\<");
    let (mut out, mut rest) = (String::new(), escaped.as_str());
    while let Some(open) = rest.find('[') {
        let image = rest[..open].ends_with('!');
        let head = &rest[..open - usize::from(image)];
        out.push_str(head);
        match link_at(&rest[open..]) {
            Some((words, target, used)) => {
                let web = target.starts_with("https://") || target.starts_with("http://");
                match (image, web) {
                    (true, _) => out.push_str(if words.is_empty() { "image" } else { words }),
                    (false, true) => out.push_str(&rest[open..open + used]),
                    (false, false) => out.push_str(words),
                }
                rest = &rest[open + used..];
            }
            None => {
                out.push_str(&rest[open - usize::from(image)..=open]);
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The link that `s` starts with, `[words](target)`: its words, its target, and how many bytes it takes. The target ends at
/// the bracket that closes the one it opens, so `javascript:alert(1)` is one target and not a target and a stray bracket.
fn link_at(s: &str) -> Option<(&str, &str, usize)> {
    let close = s.find(']')?;
    let words = &s[1..close];
    let target = s[close + 1..].strip_prefix('(')?;
    let mut depth = 1;
    let end = target.char_indices().find_map(|(at, c)| {
        depth += match c {
            '(' => 1,
            ')' => -1,
            _ => 0,
        };
        (depth == 0).then_some(at)
    })?;
    (!words.contains('[')).then_some((words, &target[..end], close + 2 + end + 1))
}
