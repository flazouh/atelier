//! Messages held for after the running turn. Enter sends into a running turn; ⌘↵ queues instead, and
//! the next queued message goes when a turn completes. A Stop or a failure keeps the queue as it is.

use gpui_kit::{Context, SharedString};

use super::AgentSession;

impl AgentSession {
    /// Holds `text` until the running turn completes, or sends it now when no turn runs.
    pub fn queue(&mut self, text: String, cx: &mut Context<Self>) {
        if !self.conversation.working() {
            return self.send(text, cx);
        }
        self.queued.push(text);
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
            let text = self.queued.remove(place);
            self.show_queue(cx);
            self.send(text, cx);
        }
    }

    /// Sends the oldest queued message, once the turn it waited for is over.
    pub(super) fn send_next_queued(&mut self, cx: &mut Context<Self>) {
        if !self.conversation.working() && !self.queued.is_empty() {
            self.send_queued(0, cx);
        }
    }

    fn show_queue(&self, cx: &mut Context<Self>) {
        let rows: Vec<SharedString> = self.queued.iter().cloned().map(SharedString::from).collect();
        self.composer.update(cx, |c, cx| c.set_queued(rows, cx));
    }
}
