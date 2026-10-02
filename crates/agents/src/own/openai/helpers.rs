use std::time::Duration;

use serde_json::{Value, json};

use super::super::{
    message::{Block, ModelError, ModelRequest, Role},
    };

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

pub(super) fn status_error(status: u16, body: &str, retry_after: Option<&str>) -> ModelError {
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
