//! The views of a window, one on screen at a time. The rail holds the app's own three, Sessions (every project's
//! sessions and their panels), Tasks (one project's tasks) and Code (one project's pull requests, files and
//! changes), and then one entry for each view a plugin registered. Code is four views: Pulls, Files (the tree and the
//! editor), History (the branch's commits) and Git (the focused session's changed files and their review).

use crate::slots::Slots;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellView {
    Tasks,
    #[default]
    Sessions,
    Git,
    Files,
    Pulls,
    History,
    /// A view a plugin registered, by its id. Its page fills the sidebar and the main area.
    Plugin(&'static str),
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
            Self::Plugin(id) => id,
        }
    }

    /// The view the settings name: the view a plugin registered under that name, else the app's own of that name.
    /// A name nobody has (a plugin taken out since) is Sessions.
    pub fn saved(words: Option<&str>, slots: Option<&Slots>) -> Self {
        let registered = words.zip(slots).and_then(|(words, slots)| slots.view(words));
        registered.map_or_else(|| Self::from_words(words), |view| Self::Plugin(view.id))
    }

    /// The app's own view of that name; Sessions for none or an unknown name.
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

    /// The app's own entries of the left rail, in its order: Sessions, Tasks, Code. Code is named by Git. The views
    /// the plugins registered stand after them.
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
