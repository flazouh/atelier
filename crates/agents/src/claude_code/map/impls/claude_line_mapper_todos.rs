//! The todo list, kept from the agent's todo and task tools.

use serde_json::Value;

use crate::claude_code::tools::TodoTool;
use crate::session::{Event, Todo, TodoStatus, ToolId};

use super::super::structs::ClaudeLineMapper;

impl ClaudeLineMapper {
    pub(super) fn edit_todos(&mut self, id: ToolId, tool: TodoTool, input: &Value) -> Vec<Event> {
        let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        match tool {
            TodoTool::Write => {
                let items = input.get("todos").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
                self.todos = items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| Todo {
                        id: (i + 1).to_string(),
                        text: text(item, "content").unwrap_or_default(),
                        status: todo_status(text(item, "status").as_deref()),
                    })
                    .collect();
                vec![Event::Todos(self.todos.clone())]
            }
            TodoTool::Create => {
                self.creating.insert(id, text(input, "subject").unwrap_or_default());
                Vec::new()
            }
            TodoTool::Update => {
                let Some(task) = text(input, "taskId") else { return Vec::new() };
                if text(input, "status").as_deref() == Some("deleted") {
                    self.todos.retain(|todo| todo.id != task);
                } else if let Some(todo) = self.todos.iter_mut().find(|todo| todo.id == task) {
                    if let Some(status) = text(input, "status") {
                        todo.status = todo_status(Some(&status));
                    }
                    if let Some(subject) = text(input, "subject") {
                        todo.text = subject;
                    }
                }
                vec![Event::Todos(self.todos.clone())]
            }
        }
    }
}

pub(super) fn todo_status(status: Option<&str>) -> TodoStatus {
    match status {
        Some("in_progress") => TodoStatus::InProgress,
        Some("completed") => TodoStatus::Done,
        _ => TodoStatus::Pending,
    }
}

/// `Task #3 created successfully` gives `3`.
pub(super) fn task_number(text: &str) -> Option<String> {
    let digits: String = text.split("#").nth(1)?.chars().take_while(char::is_ascii_digit).collect();
    (!digits.is_empty()).then_some(digits)
}
