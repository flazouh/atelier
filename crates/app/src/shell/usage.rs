//! The usage dashboard's state: opened by a press on the usage chips in the status bar. The logs are read off the UI
//! thread, and what was read stays for the next opening, so the dashboard shows at once and then refreshes.
use std::sync::Arc;
use atelier_agents::usage_history::read_cached;
use atelier_ui::{Selection, UsageRange};
use gpui_kit::{Context, SharedString};
use super::structs::Shell;
use crate::usage_view::{self, UsageState, consts_longest};
impl Shell {
    /// Opens the dashboard with what was read last time, and reads the logs again.
    pub fn show_usage(&mut self, cx: &mut Context<Self>) {
        self.usage = Some(UsageState::new(self.usage_seen.clone()));
        self.read_usage(cx);
        cx.notify();
    }
    pub fn close_usage(&mut self, cx: &mut Context<Self>) {
        self.usage = None;
        cx.notify();
    }
    pub fn set_usage_range(&mut self, range: UsageRange, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.usage {
            state.range = range;
            state.expanded = None;
            cx.notify();
        }
    }
    pub fn select_usage(&mut self, selection: Selection, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.usage {
            state.selection = selection;
            state.expanded = None;
            cx.notify();
        }
    }
    pub fn expand_usage(&mut self, session: Option<SharedString>, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.usage {
            state.expanded = session;
            cx.notify();
        }
    }
    /// Reads the session logs of the last month in the background; a file that did not change is not read again.
    fn read_usage(&mut self, cx: &mut Context<Self>) {
        let (cache, roots) = (self.usage_cache.clone(), self.usage_roots.clone());
        let (today, offset) = usage_view::today();
        cx.spawn(async move |this, cx| {
            let history = cx
                .background_executor()
                .spawn(async move {
                    let since = today.minus_days(i64::from(consts_longest()));
                    let mut cache = cache.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    read_cached(&roots, since, offset, &mut cache)
                })
                .await;
            drop(this.update(cx, |shell, cx| {
                let history = Arc::new(history);
                shell.usage_seen = history.clone();
                if let Some(state) = &mut shell.usage {
                    state.history = history;
                    state.loading = false;
                }
                cx.notify();
            }));
        })
        .detach();
    }
}
