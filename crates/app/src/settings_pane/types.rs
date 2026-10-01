use atelier_ui::{sidebar_layout::SidebarLayout, theme::{Appearance, follow_system, set_appearance}};

/// Light, dark, or whatever the system is set to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
    System,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Light, Mode::Dark, Mode::System];

    pub fn word(self) -> &'static str {
        match self {
            Mode::Light => "Light",
            Mode::Dark => "Dark",
            Mode::System => "System",
        }
    }

    /// As settings keep it.
    pub fn key(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
            Mode::System => "system",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }

    /// Puts the mode in force: the current family's light or dark theme, or the system's.
    pub fn apply(self, cx: &mut gpui_kit::App) {
        match self {
            Mode::Light => set_appearance(Appearance::Light, cx),
            Mode::Dark => set_appearance(Appearance::Dark, cx),
            Mode::System => follow_system(cx),
        }
    }
}

/// The primary colours offered, besides the default (the theme's ink): name, colour as bytes, words. They are
/// the reader's data, not the interface's colours.
pub const PRIMARIES: [(&str, [u8; 3], &str); 8] = [
    ("blue", [2, 133, 247], "Blue"),
    ("purple", [146, 112, 232], "Purple"),
    ("pink", [230, 106, 164], "Pink"),
    ("red", [229, 86, 86], "Red"),
    ("orange", [237, 145, 65], "Orange"),
    ("amber", [229, 182, 60], "Amber"),
    ("green", [101, 166, 90], "Green"),
    ("teal", [22, 157, 131], "Teal"),
];

pub enum SettingsEvent {
    Close,
    /// The sidebar's look changed (what a row shows, how much folds): the new layout, whose mode and filter the shell ignores.
    Sidebar(SidebarLayout),
}

/// The sections of the page, in the order the list at its left shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Appearance,
    Sidebar,
    Agents,
    Tasks,
    Keys,
}

impl Section {
    pub const ALL: [Section; 5] = [Section::Appearance, Section::Sidebar, Section::Agents, Section::Tasks, Section::Keys];

    pub fn words(self) -> &'static str {
        match self {
            Section::Appearance => "Appearance",
            Section::Sidebar => "Sidebar",
            Section::Agents => "Agents",
            Section::Tasks => "Tasks",
            Section::Keys => "Keys",
        }
    }

    /// One line under the section's name.
    pub fn gist(self) -> &'static str {
        match self {
            Section::Appearance => "The theme, light or dark, and the colour of the main button.",
            Section::Sidebar => "What a session row shows, and how many sessions the sidebar shows before it folds the rest.",
            Section::Agents => "How a picked skill runs, the agents this build can start, and the models each offers.",
            Section::Tasks => "What moves a task by itself. Every move shows in its activity, and you can move it back.",
            Section::Keys => "The keys of the review. They cannot be changed yet.",
        }
    }

    /// The name a test finds the section's entry in the list by.
    pub fn entry(self) -> &'static str {
        match self {
            Section::Appearance => "section-appearance",
            Section::Sidebar => "section-sidebar",
            Section::Agents => "section-agents",
            Section::Tasks => "section-tasks",
            Section::Keys => "section-keys",
        }
    }
}
