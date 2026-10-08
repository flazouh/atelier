use std::io;
use std::path::Path;

use serde::Deserialize;

use super::iso_secs::iso_secs;
use super::lines::for_each_line;
use super::title::{clean_title, is_prompt};
use crate::usage_history::consts::UNKNOWN_MODEL;
use crate::usage_history::structs::{Buckets, FileUsage, Tokens};

#[derive(Deserialize)]
struct Line<T> {
    timestamp: Option<String>,
    payload: Option<T>,
}

#[derive(Deserialize)]
struct Meta {
    id: Option<String>,
    cwd: Option<String>,
}

#[derive(Deserialize)]
struct Turn {
    model: Option<String>,
    cwd: Option<String>,
}

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: Option<String>,
    info: Option<Info>,
}

#[derive(Deserialize)]
struct Info {
    total_token_usage: Option<Raw>,
    last_token_usage: Option<Raw>,
}

#[derive(Deserialize, Default, Clone, Copy)]
struct Raw {
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_write_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct Message {
    role: Option<String>,
    content: Option<Vec<Part>>,
}

#[derive(Deserialize)]
struct Part {
    text: Option<String>,
}

impl Raw {
    /// Codex counts the cached and cache-written tokens inside `input_tokens`; they are taken out here.
    fn tokens(self) -> Tokens {
        let cached = self.cached_input_tokens.unwrap_or(0);
        let written = self.cache_write_input_tokens.unwrap_or(0);
        Tokens {
            input: self.input_tokens.unwrap_or(0).saturating_sub(cached).saturating_sub(written),
            output: self.output_tokens.unwrap_or(0),
            cache_read: cached,
            cache_write: written,
        }
    }
}

/// Reads one Codex rollout. Each `token_count` event counts the delta of the running totals, so a repeated event
/// adds nothing; when the totals go down (a new thread) or are missing, the event's own `last_token_usage` counts.
pub(crate) fn read_codex(path: &Path, fallback_id: String) -> io::Result<FileUsage> {
    let mut skipped = 0usize;
    let mut session_id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut model = UNKNOWN_MODEL.to_owned();
    let mut first_prompt: Option<String> = None;
    let mut previous: Option<Tokens> = None;
    let mut buckets = Buckets::default();

    for_each_line(path, |line| {
        if !line.ends_with('}') {
            skipped += 1;
        } else if line.contains("\"type\":\"token_count\"") {
            let Ok(l) = serde_json::from_str::<Line<Event>>(line) else {
                skipped += 1;
                return;
            };
            let (Some(payload), Some(ts)) = (l.payload, l.timestamp.as_deref().and_then(iso_secs)) else { return };
            if payload.kind.as_deref() != Some("token_count") {
                return;
            }
            let Some(info) = payload.info else { return };
            let total = info.total_token_usage.map(Raw::tokens);
            let last = info.last_token_usage.map(Raw::tokens);
            let delta = match (total, previous) {
                (Some(t), Some(p)) if t.covers(&p) => Some(t - p),
                (Some(_), Some(_)) => last,
                (Some(t), None) => Some(last.unwrap_or(t)),
                (None, _) => last,
            };
            if total.is_some() {
                previous = total;
            }
            if let Some(d) = delta.filter(|d| d.total() > 0) {
                buckets.add(ts, &model, d);
            }
        } else if line.contains("\"type\":\"session_meta\"") {
            match serde_json::from_str::<Line<Meta>>(line) {
                Ok(Line { payload: Some(m), .. }) => {
                    session_id = session_id.take().or(m.id.filter(|i| !i.is_empty()));
                    cwd = cwd.take().or(m.cwd.filter(|c| !c.is_empty()));
                }
                Ok(_) => {}
                Err(_) => skipped += 1,
            }
        } else if line.contains("\"type\":\"turn_context\"") {
            match serde_json::from_str::<Line<Turn>>(line) {
                Ok(Line { payload: Some(t), .. }) => {
                    if let Some(m) = t.model.filter(|m| !m.is_empty()) {
                        model = m;
                    }
                    cwd = cwd.take().or(t.cwd.filter(|c| !c.is_empty()));
                }
                Ok(_) => {}
                Err(_) => skipped += 1,
            }
        } else if first_prompt.is_none() && line.contains("\"role\":\"user\"") {
            let Ok(l) = serde_json::from_str::<Line<Message>>(line) else { return };
            let Some(m) = l.payload.filter(|m| m.role.as_deref() == Some("user")) else { return };
            first_prompt = m
                .content
                .into_iter()
                .flatten()
                .filter_map(|p| p.text)
                .find(|t| is_prompt(t) && !t.trim_start().starts_with('#'))
                .map(|t| clean_title(&t))
                .filter(|t| !t.is_empty());
        }
    })?;

    Ok(FileUsage {
        session_id: session_id.unwrap_or(fallback_id),
        subagent: false,
        cwd,
        title: None,
        first_prompt,
        records: buckets.into_records(),
        skipped,
    })
}
