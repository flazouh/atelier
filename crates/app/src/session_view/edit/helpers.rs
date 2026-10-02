use atelier_agents::session::{Call, ToolStatus};

use super::structs::EditView;
use crate::tool_density::ToolDensity;

/// The diff view of `call`, whose paths are shown relative to `root`; `None` for a call that is not an edit or a write,
/// that has no file yet, that failed (its row shows why), or whose approval was refused. `mark` is the answer its
/// approval left, as `calls::mark` words it.
///
/// A diff is open while it streams. When it ends it folds to its line, unless the density is full detail, which keeps it open.
pub fn edit_view(call: &Call, root: &str, density: ToolDensity, mark: Option<&str>) -> Option<EditView> {
    if matches!(mark, Some("Denied" | "Not answered")) || call.call.status == ToolStatus::Failed {
        return None;
    }
    let preview = crate::session_view::preview::streamed(&call.call, root)?;
    let streaming = matches!(call.call.status, ToolStatus::Pending | ToolStatus::Running);
    let detailed = density == ToolDensity::Detailed;
    Some(EditView { preview, streaming, open: streaming || detailed, fold_when_done: !detailed })
}
