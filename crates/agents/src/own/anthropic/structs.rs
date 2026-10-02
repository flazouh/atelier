use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::super::{
    http::{HttpOptions, HttpRequest, send},
    message::{Block, Cancel, Delta, Model, ModelError, ModelRequest, Reply, Secret, StopReason, TokenUsage},
    sse::{Parser, SseEvent},
};
use super::types::{DEFAULT_BASE, ERROR_BODY, Partial, VERSION};
use super::helpers::{error_of, from_http, request_body, status_error};

pub struct Anthropic {
    key: Secret,
    base: String,
    pub(super) http: HttpOptions,
}

impl Anthropic {
    pub fn new(key: Secret) -> Self {
        Self { key, base: DEFAULT_BASE.into(), http: HttpOptions::default() }
    }

    /// Talks to another server that speaks this API: a proxy, or a test server.
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into().trim_end_matches('/').to_string();
        self
    }

    pub fn with_http(mut self, http: HttpOptions) -> Self {
        self.http = http;
        self
    }

    fn headers(&self) -> Vec<(String, String)> {
        vec![
            ("x-api-key".into(), self.key.expose().to_string()),
            ("anthropic-version".into(), VERSION.into()),
            ("content-type".into(), "application/json".into()),
        ]
    }

    /// The models the account can use, from `GET /v1/models`, newest first as the API lists them.
    pub fn models(&self, cancel: &Cancel) -> Result<Vec<(String, String)>, ModelError> {
        let request = HttpRequest { method: "GET", url: format!("{}/v1/models?limit=100", self.base), headers: self.headers(), body: Vec::new() };
        let mut response = send(&request, cancel, &self.http).map_err(from_http)?;
        if response.status != 200 {
            let body = response.body.text(ERROR_BODY);
            return Err(status_error(response.status, &body, response.header("retry-after")));
        }
        let body = response.body.text(4 * 1024 * 1024);
        let value: Value = serde_json::from_str(&body).map_err(|e| ModelError::Malformed(format!("the model list: {e}")))?;
        Ok(value["data"]
            .as_array()
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| Some((m["id"].as_str()?.to_string(), m["display_name"].as_str().unwrap_or(m["id"].as_str()?).to_string())))
                    .collect()
            })
            .unwrap_or_default())
    }
}

impl Model for Anthropic {
    fn stream(&self, request: &ModelRequest<'_>, sink: &mut dyn FnMut(Delta), cancel: &Cancel) -> Result<Reply, ModelError> {
        let body = serde_json::to_vec(&request_body(request)).map_err(|e| ModelError::Request { status: 0, message: e.to_string() })?;
        let http = HttpRequest { method: "POST", url: format!("{}/v1/messages", self.base), headers: self.headers(), body };
        let mut response = send(&http, cancel, &self.http).map_err(from_http)?;
        if response.status != 200 {
            let text = response.body.text(ERROR_BODY);
            return Err(status_error(response.status, &text, response.header("retry-after")));
        }
        let mut state = StreamState::default();
        let mut parser = Parser::default();
        let mut failure: Option<ModelError> = None;
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let n = std::io::Read::read(&mut response.body, &mut chunk).map_err(|e| from_http(super::super::http::map_read_error(&e)))?;
            let mut events: Vec<SseEvent> = Vec::new();
            if n == 0 {
                parser.finish(&mut |e| events.push(e));
            } else {
                parser.feed(&chunk[..n], &mut |e| events.push(e)).map_err(ModelError::Malformed)?;
            }
            for event in events {
                if failure.is_none()
                    && let Err(e) = state.on_event(event.name.as_deref(), &event.data, sink)
                {
                    failure = Some(e);
                }
            }
            if let Some(e) = failure {
                return Err(e);
            }
            if state.done || n == 0 {
                break;
            }
        }
        state.finish()
    }
}

/// The reply as it builds. Feed it each event; it says what to show and, at the end, gives the reply.
#[derive(Default)]
pub struct StreamState {
    open: BTreeMap<usize, Partial>,
    pub(super) blocks: Vec<Block>,
    usage: TokenUsage,
    stop: Option<StopReason>,
    pub(super) malformed: Vec<String>,
    pub done: bool,
    started: bool,
}

