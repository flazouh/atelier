use atelier_ui::tool_preview::ToolPreview;

/// What the list draws for an edit: its diff, and how the diff opens and folds.
#[derive(Clone, Debug, PartialEq)]
pub struct EditView {
    /// The diff, under the path relative to the project.
    pub preview: ToolPreview,
    /// The agent is still writing it.
    pub streaming: bool,
    /// The diff is open when it first draws. It is open while it streams, whatever the density.
    pub open: bool,
    /// It folds by itself when the agent finishes it.
    pub fold_when_done: bool,
}
