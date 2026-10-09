use atelier_ui::{Series, UsageDay, UsageModel, UsageSession, UsageSource, UsageStat};
use gpui_kit::SharedString;

/// Everything the view draws for one state of [`super::UsageState`].
#[derive(Clone, Debug, PartialEq)]
pub struct UsageView {
    pub sources: Vec<UsageSource>,
    pub summary: (SharedString, SharedString),
    pub title: SharedString,
    pub subtitle: SharedString,
    pub provider: Option<SharedString>,
    pub dot: Option<Series>,
    pub stats: Vec<UsageStat>,
    pub days: Vec<UsageDay>,
    pub models: Vec<UsageModel>,
    pub sessions: Vec<UsageSession>,
    pub total: SharedString,
    pub empty: Option<SharedString>,
}
