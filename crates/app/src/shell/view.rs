//! The views of a window, one on screen at a time, in the order of a change's life: Tasks (the
//! board), Sessions (the session list, the agent panels, and the pull requests or the tasks when one
//! is asked for), Git (the focused session's changed files and their review), and Files (the file
//! tree and the editor).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellView {
    Tasks,
    #[default]
    Sessions,
    Git,
    Files,
}

impl ShellView {
    /// The name the settings keep it by.
    pub fn words(self) -> &'static str {
        match self {
            Self::Tasks => "tasks",
            Self::Sessions => "sessions",
            Self::Git => "git",
            Self::Files => "files",
        }
    }

    /// The view the settings name; Sessions for none or an unknown name.
    pub fn from_words(words: Option<&str>) -> Self {
        match words {
            Some("tasks") => Self::Tasks,
            Some("git") => Self::Git,
            Some("files") => Self::Files,
            _ => Self::Sessions,
        }
    }

    /// The views the left rail switches between, in its order.
    pub const ON_RAIL: [Self; 3] = [Self::Tasks, Self::Sessions, Self::Git];
}

/// In a narrow window, the Files view shows one of these at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilesPane {
    #[default]
    Tree,
    Editor,
}
