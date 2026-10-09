use std::time::{SystemTime, UNIX_EPOCH};

use atelier_capabilities::{CapError, Ref};

/// Where a Slack link points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permalink {
    pub channel: String,
    pub ts: String,
    /// The root of the thread, when the link names one.
    pub thread_ts: Option<String>,
}

fn is_channel_id(text: &str) -> bool {
    text.len() >= 2
        && matches!(text.as_bytes()[0], b'C' | b'D' | b'G')
        && text
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// `1760000000.000100` from the `p1760000000000100` of a link.
fn ts_from_link_part(text: &str) -> Option<String> {
    let digits = text.strip_prefix('p')?;
    if digits.len() == 16 && digits.bytes().all(|b| b.is_ascii_digit()) {
        Some(format!("{}.{}", &digits[..10], &digits[10..]))
    } else {
        None
    }
}

/// Whether `text` is a Slack message id: ten digits, a dot, six digits.
pub fn is_ts(text: &str) -> bool {
    match text.split_once('.') {
        Some((secs, micro)) => {
            secs.len() == 10
                && micro.len() == 6
                && secs
                    .bytes()
                    .chain(micro.bytes())
                    .all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

/// Reads `https://app.slack.com/client/T01/C01/p1760000000000100?thread_ts=1760000000.000100`, and the
/// `archives` form of a copied link.
pub fn parse_permalink(url: &str) -> Option<Permalink> {
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let parts: Vec<&str> = path.split('/').collect();
    parts.windows(2).find_map(|pair| {
        if !is_channel_id(pair[0]) {
            return None;
        }
        let ts = ts_from_link_part(pair[1])?;
        let thread_ts = query
            .split('&')
            .find_map(|kv| kv.strip_prefix("thread_ts="))
            .filter(|t| is_ts(t))
            .map(str::to_string);
        Some(Permalink {
            channel: pair[0].to_string(),
            ts,
            thread_ts,
        })
    })
}

/// Milliseconds since the epoch of a Slack message id.
pub fn ts_to_ms(ts: &str) -> Option<i64> {
    let (secs, micro) = ts.split_once('.')?;
    let secs: i64 = secs.parse().ok()?;
    let micro: i64 = micro.parse().ok()?;
    Some(secs * 1000 + micro / 1000)
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn escape(c: char, out: &mut String) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        _ => out.push(c),
    }
}

fn escaped(text: &str) -> String {
    let mut out = String::new();
    text.chars().for_each(|c| escape(c, &mut out));
    out
}

/// `[text](target)` at the start of `chars`: the text, the target and how many characters it took.
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

/// A link of the subset, in Slack markup. A reference to a person or a channel of Slack becomes a mention; a web
/// address becomes a link; anything else stays as the plain text it was.
fn link_to_mrkdwn(label: &str, target: &str) -> Option<String> {
    if let Ok(r) = target.parse::<Ref>() {
        if r.capability != "messaging" || r.provider != "slack" {
            return None;
        }
        return match r.id.strip_prefix("user/") {
            Some(user) if !user.is_empty() && !user.contains(':') => Some(format!("<@{user}>")),
            Some(_) => None,
            None if !r.id.contains(':') => Some(format!("<#{}>", r.id)),
            None => None,
        };
    }
    if target.starts_with("https://")
        || target.starts_with("http://")
        || target.starts_with("mailto:")
    {
        let clean: String = target
            .chars()
            .filter(|c| !matches!(c, '<' | '>' | '|'))
            .collect();
        return Some(format!("<{clean}|{}>", escaped(label)));
    }
    None
}

fn convert(chars: &[char], out: &mut String) {
    let mut i = 0;
    let mut line_start = true;
    while i < chars.len() {
        let c = chars[i];
        let rest = &chars[i..];
        let starts = |pat: &str| rest.iter().take(pat.len()).collect::<String>() == pat;
        let find = |pat: &str, from: usize| -> Option<usize> {
            let p: Vec<char> = pat.chars().collect();
            (from..rest.len().saturating_sub(p.len() - 1)).find(|k| rest[*k..*k + p.len()] == p[..])
        };
        let mut took = 0;
        if c == '`' {
            if let Some(end) = find("`", 1) {
                out.push('`');
                out.push_str(&escaped(&rest[1..end].iter().collect::<String>()));
                out.push('`');
                took = end + 1;
            }
        } else if starts("**") {
            if let Some(end) = find("**", 2).filter(|e| *e > 2) {
                out.push('*');
                convert(&rest[2..end], out);
                out.push('*');
                took = end + 2;
            }
        } else if starts("~~") {
            if let Some(end) = find("~~", 2).filter(|e| *e > 2) {
                out.push('~');
                convert(&rest[2..end], out);
                out.push('~');
                took = end + 2;
            }
        } else if c == '*' && rest.get(1).is_some_and(|n| !n.is_whitespace() && *n != '*') {
            if let Some(end) = find("*", 1).filter(|e| *e > 1 && !rest[e - 1].is_whitespace()) {
                out.push('_');
                convert(&rest[1..end], out);
                out.push('_');
                took = end + 1;
            }
        } else if c == '['
            && let Some((label, target, len)) = link_at(rest)
            && let Some(text) = link_to_mrkdwn(&label, &target)
        {
            out.push_str(&text);
            took = len;
        }
        if took > 0 {
            i += took;
            line_start = false;
            continue;
        }
        if c == '>' && line_start && rest.get(1) == Some(&' ') {
            out.push('>');
        } else {
            escape(c, out);
        }
        line_start = c == '\n';
        i += 1;
    }
}

/// Turns the markdown subset of the spec into Slack markup, and escapes `&`, `<` and `>` so a text cannot make a mention
/// or an `@channel` that its sender did not write as a link. Fenced code is copied as it is, only escaped.
pub fn to_mrkdwn(text: &str) -> String {
    let mut out = String::new();
    for (n, part) in text.split("```").enumerate() {
        if n > 0 {
            out.push_str("```");
        }
        if n % 2 == 1 {
            out.push_str(&escaped(part));
        } else {
            convert(&part.chars().collect::<Vec<_>>(), &mut out);
        }
    }
    out
}

fn after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let at = text.find(marker)?;
    Some(&text[at + marker.len()..])
}

/// What a failed `slackcli` command means. The messages are the ones in `src/domain/errors.ts` of slackcli.
pub fn error_of(stderr: &str) -> CapError {
    let text = stderr.trim();
    let lower = text.to_lowercase();
    if lower.contains("no slack credentials") || lower.contains("rejected the credential") {
        CapError::NotSignedIn
    } else if lower.contains("rate limiting") {
        let secs = after(text, "Retry in ")
            .map(|s| {
                s.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
            })
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(1);
        CapError::RateLimited {
            retry_after_ms: secs * 1000,
        }
    } else if lower.contains("could not reach slack") {
        CapError::Offline
    } else if lower.contains("no channel matches") || lower.contains("channel_not_found") {
        CapError::not_found("the channel")
    } else if lower.contains("thread_not_found") || lower.contains("message_not_found") {
        CapError::not_found("the message")
    } else if lower.contains("not a slack thread reference") {
        CapError::invalid("reference")
    } else {
        let code = after(text, ": ")
            .map(|c| c.split_whitespace().next().unwrap_or("error").to_string())
            .unwrap_or_else(|| "slackcli".into());
        CapError::Provider {
            code,
            message: text.to_string(),
        }
    }
}
