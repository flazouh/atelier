use std::path::PathBuf;

use atelier_plugin::{Plugin, PluginView, Registry};
use atelier_ui::IconName;

use super::BotsPage;
use super::super::helpers::reader_root;

/// The Bots plugin: the bot library as a view of the app, with its entry on the rail. Without its line in the registry
/// the rail has no Bots, and a view named bots opens nothing.
pub struct BotsPlugin {
    root: Option<PathBuf>,
}

impl BotsPlugin {
    /// The plugin on the reader's own bots, kept next to the settings file.
    pub fn for_reader() -> Self {
        Self::at(reader_root())
    }

    /// The plugin on the bots kept in `root`. With no folder the view says so and reads nothing.
    pub fn at(root: Option<PathBuf>) -> Self {
        Self { root }
    }
}

impl Plugin for BotsPlugin {
    fn register(&self, registry: &mut Registry) {
        let root = self.root.clone();
        registry.add_view(PluginView::new("bots", IconName::Bot, "Bots", 90, move |host, cx| BotsPage::new(root.clone(), host.clone(), cx)));
    }
}
