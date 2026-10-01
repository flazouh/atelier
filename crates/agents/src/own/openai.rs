//! Chat Completions, streaming, for OpenAI and every service that copies it: OpenRouter, vLLM, Ollama,
//! Together, Groq. Tool calls arrive as fragments by index. Reasoning arrives as `reasoning_content`
//! (DeepSeek and others) or `reasoning` (OpenRouter), and is shown but not sent back.
use std::{collections::BTreeMap, time::Duration};

use serde_json::{Value, json};

use super::{
    http::{HttpOptions, HttpRequest, send},
    message::{Block, Cancel, Delta, Model, ModelError, ModelRequest, Reply, Role, Secret, StopReason, TokenUsage},
    sse::{Parser, SseEvent},
};

pub const OPENAI_BASE: &str = "https://api.openai.com/v1";
pub const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
const ERROR_BODY: usize = 8 * 1024;

pub struct OpenAiCompatible {
    key: Option<Secret>,
    /// Up to and including the version, such as `https://api.openai.com/v1`.
    base: String,
    extra_headers: Vec<(String, String)>,
    http: HttpOptions,
}

impl OpenAiCompatible {
    pub fn new(key: Option<Secret>, base: impl Into<String>) -> Self {
        Self { key, base: base.into().trim_end_matches('/').to_string(), extra_headers: Vec::new(), http: HttpOptions::default() }
    }

    /// OpenRouter: the same API with a different base and two headers that name the app.
    pub fn openrouter(key: Secret) -> Self {
        let mut client = Self::new(Some(key), OPENROUTER_BASE);
        client.extra_headers = vec![("HTTP-Referer".into(), "https://github.com/flazouh/atelier".into()), ("X-Title".into(), "atelier".into())];
        client
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    pub fn with_http(mut self, http: HttpOptions) -> Self {
        self.http = http;
        self
    }
}

pub fn request_body(request: &ModelRequest<'_>) -> Value {
    let mut messages = vec![json!({"role": "system", "content": request.system})];
    for message in request.messages {
        match message.role {
            Role::User => {
                // Each tool result is a message of its own; the rest of the message is the user's words.
                for block in &message.blocks {
                    if let Block::ToolResult { id, content, .. } = block {
                        messages.push(json!({"role": "tool", "tool_call_id": id, "content": content}));
                    }
                }
                let text = message.text();
                if !text.trim().is_empty() {
                    messages.push(json!({"role": "user", "content": text}));
                }
            }
            Role::Assistant => {
                let text = message.text();
                let calls: Vec<Value> = message
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::ToolUse { id, name, input } => {
                            Some(json!({"id": id, "type": "function", "function": {"name": name, "arguments": input.to_string()}}))
                        }
                        _ => None,
                    })
                    .collect();
                if text.trim().is_empty() && calls.is_empty() {
                    continue;
                }
                let mut entry = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) }});
                if !calls.is_empty() {
                    entry["tool_calls"] = json!(calls);
                }
                messages.push(entry);
            }
        }
    }
    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "stream": true,
        "stream_options": {"include_usage": true},
        "max_tokens": request.max_tokens,
    });
    if !request.tools.is_empty() {
        body["tools"] = json!(request
            .tools
            .iter()
            .map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.schema}}))
            .collect::<Vec<_>>());
    }
    body
}

fn status_error(status: u16, body: &str, retry_after: Option<&str>) -> ModelError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().or(v["message"].as_str()).map(str::to_string))
        .unwrap_or_else(|| body.chars().take(300).collect());
    match status {
        401 | 403 => ModelError::Auth(message),
        429 => ModelError::RateLimited { retry_after: retry_after.and_then(|s| s.trim().parse::<f64>().ok()).map(Duration::from_secs_f64), message },
        500..=599 => ModelError::Server { status, message },
        _ => ModelError::Request { status, message },
    }
}

impl Model for OpenAiCompatible {
    fn stream(&self, request: &ModelRequest<'_>, sink: &mut dyn FnMut(Delta), cancel: &Cancel) -> Result<Reply, ModelError> {
        let body = serde_json::to_vec(&request_body(request)).map_err(|e| ModelError::Request { status: 0, message: e.to_string() })?;
        let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
        if let Some(key) = &self.key {
            headers.push(("authorization".into(), format!("Bearer {}", key.expose())));
        }
        headers.extend(self.extra_headers.iter().cloned());
        let http = HttpRequest { method: "POST", url: format!("{}/chat/completions", self.base), headers, body };
        let mut response = send(&http, cancel, &self.http).map_err(|e| match e {
            super::http::HttpError::Cancelled => ModelError::Cancelled,
            super::http::HttpError::Protocol(why) => ModelError::Malformed(why),
            other => ModelError::Network(other.to_string()),
        })?;
        if response.status != 200 {
            let text = response.body.text(ERROR_BODY);
            return Err(status_error(response.status, &text, response.header("retry-after")));
        }
        let mut state = ChatState::default();
        let mut parser = Parser::default();
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let n = std::io::Read::read(&mut response.body, &mut chunk).map_err(|e| match super::http::map_read_error(&e) {
                super::http::HttpError::Cancelled => ModelError::Cancelled,
                other => ModelError::Network(other.to_string()),
            })?;
            let mut events: Vec<SseEvent> = Vec::new();
            if n == 0 {
                parser.finish(&mut |e| events.push(e));
            } else {
                parser.feed(&chunk[..n], &mut |e| events.push(e)).map_err(ModelError::Malformed)?;
            }
            for event in events {
                state.on_data(&event.data, sink)?;
            }
            if state.done || n == 0 {
                break;
            }
        }
        state.finish(sink)
    }
}

