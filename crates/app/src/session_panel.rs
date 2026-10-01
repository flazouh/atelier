//! One session's panel as a view of its own, and its conversation rows as another.
//! Both are drawn afresh for now: cached, either one made a caret blink draw a second frame (see
//! docs/performance.md, "View cache"). Each observes its session, so caching them is one line each.

use gpui_kit::{AppContext, Context, Entity, IntoElement, Render, Subscription, Window};

use crate::{
    agent_session::AgentSession,
    session_view::{rows, session_view_with},
};

pub struct SessionPanel {
    session: Entity<AgentSession>,
    rows: Entity<ConversationRows>,
    _watch: Subscription,
}

impl SessionPanel {
    pub fn new(session: Entity<AgentSession>, cx: &mut Context<Self>) -> Self {
        let _watch = cx.observe(&session, |_, _, cx| cx.notify());
        let rows = cx.new(|cx| ConversationRows::new(session.clone(), cx));
        Self { session, rows, _watch }
    }
}

impl Render for SessionPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Not cached yet: cached, the rows drew a second frame after each caret blink (40 frames in 10 s
        // against 20), for a reason not found. See docs/performance.md, "View cache".
        session_view_with(&self.session, Some(self.rows.clone().into_any_element()), window, cx)
    }
}

/// A session's rows, drawn from their last frame until the session changes.
pub struct ConversationRows {
    session: Entity<AgentSession>,
    _watch: Subscription,
}

impl ConversationRows {
    pub fn new(session: Entity<AgentSession>, cx: &mut Context<Self>) -> Self {
        let _watch = cx.observe(&session, |_, _, cx| cx.notify());
        Self { session, _watch }
    }
}

impl Render for ConversationRows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        rows(&self.session, cx)
    }
}

#[cfg(test)]
mod tests;