impl StreamState {
    pub fn on_event(&mut self, name: Option<&str>, data: &str, sink: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        let bad = |why: &str| ModelError::Malformed(why.to_string());
        let event: Value = serde_json::from_str(data).map_err(|_| bad("an event is not JSON"))?;
        let kind = event["type"].as_str().or(name).unwrap_or("");
        match kind {
            "ping" => {}
            "error" => return Err(error_of(&event["error"])),
            "message_start" => {
                self.started = true;
                let usage = &event["message"]["usage"];
                self.usage.input = usage["input_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_read = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                self.usage.cache_write = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                self.usage.output = usage["output_tokens"].as_u64().unwrap_or(0);
            }
            "content_block_start" => {
                let index = event["index"].as_u64().ok_or_else(|| bad("a block has no index"))? as usize;
                let block = &event["content_block"];
                let partial = match block["type"].as_str().unwrap_or("") {
                    "text" => Partial::Text(String::new()),
                    "thinking" => Partial::Thinking { text: String::new(), signature: None },
                    "redacted_thinking" => {
                        self.blocks.push(Block::Redacted { data: block["data"].as_str().unwrap_or("").to_string() });
                        Partial::Skip
                    }
                    "tool_use" => {
                        let id = block["id"].as_str().ok_or_else(|| bad("a tool call has no id"))?.to_string();
                        let name = block["name"].as_str().ok_or_else(|| bad("a tool call has no name"))?.to_string();
                        sink(Delta::ToolStart { id: id.clone(), name: name.clone() });
                        Partial::Tool { id, name, json: String::new() }
                    }
                    _ => Partial::Skip,
                };
                self.open.insert(index, partial);
            }
            "content_block_delta" => {
                let index = event["index"].as_u64().ok_or_else(|| bad("a delta has no index"))? as usize;
                let delta = &event["delta"];
                let Some(partial) = self.open.get_mut(&index) else { return Err(bad("a delta for a block that is not open")) };
                match (delta["type"].as_str().unwrap_or(""), partial) {
                    ("text_delta", Partial::Text(text)) => {
                        let piece = delta["text"].as_str().unwrap_or("");
                        text.push_str(piece);
                        sink(Delta::Text(piece.to_string()));
                    }
                    ("thinking_delta", Partial::Thinking { text, .. }) => {
                        let piece = delta["thinking"].as_str().unwrap_or("");
                        text.push_str(piece);
                        sink(Delta::Thinking(piece.to_string()));
                    }
                    ("signature_delta", Partial::Thinking { signature, .. }) => {
                        signature.get_or_insert_with(String::new).push_str(delta["signature"].as_str().unwrap_or(""));
                    }
                    ("input_json_delta", Partial::Tool { json, .. }) => json.push_str(delta["partial_json"].as_str().unwrap_or("")),
                    _ => {}
                }
            }
            "content_block_stop" => {
                let index = event["index"].as_u64().ok_or_else(|| bad("a stop has no index"))? as usize;
                match self.open.remove(&index) {
                    Some(Partial::Text(text)) => {
                        self.blocks.push(Block::Text { text });
                        sink(Delta::BlockEnd);
                    }
                    Some(Partial::Thinking { text, signature }) => {
                        self.blocks.push(Block::Thinking { text, signature });
                        sink(Delta::BlockEnd);
                    }
                    Some(Partial::Tool { id, name, json }) => {
                        let input = if json.trim().is_empty() { Some(json!({})) } else { serde_json::from_str::<Value>(&json).ok() };
                        match input {
                            Some(input) => {
                                sink(Delta::ToolDone { id: id.clone(), input: input.clone() });
                                self.blocks.push(Block::ToolUse { id, name, input });
                            }
                            None => {
                                sink(Delta::ToolDone { id: id.clone(), input: Value::Null });
                                self.malformed.push(id.clone());
                                self.blocks.push(Block::ToolUse { id, name, input: Value::Null });
                            }
                        }
                    }
                    Some(Partial::Skip) | None => {}
                }
            }
            "message_delta" => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    self.stop = Some(match reason {
                        "end_turn" | "stop_sequence" => StopReason::EndTurn,
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::MaxTokens,
                        "refusal" => StopReason::Refusal,
                        other => StopReason::Other(other.to_string()),
                    });
                }
                if let Some(output) = event["usage"]["output_tokens"].as_u64() {
                    self.usage.output = output;
                }
                if let Some(input) = event["usage"]["input_tokens"].as_u64() {
                    self.usage.input = input;
                }
            }
            "message_stop" => self.done = true,
            _ => {}
        }
        Ok(())
    }

    /// The reply, once the stream has ended. An end before `message_stop` is an error.
    pub fn finish(self) -> Result<Reply, ModelError> {
        if !self.done || !self.started {
            return Err(ModelError::Malformed("the stream ended before the reply was complete".into()));
        }
        Ok(Reply { blocks: self.blocks, stop: self.stop.unwrap_or(StopReason::EndTurn), usage: self.usage, malformed: self.malformed })
    }
}
