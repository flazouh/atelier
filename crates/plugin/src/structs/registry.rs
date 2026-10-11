use super::PluginView;
use crate::traits::Plugin;

/// What the plugins of the app have added. The app fills it at startup and draws what it holds: take a plugin's line
/// out, and what the plugin added is gone from the app.
#[derive(Clone, Default)]
pub struct Registry {
    views: Vec<PluginView>,
}

impl Registry {
    /// Adds what `plugin` registers.
    pub fn add(&mut self, plugin: &dyn Plugin) {
        plugin.register(self);
    }

    /// Adds a view; one already there with the same id is replaced.
    pub fn add_view(&mut self, view: PluginView) {
        self.views.retain(|have| have.id != view.id);
        self.views.push(view);
    }

    pub fn view(&self, id: &str) -> Option<&PluginView> {
        self.views.iter().find(|view| view.id == id)
    }

    /// Every view, in the order of the rail.
    pub fn views(&self) -> Vec<&PluginView> {
        let mut views: Vec<&PluginView> = self.views.iter().collect();
        views.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(b.id)));
        views
    }
}
