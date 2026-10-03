//! Tool calls: from the call being announced to its result.

use serde_json::Value;

use crate::claude_code::tools::self;
use crate::session::{Event, Subagent, Todo, TodoStatus, ToolCall, ToolId, ToolOutput, ToolStatus};

use super::super::structs::ClaudeLineMapper;
use super::claude_line_mapper_todos::task_number;

impl ClaudeLineMapper {
    pub(super) fn announce(&mut self, id: ToolId, name: String, input: Value, parent: Option<ToolId>) -> Vec<Event> {
        self.running.insert(id.clone());
        vec![Event::ToolStarted(ToolCall {
            id,
            kind: tools::kind(&name),
            file: tools::file(&input),
            name,
            input,
            parent,
            status: ToolStatus::Running,
        })]
    }

    /// A tool call with its whole input, from a finished message.
    pub(super) fn tool_use(&mut self, id: ToolId, name: String, input: Value, parent: Option<ToolId>) -> Vec<Event> {
        if tools::starts_subagent(&name) {
            self.hidden.insert(id.clone());
            if !self.subagents.insert(id.clone()) {
                return Vec::new();
            }
            let text = |key: &str| input.get(key).and_then(Value::as_str).map(str::to_string);
            let task = text("description").or_else(|| text("prompt")).unwrap_or_default();
            return vec![Event::SubagentStarted(Subagent { id, task, kind: text("subagent_type"), model: text("model") })];
        }
        if let Some(tool) = tools::todo_tool(&name) {
            self.hidden.insert(id.clone());
            return self.edit_todos(id, tool, &input);
        }
        let edit = tools::edit_of(&name, &input).map(|edit| Event::ToolEdit { id: id.clone(), edit });
        let mut events = if self.running.contains(&id) {
            vec![Event::ToolInput { id, file: tools::file(&input), input }]
        } else {
            self.announce(id, name, input, parent)
        };
        events.extend(edit);
        events
    }

    pub(super) fn tool_result(&mut self, id: ToolId, content: &Value, is_error: bool, detail: Option<Value>) -> Vec<Event> {
        if let Some(subject) = self.creating.remove(&id) {
            self.hidden.remove(&id);
            let task = detail.as_ref().and_then(|d| d.get("task")?.get("id")?.as_str().map(str::to_string));
            let task = task.or_else(|| task_number(&flatten(content)));
            if is_error || task.is_none() {
                return Vec::new();
            }
            self.todos.push(Todo { id: task.unwrap_or_default(), text: subject, status: TodoStatus::Pending });
            return vec![Event::Todos(self.todos.clone())];
        }
        if self.hidden.remove(&id) {
            return Vec::new();
        }
        // A background command's result only says it started. The call stays running until it ends.
        if self.background.contains(&id) && self.running.contains(&id) {
            return Vec::new();
        }
        if !self.running.remove(&id) {
            return Vec::new();
        }
        vec![Event::ToolFinished { id, output: tool_output(&flatten(content), is_error) }]
    }

    /// Ends the calls still running with an error. A background command outlives the turn that started it,
    /// so `keep_background` leaves it running; a process that ended takes it down too.
    pub(super) fn fail_open_tools(&mut self, why: &str, keep_background: bool) -> Vec<Event> {
        let mut open: Vec<_> = self.running.iter().filter(|id| !(keep_background && self.background.contains(*id))).cloned().collect();
        for id in &open {
            self.running.remove(id);
        }
        open.sort();
        open.into_iter()
            .map(|id| Event::ToolFinished {
                id,
                output: ToolOutput { text: why.to_string(), is_error: true, truncated: false, full_at: None },
            })
            .collect()
    }
}

/// A tool result's text: a string, or the text blocks of a list.
pub(super) fn flatten(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block.get("text")?.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// `claude` keeps an output it finds too large in a file and puts a preview and the path in the
/// result. atelier passes both on. An output that is large and not kept is cut to its head.
pub(super) fn tool_output(text: &str, is_error: bool) -> ToolOutput {
    const OPEN: &str = "<persisted-output>";
    const SAVED: &str = "Full output saved to: ";
    const PREVIEW: &str = "Preview";
    if let Some(body) = text.strip_prefix(OPEN) {
        let full_at = body.split_once(SAVED).and_then(|(_, rest)| rest.lines().next()).map(str::to_string);
        let preview = body
            .split_once(PREVIEW)
            .and_then(|(_, rest)| rest.split_once('\n'))
            .map_or("", |(_, preview)| preview.trim_end().trim_end_matches("</persisted-output>").trim_end_matches("...").trim_end());
        return ToolOutput { truncated: true, full_at, ..ToolOutput::head(preview, is_error) };
    }
    ToolOutput::head(text, is_error)
}
