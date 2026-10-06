use std::time::Instant;

use crate::session::{ChoiceId, Event, RequestId};

/// Reads what an agent writes, a line at a time, and gives atelier's events. It keeps what one session
/// needs between lines, reads no clock and touches no process: the caller passes the time with each line.
pub trait LineMapper {
    /// atelier sent a user message, by its id: a turn is open until `claude` reports the result that
    /// takes it. A message sent while a turn runs either folds into that turn or runs after it, as a turn
    /// of its own; atelier shows both as one turn.
    fn user_sent(&mut self, id: String);

    /// Reads one line of `claude`'s stdout. A line that is not JSON gives a warning; a line of a
    /// kind atelier does not know gives nothing.
    fn line(&mut self, line: &str, now: Instant) -> Vec<Event>;

    /// The line to write for a permission answer, or `None` when the request is unknown or already
    /// answered.
    fn answer(&mut self, request: &RequestId, choice: &ChoiceId) -> Option<String>;

    /// The line to write for the reader's answers to a question, or `None` when it is not waiting.
    fn answer_questions(&mut self, request: &RequestId, answers: &[(String, String)]) -> Option<String>;

    /// The events for a process that ended: `code` is its exit code, `None` when a signal ended it. A
    /// turn or a tool still open fails, so the UI never waits for an agent that is gone. A session ends
    /// once: a second call to `exited` or `closed` gives nothing.
    fn exited(&mut self, code: Option<i32>, stderr: &str) -> Vec<Event>;

    /// The end of a transcript that is read as history. A record has no end for a subagent or a background
    /// command that was still going when it stopped being written, and no live agent will ever send one, so
    /// each of them ends here rather than show as running forever. If the last turn ended aborted they
    /// end as interrupted; otherwise as finished, with a note that the record does not say how it went.
    fn end_of_history(&mut self) -> Vec<Event>;

    /// The event for a session atelier closed on purpose: it ends, and nothing else fails.
    fn closed(&mut self) -> Vec<Event>;
}
