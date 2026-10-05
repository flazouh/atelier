//! The entry points: a line in, the events of that line out. Each kind of line is read in the file of its topic.

use std::time::Instant;

use crate::claude_code::wire::Line;
use crate::session::{ChoiceId, Event, RequestId};

use super::super::structs::ClaudeLineMapper;
use super::super::traits::LineMapper;

impl ClaudeLineMapper {
    pub fn new() -> Self {
        Self::default()
    }

    fn parsed(&mut self, line: Line, now: Instant) -> Vec<Event> {
        match line {
            Line::System(system) => self.system(system),
            Line::StreamEvent(stream) => self.stream(stream, now),
            Line::Assistant(message) => self.assistant(message),
            Line::User(message) => {
                let mut events = if message.sidechain || message.written_by_claude() { Vec::new() } else { self.flush_held() };
                events.extend(self.user(message));
                events
            }
            Line::Finished(finish) => self.finished(finish),
            Line::ControlRequest(request) => self.control_request(request),
            Line::ControlResponse(response) => self.control_answered(response),
            Line::ControlCancelRequest { request_id } => self.permission_cancelled(request_id),
            Line::RateLimitEvent(event) => self.limit_told(event),
            Line::Ignored => Vec::new(),
        }
    }
}

impl LineMapper for ClaudeLineMapper {
    fn user_sent(&mut self, id: String) {
        self.message_sent(id);
    }

    fn line(&mut self, line: &str, now: Instant) -> Vec<Event> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        match serde_json::from_str::<Line>(line) {
            Ok(parsed) => self.parsed(parsed, now),
            Err(error) => vec![Event::Warning(format!("a line from the agent did not parse: {error}"))],
        }
    }

    fn answer(&mut self, request: &RequestId, choice: &ChoiceId) -> Option<String> {
        self.answer_line(request, choice)
    }

    fn exited(&mut self, code: Option<i32>, stderr: &str) -> Vec<Event> {
        self.process_exited(code, stderr)
    }

    fn end_of_history(&mut self) -> Vec<Event> {
        self.history_ended()
    }

    fn closed(&mut self) -> Vec<Event> {
        self.session_closed()
    }
}
