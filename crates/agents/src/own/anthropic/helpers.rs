use std::time::Duration;

use serde_json::{Value, json};

use super::super::{
    http::HttpError,
    message::{Block, Message, ModelError, ModelRequest, Role, Thinking},
};

/// True for a model that needs a token budget for thinking (Haiku 4.5) instead of adaptive thinking.
pub(crate) fn wants_budget(model: &str) -> bool {
    model.starts_with("claude-haiku-4-5") || model.starts_with("claude-sonnet-4-5") || model.starts_with("claude-opus-4-5")
}

fn cached(mut value: Value) -> Value {
    value["cache_control"] = json!({"type": "ephemeral"});
    value
}

pub fn block_json(block: &Block) -> Option<Value> {
    Some(match block {
        Block::Text { text } if text.trim().is_empty() => return None,
        Block::Text { text } => json!({"type": "text", "text": text}),
        Block::Thinking { text, signature: Some(signature) } => json!({"type": "thinking", "thinking": text, "signature": signature}),
        // A thinking block with no signature cannot be sent back, so it is left out.
        Block::Thinking { .. } => return None,
        Block::Redacted { data } => json!({"type": "redacted_thinking", "data": data}),
        Block::ToolUse { id, name, input } => json!({"type": "tool_use", "id": id, "name": name, "input": if input.is_null() { json!({}) } else { input.clone() }}),
        Block::ToolResult { id, content, is_error } => {
            let mut result = json!({"type": "tool_result", "tool_use_id": id, "content": content});
            if *is_error {
                result["is_error"] = json!(true);
            }
            result
        }
    })
}

/// The request body.
pub fn request_body(request: &ModelRequest<'_>) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    for message in request.messages {
        let content: Vec<Value> = message.blocks.iter().filter_map(block_json).collect();
        if content.is_empty() {
            continue;
        }
        let role = if message.role == Role::User { "user" } else { "assistant" };
        messages.push(json!({"role": role, "content": content}));
    }
    // The third breakpoint: the end of the conversation so far.
    if let Some(last) = messages.last_mut().and_then(|m| m["content"].as_array_mut()).and_then(|c| c.last_mut()) {
        *last = cached(last.clone());
    }
    let mut tools: Vec<Value> = request
        .tools
        .iter()
        // `eager_input_streaming` makes a big input (a file to write) stream as it is made. The API then does
        // not validate it, so a cut-off or invalid input is caught by the stream state (`malformed`).
        .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.schema, "eager_input_streaming": true}))
        .collect();
    if let Some(last) = tools.last_mut() {
        *last = cached(last.clone());
    }
    let mut body = json!({
        "model": request.model,
        "max_tokens": request.max_tokens,
        "stream": true,
        "system": [cached(json!({"type": "text", "text": request.system}))],
        "messages": messages,
    });
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    if request.thinking == Thinking::Auto {
        body["thinking"] = if wants_budget(request.model) {
            json!({"type": "enabled", "budget_tokens": (request.max_tokens / 2).clamp(1024, 16_000).min(request.max_tokens.saturating_sub(1))})
        } else {
            // "summarized" so the reasoning can be shown; the default hides it.
            json!({"type": "adaptive", "display": "summarized"})
        };
    }
    body
}

pub(super) fn from_http(error: HttpError) -> ModelError {
    match error {
        HttpError::Cancelled => ModelError::Cancelled,
        HttpError::Connect(why) | HttpError::Io(why) => ModelError::Network(why),
        HttpError::Protocol(why) => ModelError::Malformed(why),
    }
}

pub(super) fn status_error(status: u16, body: &str, retry_after: Option<&str>) -> ModelError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(300).collect());
    match status {
        401 | 403 => ModelError::Auth(message),
        429 => ModelError::RateLimited { retry_after: retry_after.and_then(|s| s.trim().parse::<f64>().ok()).map(Duration::from_secs_f64), message },
        500..=599 => ModelError::Server { status, message },
        _ => ModelError::Request { status, message },
    }
}

pub(super) fn error_of(error: &Value) -> ModelError {
    let message = error["message"].as_str().unwrap_or("the API reported an error").to_string();
    match error["type"].as_str().unwrap_or("") {
        "overloaded_error" => ModelError::Server { status: 529, message },
        "rate_limit_error" => ModelError::RateLimited { retry_after: None, message },
        "authentication_error" | "permission_error" => ModelError::Auth(message),
        "api_error" => ModelError::Server { status: 500, message },
        _ => ModelError::Request { status: 400, message },
    }
}

/// Messages in the API's JSON, for a test or a log that wants to see what would be sent.
pub fn messages_json(messages: &[Message]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| json!({"role": if m.role == Role::User { "user" } else { "assistant" }, "content": m.blocks.iter().filter_map(block_json).collect::<Vec<_>>()}))
        .collect()
}
