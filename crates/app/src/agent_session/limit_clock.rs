//! While the account is at its usage limit, the session repaints each minute: the box over the composer
//! counts down to the reset and goes once the reset passes.

use std::time::Duration;

use atelier_agents::session::LimitState;
use gpui_kit::{Context, Task};

use super::{AgentSession, now};

const TICK: Duration = Duration::from_secs(60);

impl AgentSession {
    pub(super) fn watch_limit(&mut self, cx: &mut Context<Self>) {
        if self.reset_at().is_none() {
            self.limit_clock = Task::ready(());
            return;
        }
        self.limit_clock = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let Ok(waiting) = this.update(cx, |this, cx| {
                    cx.notify();
                    this.reset_at().is_some_and(|at| now() < at)
                }) else {
                    break;
                };
                if !waiting {
                    break;
                }
            }
        });
    }

    /// When the limit the account reached resets, in seconds since the Unix epoch.
    fn reset_at(&self) -> Option<u64> {
        self.conversation.limit().filter(|limit| limit.state == LimitState::Reached)?.resets_at
    }
}
