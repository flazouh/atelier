pub use super::locale::Message;
use super::locale::current;

/// `message` in the current language.
pub fn t(message: &Message) -> &'static str {
    message.get(current())
}

/// `message` in the current language, each `{name}` in it replaced by the value `params` gives that name.
/// A name `params` lacks is left as written, so a missing value shows rather than hides.
pub fn t_with(message: &Message, params: &[(&str, &str)]) -> String {
    fill(t(message), params)
}

pub(crate) fn fill(text: &str, params: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match params.iter().find(|(n, _)| *n == name) {
                    Some((_, value)) => out.push_str(value),
                    None => out.push_str(&rest[open..open + close + 2]),
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