#[derive(Default)]
struct ToolFragment {
    id: String,
    name: String,
    args: String,
    announced: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    None,
    Text,
    Thinking,
}

/// The reply as it builds, from `data:` lines.
#[derive(Default)]
pub struct ChatState {
    text: String,
    thinking: String,
    open: Option<Open>,
    tools: BTreeMap<usize, ToolFragment>,
    stop: Option<StopReason>,
    usage: TokenUsage,
    pub done: bool,
    saw_chunk: bool,
}

impl ChatState {
    fn switch(&mut self, to: Open, sink: &mut dyn FnMut(Delta)) {
        let now = self.open.unwrap_or(Open::None);
        if now != to && now != Open::None {
            sink(Delta::BlockEnd);
        }
        self.open = Some(to);
    }

    pub fn on_data(&mut self, data: &str, sink: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        let data = data.trim();
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let chunk: Value = serde_json::from_str(data).map_err(|_| ModelError::Malformed("a chunk is not JSON".into()))?;
        if chunk["error"].is_object() {
            let message = chunk["error"]["message"].as_str().unwrap_or("the API reported an error").to_string();
            return Err(ModelError::Server { status: chunk["error"]["code"].as_u64().unwrap_or(500) as u16, message });
        }
        self.saw_chunk = true;
        if let Some(usage) = chunk["usage"].as_object().filter(|u| !u.is_empty()) {
            let get = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
            let cached = usage.get("prompt_tokens_details").and_then(|d| d["cached_tokens"].as_u64()).unwrap_or(0);
            self.usage = TokenUsage { input: get("prompt_tokens").saturating_sub(cached), output: get("completion_tokens"), cache_read: cached, cache_write: 0 };
        }
        let Some(choice) = chunk["choices"].get(0) else { return Ok(()) };
        let delta = &choice["delta"];
        let reasoning = delta["reasoning_content"].as_str().or(delta["reasoning"].as_str()).filter(|s| !s.is_empty());
        if let Some(piece) = reasoning {
            self.switch(Open::Thinking, sink);
            self.thinking.push_str(piece);
            sink(Delta::Thinking(piece.to_string()));
        }
        if let Some(piece) = delta["content"].as_str().filter(|s| !s.is_empty()) {
            self.switch(Open::Text, sink);
            self.text.push_str(piece);
            sink(Delta::Text(piece.to_string()));
        }
        if let Some(calls) = delta["tool_calls"].as_array() {
            for call in calls {
                let index = call["index"].as_u64().unwrap_or(0) as usize;
                let fragment = self.tools.entry(index).or_default();
                if let Some(id) = call["id"].as_str().filter(|s| !s.is_empty()) {
                    fragment.id = id.to_string();
                }
                if let Some(name) = call["function"]["name"].as_str().filter(|s| !s.is_empty()) {
                    fragment.name.push_str(name);
                }
                fragment.args.push_str(call["function"]["arguments"].as_str().unwrap_or(""));
                if !fragment.announced && !fragment.id.is_empty() && !fragment.name.is_empty() {
                    fragment.announced = true;
                    let (id, name) = (fragment.id.clone(), fragment.name.clone());
                    // Text or thinking before the calls ends here.
                    if self.open.is_some_and(|o| o != Open::None) {
                        sink(Delta::BlockEnd);
                        self.open = Some(Open::None);
                    }
                    sink(Delta::ToolStart { id, name });
                }
            }
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.stop = Some(match reason {
                "stop" => StopReason::EndTurn,
                "tool_calls" | "function_call" => StopReason::ToolUse,
                "length" => StopReason::MaxTokens,
                "content_filter" => StopReason::Refusal,
                other => StopReason::Other(other.to_string()),
            });
        }
        Ok(())
    }

    pub fn finish(mut self, sink: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
        if !self.done || !self.saw_chunk {
            return Err(ModelError::Malformed("the stream ended before the reply was complete".into()));
        }
        if self.open.is_some_and(|o| o != Open::None) {
            sink(Delta::BlockEnd);
        }
        let mut blocks = Vec::new();
        if !self.thinking.is_empty() {
            blocks.push(Block::Thinking { text: std::mem::take(&mut self.thinking), signature: None });
        }
        if !self.text.is_empty() {
            blocks.push(Block::Text { text: std::mem::take(&mut self.text) });
        }
        let mut malformed = Vec::new();
        for (_, tool) in std::mem::take(&mut self.tools) {
            if tool.name.is_empty() {
                continue;
            }
            let id = if tool.id.is_empty() { format!("call_{}", blocks.len()) } else { tool.id };
            let input = if tool.args.trim().is_empty() { Some(json!({})) } else { serde_json::from_str::<Value>(&tool.args).ok() };
            match input {
                Some(input) => {
                    sink(Delta::ToolDone { id: id.clone(), input: input.clone() });
                    blocks.push(Block::ToolUse { id, name: tool.name, input });
                }
                None => {
                    sink(Delta::ToolDone { id: id.clone(), input: Value::Null });
                    malformed.push(id.clone());
                    blocks.push(Block::ToolUse { id, name: tool.name, input: Value::Null });
                }
            }
        }
        let has_calls = blocks.iter().any(|b| matches!(b, Block::ToolUse { .. }));
        let stop = match self.stop {
            Some(StopReason::EndTurn) if has_calls => StopReason::ToolUse,
            Some(stop) => stop,
            None if has_calls => StopReason::ToolUse,
            None => StopReason::EndTurn,
        };
        Ok(Reply { blocks, stop, usage: self.usage, malformed })
    }
}
