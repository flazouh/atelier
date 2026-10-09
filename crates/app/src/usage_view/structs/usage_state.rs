use std::{cell::RefCell, sync::Arc};
use atelier_agents::usage_history::UsageHistory;
use atelier_ui::{Selection, UsageRange};
use gpui_kit::SharedString;
use super::UsageView;
/// The dashboard while it is open: what the reader chose, and the logs read so far.
pub struct UsageState {
    pub range: UsageRange,
    pub selection: Selection,
    pub expanded: Option<SharedString>,
    pub history: Arc<UsageHistory>,
    /// The logs are being read; the first reading has not come yet when `history` is empty.
    pub loading: bool,
    /// The view last built and what it was built from, so a redraw that changed nothing builds nothing.
    pub(in crate::usage_view) built: RefCell<Option<(String, UsageView)>>,
}
impl UsageState {
    pub fn new(history: Arc<UsageHistory>) -> Self {
        Self { range: UsageRange::Fortnight, selection: Selection::All, expanded: None, history, loading: true, built: RefCell::new(None) }
    }
}
