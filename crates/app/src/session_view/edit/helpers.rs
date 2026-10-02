use atelier_agents::session::{Call, ToolKind, ToolStatus};
use atelier_ui::tool_preview::{ToolPreview, relative_path};

use super::structs::EditView;
use crate::tool_density::ToolDensity;

/// The diff view of `call`, whose paths are shown relative to `root`: its shared edit, drawn as its kind says. `None`
/// for a call whose agent told no edit, that failed (its row shows why), or whose approval was refused. `mark` is the answer its
/// approval left, as `calls::mark` words it.
///
/// A diff is open while it streams. When it ends it folds to its line, unless the density is full detail, which keeps it open.
pub fn edit_view(call: &Call, root: &str, density: ToolDensity, mark: Option<&str>) -> Option<EditView> {
    if matches!(mark, Some("Denied" | "Not answered")) || call.call.status == ToolStatus::Failed {
        return None;
    }
    let edit = call.edit.as_ref()?;
    let path = relative_path(&edit.path, root);
    let preview = match call.call.kind {
        ToolKind::Write => ToolPreview::written(path, edit.new.clone()),
        _ => ToolPreview::edit(path, edit.old.clone(), edit.new.clone()),
    };
    let streaming = matches!(call.call.status, ToolStatus::Pending | ToolStatus::Running);
    let detailed = density == ToolDensity::Detailed;
    Some(EditView { preview, streaming, open: streaming || detailed, fold_when_done: !detailed })
}
