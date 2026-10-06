//! `system` lines: the session starting, and the tasks `claude` runs: subagents and background commands.

use crate::claude_code::{control::mode_from_name, wire::System};
use crate::session::{Event, SessionId, Started, Subagent, ToolId, TurnEnd, TurnOutcome};

use super::super::structs::ClaudeLineMapper;
use super::claude_line_mapper_tools::tool_output;

impl ClaudeLineMapper {
    pub(super) fn system(&mut self, system: System) -> Vec<Event> {
        if system.is_sign_in_retry() {
            return self.sign_in_refused();
        }
        match system.subtype.as_str() {
            "init" => match system.session_id {
                Some(id) => vec![Event::Started(Started {
                    session: SessionId::new(id),
                    model: system.model,
                    mode: system.permission_mode.as_deref().and_then(mode_from_name),
                    commands: system.slash_commands,
                })],
                None => Vec::new(),
            },
            "task_started" => {
                let Some(id) = system.tool_use_id.map(ToolId::new) else { return Vec::new() };
                // Only an agent task is a subagent. A background shell (`local_bash`), and any other kind of
                // task, is the call that started it, still running.
                if !is_agent_task(system.task_type.as_deref()) {
                    self.background.insert(id);
                    return Vec::new();
                }
                if self.subagents.insert(id.clone()) {
                    self.hidden.insert(id.clone());
                    let subagent = Subagent {
                        id,
                        task: system.description.unwrap_or_default(),
                        kind: system.subagent_type,
                        model: None,
                    };
                    return vec![Event::SubagentStarted(subagent)];
                }
                Vec::new()
            }
            "task_progress" => match (system.tool_use_id, system.description) {
                (Some(id), Some(activity)) => vec![Event::SubagentProgress { id: ToolId::new(id), activity }],
                _ => Vec::new(),
            },
            "task_notification" => {
                let Some(id) = system.tool_use_id.map(ToolId::new) else { return Vec::new() };
                if self.background.remove(&id) {
                    // The call returned long ago; now it ends, with what `claude` says of it.
                    if !self.running.remove(&id) {
                        return Vec::new();
                    }
                    let ok = system.status.as_deref() == Some("completed");
                    let summary = system.summary.unwrap_or_else(|| if ok { "The command finished.".into() } else { "The command did not finish well.".into() });
                    return vec![Event::ToolFinished { id, output: tool_output(&summary, !ok) }];
                }
                self.subagents.remove(&id);
                vec![Event::SubagentEnded { id, ok: system.status.as_deref() == Some("completed"), summary: system.summary }]
            }
            _ => Vec::new(),
        }
    }
}

impl ClaudeLineMapper {
    /// The API refused the sign-in `claude` holds, and `claude` retries for minutes before it says so. The turn ends now,
    /// so the notice offers the sign-in and the reader is not left at "Waiting for Claude…". The retries that follow
    /// find no turn open and tell nothing.
    fn sign_in_refused(&mut self) -> Vec<Event> {
        if !std::mem::take(&mut self.turn_open) {
            return Vec::new();
        }
        self.waiting.clear();
        let why = "Claude did not accept the sign-in.".to_string();
        vec![Event::SignedOut, Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Failed(why), summary: None })]
    }
}

/// Whether a `task_started` names a subagent. A task with no type is an older `claude`'s subagent.
pub(super) fn is_agent_task(task_type: Option<&str>) -> bool {
    task_type.is_none_or(|t| t.contains("agent"))
}
