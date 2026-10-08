use atelier_ui::{UsageDay, UsageModel, UsageSession, UsageSource};
use gpui_kit::SharedString;
/// Everything the dashboard draws for one state of [`super::UsageState`].
#[derive(Clone, Debug, PartialEq)]
pub struct UsageView {
    pub sources: Vec<UsageSource>,
    pub summary: (SharedString, SharedString),
    pub days: Vec<UsageDay>,
    pub models: Vec<UsageModel>,
    pub sessions: Vec<UsageSession>,
    pub total: SharedString,
    pub empty: Option<SharedString>,
}
