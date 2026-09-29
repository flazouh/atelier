//! The lines lathe writes to `claude`'s stdin. Each is one JSON object; the caller adds the line end.
use serde_json::{Value, json};

use crate::session::{Attachment, PermissionMode, message_text};

pub(super) fn user_message(text: &str, attachments: &[Attachment]) -> String {
    let content = message_text(text, attachments);
    json!({"type": "user", "message": {"role": "user", "content": content}}).to_string()
}

pub(super) fn interrupt(request_id: &str) -> String {
    control(request_id, json!({"subtype": "interrupt"}))
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
