//! The views of a window, one on screen at a time. The rail holds three lenses: Sessions (every project's
//! sessions and their panels), Tasks (one project's tasks) and Code (one project's pull requests, files and
//! changes). Code is four views: Pulls, Files (the tree and the editor), History (the branch's commits) and
//! Git (the focused session's changed files and their review).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellView {
    Tasks,
    #[default]
    Sessions,
    Git,
    Files,
    Pulls,
    History,
}

impl ShellView {
    /// The name the settings keep it by.
    pub fn words(self) -> &'static str {
        match self {
            Self::Tasks => "tasks",
            Self::Sessions => "sessions",
            Self::Git => "git",
            Self::Files => "files",
            Self::Pulls => "pulls",
            Self::History => "history",
        }
    }

    /// The view the settings name; Sessions for none or an unknown name.
    pub fn from_words(words: Option<&str>) -> Self {
        match words {
            Some("tasks") => Self::Tasks,
            Some("git") => Self::Git,
            Some("files") => Self::Files,
            Some("pulls") => Self::Pulls,
            Some("history") => Self::History,
            _ => Self::Sessions,
        }
    }

    /// The lenses the left rail switches between, in its order: Sessions, Tasks, Code. Code is named by Git.
    pub const ON_RAIL: [Self; 3] = [Self::Sessions, Self::Tasks, Self::Git];

    /// The views of the Code lens, in the order its sidebar lists them.
    pub const IN_CODE: [Self; 4] = [Self::Pulls, Self::Files, Self::History, Self::Git];

    /// The lens on the rail this view is part of.
    pub fn lens(self) -> Self {
        if self.in_code() { Self::Git } else { self }
    }

    /// Whether this is a view of the Code lens, which is about one project.
    pub fn in_code(self) -> bool {
        Self::IN_CODE.contains(&self)
    }
}

/// In a narrow window, the Files view shows one of these at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilesPane {
    #[default]
    Tree,
    Editor,
}
