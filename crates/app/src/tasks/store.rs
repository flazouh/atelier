//! Opening a project's tracker. A local project keeps its tasks in its data folder, next to its sessions
//! (`<data>/lathe/projects/<folder>-<hash>/tracker.sqlite`), never in the repository. A project over SSH
//! keeps them on its host through lathe-remote, which is the next step (plans/m6-tasks.md, M6b).
use std::sync::Arc;

use lathe_settings::Location;
use gpui_kit::SharedString;
use lathe_project::Project;
use lathe_tracker::{LocalTracker, Tracker, prefix_for};

/// The file of a local project's tasks, in its data folder.
pub const FILE: &str = "tracker.sqlite";

/// The project's tracker, or the words that say why it has none.
pub fn open(location: &Location, project: &dyn Project, name: &str) -> Result<Arc<dyn Tracker>, SharedString> {
    if !matches!(location, Location::Local { .. }) {
        return Err("Tasks for a project over SSH come with the next lathe-remote.".into());
    }
    let Some(dir) = project.data_path() else {
        return Err("This project has no data folder to keep tasks in.".into());
    };
    std::fs::create_dir_all(&dir).map_err(|e| SharedString::from(format!("Could not make the data folder: {e}")))?;
    let tracker = LocalTracker::open(&dir.join(FILE), &prefix_for(name)).map_err(|e| SharedString::from(e.to_string()))?;
    Ok(Arc::new(tracker))
}
