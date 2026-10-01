//! The views of a window, one on screen at a time (`plans/views-and-commands.md`): Sessions (the
//! session list, the agent panels, and the review, the pull requests or the tasks when one is asked for) and
//! Files (the file tree and the editor), and Team (every person's agent sessions in lanes).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellView {
    #[default]
    Sessions,
    Files,
    /// Who is doing what: one lane for each person with their agent sessions.
    Team,
}

impl ShellView {
    /// The name the settings keep it by.
    pub fn words(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::Files => "files",
            Self::Team => "team",
        }
    }

    /// The view the settings name; Sessions for none or an unknown name.
    pub fn from_words(words: Option<&str>) -> Self {
        match words {
            Some("files") => Self::Files,
            Some("team") => Self::Team,
            _ => Self::Sessions,
        }
    }
}

/// In a narrow window, the Files view shows one of these at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilesPane {
    #[default]
    Tree,
    Editor,
}
