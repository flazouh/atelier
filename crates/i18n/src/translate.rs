//! Has a model write one key in every locale.
//!
//! The model gets the English, a note on where the string shows, and what the glossary says about the words in it;
//! it answers with one JSON object of tag to string. The answer is checked before anything is written: every locale
//! is there, every `{name}` survived, and every name in the glossary is written as it is. A failed check goes back
//! to the model once, with what failed.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{Locale, glossary, locale::FIELDS};

/// The model that translates, as OpenRouter names it.
pub const MODEL: &str = "openai/gpt-6-luna";

/// A key to translate.
pub struct Key<'a> {
    /// `snake_case`; it names the file and the constant.
    pub name: &'a str,
    /// The English string, with `{name}` where a value goes.
    pub english: &'a str,
    /// Where the string shows and how much room it has: what a translator would ask.
    pub note: &'a str,
}

/// Something wrong with an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    NotJson(String),
    Missing(&'static str),
    Unknown(String),
    Empty(&'static str),
    Placeholders { tag: &'static str, want: Vec<String>, got: Vec<String> },
    Name { tag: &'static str, term: &'static str },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NotJson(why) => write!(f, "the answer is not one JSON object of strings: {why}"),
            Problem::Missing(tag) => write!(f, "\"{tag}\" is missing"),
            Problem::Unknown(tag) => write!(f, "\"{tag}\" is not one of the locales"),
            Problem::Empty(tag) => write!(f, "\"{tag}\" is empty"),
            Problem::Placeholders { tag, want, got } => write!(f, "\"{tag}\" has the placeholders {got:?}, and should have {want:?}"),
            Problem::Name { tag, term } => write!(f, "\"{tag}\" must write \"{term}\" exactly as it is"),
        }
    }
}

/// Whether `name` can name a key: lowercase words joined by underscores.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(|c: char| c.is_ascii_digit() || c == '_')
        && !name.ends_with('_')
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The `{name}`s in `text`, in order.
pub fn placeholders(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        found.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    found
}

/// The system and the user message to send.
pub fn prompt(key: &Key) -> (String, String) {
    let tags: Vec<String> = Locale::ALL.iter().map(|l| format!("{} ({})", l.tag(), l.name())).collect();
    let system = format!(
        "You translate the interface text of Atelier, a desktop app where people work with coding agents. \
         Write the text the way a careful native speaker who builds software would: natural, short, and in the register of a \
         modern app, not word for word. Keep the room it has in mind; a label that is short in English stays short.\n\n\
         Rules:\n\
         - Answer with one JSON object and nothing else. Its keys are exactly these locale tags, each once: {tags}.\n\
         - Keep every {{placeholder}} exactly as written, braces included. Move it where the grammar of the language needs it.\n\
         - Keep any name you are told to keep exactly as written, in Latin letters, in every language.\n\
         - Where a term is given an approved translation for a language, use that translation.\n\
         - Keep the ellipsis, punctuation and capitalisation style of the language: sentence case, and the language's own \
         ellipsis habit (\"…\" if the English has it).\n\
         - Write Arabic and Hebrew right to left as ordinary text; add no direction marks.\n\
         - \"es\" is Spain, \"es-419\" is Latin America, \"fr-CA\" is Canada, \"pt-BR\" is Brazil, \"pt-PT\" is Portugal, \
         \"zh-CN\" is Simplified Chinese, \"zh-TW\" is Traditional Chinese. \"en\" is the English you were given.",
        tags = tags.join(", "),
    );
    let mut user = format!("Key: {}\nEnglish: {}\nWhere it shows: {}\n", key.name, key.english, key.note);
    let terms = glossary::matching(key.english);
    if !terms.is_empty() {
        user.push_str("\nGlossary for this text:\n");
        for term in terms {
            if term.keep {
                user.push_str(&format!("- \"{}\": a name. Keep it exactly as written.\n", term.source));
            }
            for (locale, word) in term.fixed {
                user.push_str(&format!("- \"{}\" in {}: \"{}\"\n", term.source, locale.tag(), word));
            }
        }
    }
    (system, user)
}

/// The Chat Completions body for OpenRouter.
pub fn request_body(system: &str, user: &str, earlier: &[Problem]) -> Value {
    let mut messages = vec![json!({"role": "system", "content": system}), json!({"role": "user", "content": user})];
    if !earlier.is_empty() {
        let list: Vec<String> = earlier.iter().map(|p| format!("- {p}")).collect();
        messages.push(json!({"role": "user", "content": format!("Your last answer had problems:\n{}\nAnswer again with the whole object, fixed.", list.join("\n"))}));
    }
    json!({"model": MODEL, "messages": messages, "response_format": {"type": "json_object"}, "reasoning": {"effort": "low"}, "temperature": 0.2})
}

