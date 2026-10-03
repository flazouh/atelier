//! Permission questions: asked by the agent, answered by the reader.

use crate::claude_code::{control::self, tools::self, wire::{CanUseTool, ControlBody, ControlRequest}};
use crate::session::{Choice, ChoiceId, ChoiceKind, Event, PermissionRequest, RequestId, ToolCall, ToolId, ToolStatus};

use super::super::structs::ClaudeLineMapper;
use super::super::structs::Asked;
use super::super::consts::{ALLOW_ALWAYS, ALLOW, DENY};

impl ClaudeLineMapper {
    pub(super) fn control_request(&mut self, request: ControlRequest) -> Vec<Event> {
        let ControlBody::CanUseTool(ask) = request.request else { return Vec::new() };
        let CanUseTool { tool_name, input, tool_use_id, description, permission_suggestions } = *ask;
        let id = RequestId::new(request.request_id);
        let has_rules = permission_suggestions.as_ref().is_some_and(|rules| rules.as_array().is_none_or(|a| !a.is_empty()));
        let mut choices = vec![Choice { id: ChoiceId::new(ALLOW), label: "Allow".into(), kind: ChoiceKind::Allow }];
        if has_rules {
            choices.push(Choice { id: ChoiceId::new(ALLOW_ALWAYS), label: "Always allow".into(), kind: ChoiceKind::AllowAlways });
        }
        choices.push(Choice { id: ChoiceId::new(DENY), label: "Deny".into(), kind: ChoiceKind::Deny });
        let call = ToolCall {
            id: ToolId::new(tool_use_id.unwrap_or_else(|| id.as_str().to_string())),
            kind: tools::kind(&tool_name),
            file: tools::file(&input),
            name: tool_name,
            input: input.clone(),
            parent: None,
            status: ToolStatus::Pending,
        };
        self.asked.insert(id.clone(), Asked { input, suggestions: permission_suggestions.filter(|_| has_rules) });
        vec![Event::Permission(PermissionRequest { id, call, reason: description.filter(|d| !d.is_empty()), choices })]
    }

    /// The line to write for a permission answer, or `None` when the request is unknown or already
    /// answered.
    pub(super) fn answer_line(&mut self, request: &RequestId, choice: &ChoiceId) -> Option<String> {
        let asked = self.asked.remove(request)?;
        Some(match choice.as_str() {
            ALLOW => control::allow(request.as_str(), &asked.input, None),
            ALLOW_ALWAYS => control::allow(request.as_str(), &asked.input, asked.suggestions.as_ref()),
            _ => control::deny(request.as_str()),
        })
    }

    pub(super) fn permission_cancelled(&mut self, request_id: String) -> Vec<Event> {
        let id = RequestId::new(request_id);
        if self.asked.remove(&id).is_some() { vec![Event::PermissionCancelled(id)] } else { Vec::new() }
    }
}
