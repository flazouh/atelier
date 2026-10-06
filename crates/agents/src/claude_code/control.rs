//! The lines atelier writes to `claude`'s stdin. Each is one JSON object; the caller adds the line end.
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use serde_json::{Value, json};

use crate::session::{Attachment, PermissionMode, message_text};

/// `id` comes back in the result of the turn that takes the message, so a message `claude` holds for
/// later is told apart from one it has answered.
///
/// The content is a string, until a picture comes with it: then it is blocks, the words first and each picture after.
pub(super) fn user_message(id: &str, text: &str, attachments: &[Attachment]) -> String {
    let content = if attachments.iter().any(|a| a.image().is_some()) {
        let words: Vec<Attachment> = attachments.iter().filter(|a| a.image().is_none()).cloned().collect();
        let mut blocks = vec![json!({"type": "text", "text": message_text(text, &words)})];
        for (format, bytes) in attachments.iter().filter_map(Attachment::image) {
            let data = base64::engine::general_purpose::STANDARD.encode(bytes);
            blocks.push(json!({"type": "image", "source": {"type": "base64", "media_type": format.media_type(), "data": data}}));
        }
        Value::Array(blocks)
    } else {
        Value::String(message_text(text, attachments))
    };
    json!({"type": "user", "uuid": id, "message": {"role": "user", "content": content}}).to_string()
}

/// The ids of the messages one session sends, in the UUID form `claude` takes: a seed from the time the
/// session started, then a count.
pub(super) struct MessageIds {
    seed: u32,
    next: AtomicU64,
}

impl MessageIds {
    pub fn new() -> Self {
        let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        Self { seed: since.subsec_nanos() ^ since.as_secs() as u32, next: AtomicU64::new(1) }
    }

    pub fn next(&self) -> String {
        message_id(self.seed, self.next.fetch_add(1, Ordering::Relaxed))
    }
}

pub(super) fn message_id(seed: u32, number: u64) -> String {
    format!("{seed:08x}-0000-4000-8000-{:012x}", number & 0xffff_ffff_ffff)
}

pub(super) fn interrupt(request_id: &str) -> String {
    control(request_id, json!({"subtype": "interrupt"}))
}

/// The prefix of the id of a request for the context's breakdown, so its answer is told from the answers to other requests.
pub(super) const CONTEXT_REQUEST: &str = "atelier-context-";

pub(super) fn context_usage(request_id: &str) -> String {
    control(request_id, json!({"subtype": "get_context_usage"}))
}

pub(super) fn set_model(request_id: &str, model: &str) -> String {
    control(request_id, json!({"subtype": "set_model", "model": model}))
}

pub(super) fn set_permission_mode(request_id: &str, mode: PermissionMode) -> String {
    control(request_id, json!({"subtype": "set_permission_mode", "mode": mode_name(mode)}))
}

/// The answer to a `can_use_tool` request: allow with the input as it was, with the rules `claude`
/// suggested added, or deny with a message the model reads.
pub(super) fn allow(request_id: &str, input: &Value, always: Option<&Value>) -> String {
    let mut body = json!({"behavior": "allow", "updatedInput": input});
    if let Some(rules) = always {
        body["updatedPermissions"] = rules.clone();
    }
    answer(request_id, body)
}

/// The answer to a `can_use_tool` request for a question: allow, with the reader's answers added to the input.
pub(super) fn allow_with_answers(request_id: &str, input: &Value, answers: &[(String, String)]) -> String {
    let body = json!({"behavior": "allow", "updatedInput": crate::session::answers_input(input, answers)});
    answer(request_id, body)
}
pub(super) fn deny(request_id: &str) -> String {
    answer(request_id, json!({"behavior": "deny", "message": "The user denied this action."}))
}

fn control(request_id: &str, request: Value) -> String {
    json!({"type": "control_request", "request_id": request_id, "request": request}).to_string()
}

fn answer(request_id: &str, response: Value) -> String {
    json!({
        "type": "control_response",
        "response": {"subtype": "success", "request_id": request_id, "response": response},
    })
    .to_string()
}

pub(super) fn mode_name(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "default",
        PermissionMode::AcceptEdits => "acceptEdits",
        PermissionMode::Plan => "plan",
        PermissionMode::Auto => "auto",
        PermissionMode::Bypass => "bypassPermissions",
    }
}

pub(super) fn mode_from_name(name: &str) -> Option<PermissionMode> {
    Some(match name {
        "default" | "manual" => PermissionMode::Ask,
        "acceptEdits" => PermissionMode::AcceptEdits,
        "plan" => PermissionMode::Plan,
        "auto" => PermissionMode::Auto,
        "bypassPermissions" => PermissionMode::Bypass,
        _ => return None,
    })
}
