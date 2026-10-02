/// The arguments that name a file.
pub(super) const FILE_KEYS: [&str; 3] = ["file_path", "notebook_path", "path"];

/// The tools that write the todo list. atelier shows the list, not the calls.
#[derive(Clone, Copy)]
pub(in super::super) enum TodoTool {
    /// `TodoWrite` sends the whole list.
    Write,
    Create,
    Update,
}
