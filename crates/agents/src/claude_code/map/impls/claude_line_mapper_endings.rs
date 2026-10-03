//! How a session ends: its process exits, its record runs out, or atelier closes it.

use crate::session::{EndReason, Event, ToolOutput, TurnEnd, TurnOutcome};
use crate::subprocess;

use super::super::structs::ClaudeLineMapper;

impl ClaudeLineMapper {
    /// The events for a process that ended: `code` is its exit code, `None` when a signal ended it. A
    /// turn or a tool still open fails, so the UI never waits for an agent that is gone. A session ends
    /// once: a second call to `exited` or `closed` gives nothing.
    pub(super) fn process_exited(&mut self, code: Option<i32>, stderr: &str) -> Vec<Event> {
        if std::mem::replace(&mut self.ended, true) {
            return Vec::new();
        }
        let tail = subprocess::stderr_tail(stderr);
        let why = subprocess::exit_why(code, &tail);
        let mut events = self.flush_held();
        events.extend(self.fail_open_tools(&why, false));
        events.extend(self.asked.drain().map(|(id, _)| Event::PermissionCancelled(id)));
        let mut open: Vec<_> = self.subagents.drain().collect();
        open.sort();
        events.extend(open.into_iter().map(|id| Event::SubagentEnded { id, ok: false, summary: Some(why.clone()) }));
        if std::mem::take(&mut self.turn_open) {
            events.push(Event::TurnEnded(TurnEnd { outcome: TurnOutcome::Failed(why), summary: None }));
        }
        events.push(Event::Ended(EndReason::Exited { code, stderr: tail }));
        events
    }

    /// The end of a transcript that is read as history. A record has no end for a subagent or a background
    /// command that was still going when it stopped being written, and no live agent will ever send one, so
    /// each of them ends here rather than show as running forever. If the last turn ended aborted they
    /// end as interrupted; otherwise as finished, with a note that the record does not say how it went.
    pub(super) fn history_ended(&mut self) -> Vec<Event> {
        let (ok, note) = if self.aborted {
            (false, "Interrupted before it finished.")
        } else {
            (true, "The record has no end for this; it is shown as finished.")
        };
        let mut open: Vec<_> = self.running.drain().collect();
        open.sort();
        let mut events = self.flush_held();
        events.extend(
            open.into_iter()
                .map(|id| Event::ToolFinished { id, output: ToolOutput { text: note.into(), is_error: !ok, truncated: false, full_at: None } }),
        );
        let mut subagents: Vec<_> = self.subagents.drain().collect();
        subagents.sort();
        events.extend(subagents.into_iter().map(|id| Event::SubagentEnded { id, ok, summary: Some(note.into()) }));
        events.extend(self.asked.drain().map(|(id, _)| Event::PermissionCancelled(id)));
        self.background.clear();
        events
    }

    /// The event for a session atelier closed on purpose: it ends, and nothing else fails.
    pub(super) fn session_closed(&mut self) -> Vec<Event> {
        if std::mem::replace(&mut self.ended, true) { Vec::new() } else { vec![Event::Ended(EndReason::Closed)] }
    }
}
