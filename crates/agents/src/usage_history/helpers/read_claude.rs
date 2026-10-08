use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::Path;

use serde::Deserialize;

use super::iso_secs::iso_secs;
use super::lines::for_each_line;
use super::title::{clean_title, is_prompt};
use crate::usage_history::consts::UNKNOWN_MODEL;
use crate::usage_history::structs::{Buckets, FileUsage, Tokens};

#[derive(Deserialize)]
struct Assistant {
    timestamp: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<AssistantMessage>,
}

#[derive(Deserialize)]
struct AssistantMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize, Default)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct User {
    cwd: Option<String>,
    #[serde(rename = "isMeta")]
    is_meta: Option<bool>,
    message: Option<UserMessage>,
}

#[derive(Deserialize)]
struct UserMessage {
    content: Option<Content>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
struct Block {
    text: Option<String>,
}

#[derive(Deserialize)]
struct TitleLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "customTitle")]
    custom_title: Option<String>,
    #[serde(rename = "aiTitle")]
    ai_title: Option<String>,
    summary: Option<String>,
}

/// Reads one Claude Code log. `session_id` comes from the file name (or, for a subagent file, its parent session).
pub(crate) fn read_claude(path: &Path, session_id: String, subagent: bool) -> io::Result<FileUsage> {
    let mut skipped = 0usize;
    let mut cwd: Option<String> = None;
    let mut first_prompt: Option<String> = None;
    let (mut custom, mut ai, mut summary): (Option<String>, Option<String>, Option<String>) = (None, None, None);
    let mut keyed: HashMap<u64, (i64, String, Tokens)> = HashMap::new();
    let mut buckets = Buckets::default();

    for_each_line(path, |line| {
        if !line.ends_with('}') {
            skipped += 1; // a half-written or damaged line
        } else if line.contains("\"usage\"") && line.contains("\"type\":\"assistant\"") {
            let Ok(a) = serde_json::from_str::<Assistant>(line) else {
                skipped += 1;
                return;
            };
            let Some(msg) = a.message else { return };
            let (Some(usage), Some(ts)) = (msg.usage, a.timestamp.as_deref().and_then(iso_secs)) else { return };
            let tokens = Tokens {
                input: usage.input_tokens.unwrap_or(0),
                output: usage.output_tokens.unwrap_or(0),
                cache_read: usage.cache_read_input_tokens.unwrap_or(0),
                cache_write: usage.cache_creation_input_tokens.unwrap_or(0),
            };
            if tokens.total() == 0 {
                return;
            }
            if cwd.is_none() {
                cwd = a.cwd.filter(|c| !c.is_empty());
            }
            let model = msg.model.unwrap_or_else(|| UNKNOWN_MODEL.to_owned());
            match (msg.id, a.request_id) {
                (Some(id), Some(request)) => {
                    let mut hasher = DefaultHasher::new();
                    (id, request).hash(&mut hasher);
                    keyed.insert(hasher.finish(), (ts, model, tokens)); // the last repeat wins
                }
                _ => buckets.add(ts, &model, tokens),
            }
        } else if !subagent && first_prompt.is_none() && line.contains("\"type\":\"user\"") {
            let Ok(user) = serde_json::from_str::<User>(line) else {
                skipped += 1;
                return;
            };
            if cwd.is_none() {
                cwd = user.cwd.filter(|c| !c.is_empty());
            }
            if user.is_meta == Some(true) {
                return;
            }
            let text = match user.message.and_then(|m| m.content) {
                Some(Content::Text(t)) => Some(t),
                Some(Content::Blocks(blocks)) => blocks.into_iter().filter_map(|b| b.text).next(),
                None => None,
            };
            first_prompt = text.filter(|t| is_prompt(t)).map(|t| clean_title(&t)).filter(|t| !t.is_empty());
        } else if !subagent
            && (line.contains("\"type\":\"custom-title\"")
                || line.contains("\"type\":\"ai-title\"")
                || line.contains("\"type\":\"summary\""))
        {
            let Ok(t) = serde_json::from_str::<TitleLine>(line) else {
                skipped += 1;
                return;
            };
            let clean = |s: Option<String>| s.map(|s| clean_title(&s)).filter(|s| !s.is_empty());
            match t.kind.as_deref() {
                Some("custom-title") => custom = clean(t.custom_title).or(custom.take()),
                Some("ai-title") => ai = clean(t.ai_title).or(ai.take()),
                Some("summary") => summary = summary.take().or(clean(t.summary)),
                _ => {}
            }
        }
    })?;

    for (ts, model, tokens) in keyed.into_values() {
        buckets.add(ts, &model, tokens);
    }
    Ok(FileUsage {
        session_id,
        subagent,
        cwd,
        title: custom.or(ai).or(summary),
        first_prompt,
        records: buckets.into_records(),
        skipped,
    })
}
