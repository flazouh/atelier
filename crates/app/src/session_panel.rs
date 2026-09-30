//! One session's panel as a view of its own, so the panels draw it from its last frame until its session
//! changes (`plans/view-cache.md`). It observes the session; the composer and the other inputs inside are
//! entities that redraw themselves.

use gpui_kit::{Context, Entity, IntoElement, Render, Subscription, Window};

use crate::{agent_session::AgentSession, session_view::session_view};

pub struct SessionPanel {
    session: Entity<AgentSession>,
    _watch: Subscription,
}

impl SessionPanel {
    pub fn new(session: Entity<AgentSession>, cx: &mut Context<Self>) -> Self {
        let _watch = cx.observe(&session, |_, _, cx| cx.notify());
        Self { session, _watch }
    }
}

impl Render for SessionPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        session_view(&self.session, window, cx)
    }
}
