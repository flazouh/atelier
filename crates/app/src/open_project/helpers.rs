use std::sync::Arc;

use atelier_project::Project;

/// The tracker of a project for a session's sake: the pane's if it is open. Else the project has one only if
/// it kept a store, so a session of a project that never used tasks makes none. Blocking.
pub(super) fn find_tracker(open: Option<Arc<dyn atelier_tracker::Tracker>>, project: &Arc<dyn Project>) -> Option<Arc<dyn atelier_tracker::Tracker>> {
    if open.is_some() {
        return open;
    }
    // The data folder says whether the project kept a store, on this machine or on its host.
    let kept = project.data_list("").is_ok_and(|entries| entries.iter().any(|e| e.path == atelier_project::TRACKER_FILE));
    if kept { project.tracker().ok() } else { None }
}
