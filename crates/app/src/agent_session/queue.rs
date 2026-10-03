//! Messages held for after the running turn. Enter sends into a running turn; ⌘↵ queues instead, and
//! the next queued message goes when a turn completes. A Stop or a failure keeps the queue as it is.

use atelier_agents::session::Event;
use gpui_kit::{Context, SharedString};

use super::{AgentSession, chips::Draft, helpers::completes_turn};

impl AgentSession {
    /// Holds `draft` until the running turn completes, or sends it now when no turn runs.
    pub fn queue(&mut self, draft: Draft, cx: &mut Context<Self>) {
        if !self.conversation.working() {
            return self.send_draft(draft, cx);
        }
        self.queued.push(draft);
        self.show_queue(cx);
    }

    pub fn unqueue(&mut self, place: usize, cx: &mut Context<Self>) {
        if place < self.queued.len() {
            self.queued.remove(place);
            self.show_queue(cx);
        }
    }

    /// Sends the queued message at `place` now, into the running turn if one runs.
    pub fn send_queued(&mut self, place: usize, cx: &mut Context<Self>) {
        if place < self.queued.len() {
            let draft = self.queued.remove(place);
            self.show_queue(cx);
            self.send_draft(draft, cx);
        }
    }

    /// Once a turn the reader asked for completes, the next queued message goes. A turn the agent starts
    /// itself, as when a background task it ran finishes, leaves alone the queue a Stop kept.
    pub(super) fn after_turn(&mut self, events: &[Event], cx: &mut Context<Self>) {
        if !events.iter().any(|event| matches!(event, Event::TurnEnded(_))) {
            return;
        }
        if std::mem::take(&mut self.asked_turn) && events.iter().any(completes_turn) {
            self.send_next_queued(cx);
        }
    }

    /// Sends the oldest queued message, once the turn it waited for is over.
    fn send_next_queued(&mut self, cx: &mut Context<Self>) {
        if !self.conversation.working() && !self.queued.is_empty() {
            self.send_queued(0, cx);
        }
    }

    fn show_queue(&self, cx: &mut Context<Self>) {
        let rows: Vec<SharedString> = self.queued.iter().map(|d| SharedString::from(super::chips::shown(&d.text, &d.attachments))).collect();
        self.composer.update(cx, |c, cx| c.set_queued(rows, cx));
    }
}