/// The text of the model's reply in a Chat Completions response.
pub fn reply_text(response: &Value) -> Option<&str> {
    response.pointer("/choices/0/message/content")?.as_str()
}

/// The answer as tag to string. A reply wrapped in a code fence is read as what is inside it.
pub fn parse(reply: &str) -> Result<BTreeMap<String, String>, Problem> {
    let text = reply.trim();
    let text = text.strip_prefix("```json").or_else(|| text.strip_prefix("```")).unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text).trim();
    let value: Value = serde_json::from_str(text).map_err(|e| Problem::NotJson(e.to_string()))?;
    let Value::Object(map) = value else { return Err(Problem::NotJson("it is not an object".into())) };
    map.into_iter()
        .map(|(tag, v)| match v {
            Value::String(s) => Ok((tag, s)),
            _ => Err(Problem::NotJson(format!("\"{tag}\" is not a string"))),
        })
        .collect()
}

/// Everything wrong with `answer` as the translation of `key`; empty when it can be written.
pub fn check(key: &Key, answer: &BTreeMap<String, String>) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut want = placeholders(key.english);
    want.sort();
    let terms: Vec<&'static str> = glossary::matching(key.english).into_iter().filter(|t| t.keep).map(|t| t.source).collect();
    for tag in answer.keys() {
        if !Locale::ALL.iter().any(|l| l.tag() == tag) {
            problems.push(Problem::Unknown(tag.clone()));
        }
    }
    for locale in Locale::ALL {
        let tag = locale.tag();
        let Some(text) = answer.get(tag) else {
            problems.push(Problem::Missing(tag));
            continue;
        };
        if text.trim().is_empty() {
            problems.push(Problem::Empty(tag));
            continue;
        }
        let mut got = placeholders(text);
        got.sort();
        if got != want {
            problems.push(Problem::Placeholders { tag, want: want.clone(), got });
        }
        for term in &terms {
            if !text.contains(term) {
                problems.push(Problem::Name { tag, term });
            }
        }
    }
    problems
}

/// The Rust file for `key`: one constant that fills every field of [`super::Message`].
pub fn render(key: &Key, answer: &BTreeMap<String, String>) -> String {
    let mut out = format!("//! {}\n//!\n//! {}\nuse atelier_i18n::Message;\n\npub const {}: Message = Message {{\n", key.english.replace('\n', " "), key.note.replace('\n', " "), key.name.to_uppercase());
    for (locale, field) in Locale::ALL.iter().zip(FIELDS) {
        let text = answer.get(locale.tag()).map_or("", String::as_str);
        out.push_str(&format!("    {field}: {},\n", literal(text)));
    }
    out.push_str("};\n");
    out
}

/// Marks that draw nothing and are easy to lose in a diff: zero-width and direction marks, and the soft hyphen.
fn invisible(c: char) -> bool {
    matches!(c, '\u{ad}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

/// `text` as a Rust string literal. Letters stay letters, in any script: only the quote, the backslash and
/// the characters that cannot be seen are escaped, so a reviewer can read the file.
fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() || invisible(c) => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Why a key could not be translated.
#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    Name,
    Send(String),
    NoReply,
    Answer(Vec<Problem>),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Name => write!(f, "a key is lowercase words joined by underscores"),
            Failure::Send(why) => write!(f, "the request failed: {why}"),
            Failure::NoReply => write!(f, "the response held no reply"),
            Failure::Answer(problems) => {
                write!(f, "the model's answer was refused after a second try:")?;
                problems.iter().try_for_each(|p| write!(f, "\n  {p}"))
            }
        }
    }
}

/// Translates `key`: asks, checks, and asks once more with the problems if the answer fails. Returns the file's source.
/// `send` posts a body and returns the response; the tool gives it HTTP, a test gives it a script.
pub fn translate(key: &Key, send: &mut dyn FnMut(&Value) -> Result<Value, String>) -> Result<String, Failure> {
    if !valid_name(key.name) {
        return Err(Failure::Name);
    }
    let (system, user) = prompt(key);
    let mut problems = Vec::new();
    for _ in 0..2 {
        let response = send(&request_body(&system, &user, &problems)).map_err(Failure::Send)?;
        let reply = reply_text(&response).ok_or(Failure::NoReply)?;
        problems = match parse(reply) {
            Ok(answer) => match check(key, &answer) {
                found if found.is_empty() => return Ok(render(key, &answer)),
                found => found,
            },
            Err(problem) => vec![problem],
        };
    }
    Err(Failure::Answer(problems))
}
