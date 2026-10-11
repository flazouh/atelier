use atelier_plugin::{Plugin, PluginView, Registry};
use atelier_ui::IconName;

use super::UsagePage;

/// The Usage plugin: the dashboard as a view of the app, with its entry on the rail. Without it the rail has no Usage,
/// and a view named usage opens nothing.
pub struct UsagePlugin;

impl Plugin for UsagePlugin {
    fn register(&self, registry: &mut Registry) {
        registry.add_view(PluginView::new("usage", IconName::BarChart, "Usage", 100, |host, cx| UsagePage::new(host.clone(), cx)));
    }
}
