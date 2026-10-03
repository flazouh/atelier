//! A turn: when it opens, which messages it still waits on, and how it ends.

use crate::claude_code::wire::Finish;
use crate::session::{ContextFill, Event, TurnEnd, TurnOutcome, Usage};

use super::super::structs::ClaudeLineMapper;
use super::claude_line_mapper_context::context_window;

impl ClaudeLineMapper {
    /// atelier sent a user message, by its id: a turn is open until `claude` reports the result that
    /// takes it. A message sent while a turn runs either folds into that turn or runs after it, as a turn
    /// of its own; atelier shows both as one turn.
    pub(super) fn message_sent(&mut self, id: String) {
        self.turn_open = true;
        self.waiting.push(id);
    }

    /// Whether the turn goes on past a result that `took` these messages: one sent while it ran, and not
    /// folded into it, runs next as a turn of its own. A result that names none took them all; a stop or
    /// a failure ends the turn whatever waits.
    pub(super) fn goes_on(&mut self, outcome: &TurnOutcome, took: Option<&[String]>) -> bool {
        match (outcome, took) {
            (TurnOutcome::Completed, Some(ids)) => self.waiting.retain(|id| !ids.contains(id)),
            _ => self.waiting.clear(),
        }
        !self.waiting.is_empty()
    }

    pub(super) fn finished(&mut self, finish: Finish) -> Vec<Event> {
        let interrupted = matches!(finish.terminal_reason.as_deref(), Some("aborted_tools" | "aborted_streaming"));
        let outcome = if interrupted {
            TurnOutcome::Interrupted
        } else if finish.subtype == "success" && !finish.is_error {
            TurnOutcome::Completed
        } else {
            let why = if finish.errors.is_empty() { finish.result.clone().unwrap_or(finish.subtype) } else { finish.errors.join("; ") };
            TurnOutcome::Failed(why)
        };
        let goes_on = self.goes_on(&outcome, finish.user_message_uuids.as_deref());
        self.turn_open = goes_on;
        self.aborted = interrupted;
        let mut events = self.fail_open_tools("the turn ended before the tool finished", true);
        events.extend(self.asked.drain().map(|(id, _)| Event::PermissionCancelled(id)));
        if let Some(usage) = finish.usage {
            events.push(Event::Usage(Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_input_tokens,
                cache_write_tokens: usage.cache_creation_input_tokens,
                cost_usd: finish.total_cost_usd,
            }));
        }
        if let Some(window) = context_window(&finish.model_usage, self.model.as_deref()) {
            events.extend(self.context_changed(ContextFill { window: Some(window), ..self.context }));
        }
        if !goes_on {
            events.push(Event::TurnEnded(TurnEnd { outcome, summary: finish.result.filter(|text| !text.is_empty()) }));
        }
        events
    }
}
