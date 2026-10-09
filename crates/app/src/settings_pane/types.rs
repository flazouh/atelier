use atelier_i18n::{Message, t};
use atelier_ui::{sidebar_layout::SidebarLayout, theme::{Appearance, follow_system, set_appearance}};

use super::strings as words;

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
            Mode::Light => t(&words::MODE_LIGHT),
            Mode::Dark => t(&words::MODE_DARK),
            Mode::System => t(&words::MODE_SYSTEM),
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
        cx.set_global(InForce(self));
        match self {
            Mode::Light => set_appearance(Appearance::Light, cx),
            Mode::Dark => set_appearance(Appearance::Dark, cx),
            Mode::System => follow_system(cx),
        }
    }

    /// The system turned light or dark: the app turns with it while the mode is System.
    pub fn system_changed(appearance: gpui_kit::WindowAppearance, cx: &mut gpui_kit::App) {
        if cx.try_global::<InForce>().is_none_or(|m| m.0 == Mode::System) {
            set_appearance(Appearance::of_system(appearance), cx);
        }
    }
}

/// The mode last put in force, which a change of the system's appearance reads.
struct InForce(Mode);

impl gpui_kit::Global for InForce {}

/// The primary colours offered, besides the default (the theme's ink): name, colour as bytes, words. They are
/// the reader's data, not the interface's colours. The words are read in the current language, with [`t`].
pub const PRIMARIES: [(&str, [u8; 3], &Message); 8] = [
    ("blue", [2, 133, 247], &words::COLOUR_BLUE),
    ("purple", [146, 112, 232], &words::COLOUR_PURPLE),
    ("pink", [230, 106, 164], &words::COLOUR_PINK),
    ("red", [229, 86, 86], &words::COLOUR_RED),
    ("orange", [237, 145, 65], &words::COLOUR_ORANGE),
    ("amber", [229, 182, 60], &words::COLOUR_AMBER),
    ("green", [101, 166, 90], &words::COLOUR_GREEN),
    ("teal", [22, 157, 131], &words::COLOUR_TEAL),
];

pub enum SettingsEvent {
    Close,
    /// The sidebar's look changed (what a row shows, how much folds): the new layout, whose mode and filter the shell ignores.
    Sidebar(SidebarLayout),
    /// The interface font size was chosen: the zoom it takes, which the shell sets and keeps.
    Zoom(f32),
}

/// The sections of the page, in the order the list at its left shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Appearance,
    Sidebar,
    Agents,
    Providers,
    Accounts,
    Dictation,
    Tasks,
    PullRequests,
    Keys,
}

impl Section {
    pub const ALL: [Section; 9] = [Section::Appearance, Section::Sidebar, Section::Agents, Section::Providers, Section::Accounts, Section::Dictation, Section::Tasks, Section::PullRequests, Section::Keys];

    pub fn words(self) -> &'static str {
        match self {
            Section::Appearance => t(&words::SECTION_APPEARANCE),
            Section::Sidebar => t(&words::SECTION_SIDEBAR),
            Section::Agents => t(&words::SECTION_AGENTS),
            Section::Providers => "Providers",
            Section::Accounts => "Accounts",
            Section::Dictation => t(&words::SECTION_DICTATION),
            Section::Tasks => t(&words::SECTION_TASKS),
            Section::PullRequests => t(&words::SECTION_PULL_REQUESTS),
            Section::Keys => t(&words::SECTION_KEYS),
        }
    }

    /// One line under the section's name.
    pub fn gist(self) -> &'static str {
        match self {
            Section::Appearance => t(&words::GIST_APPEARANCE),
            Section::Sidebar => t(&words::GIST_SIDEBAR),
            Section::Agents => t(&words::GIST_AGENTS),
            Section::Providers => "Where Claude Code gets its model. A session picks one beside its agent.",
            Section::Accounts => "Linear and GitHub Issues, for the Tasks screen and your agents.",
            Section::Dictation => t(&words::GIST_DICTATION),
            Section::Tasks => t(&words::GIST_TASKS),
            Section::PullRequests => t(&words::GIST_PULL_REQUESTS),
            Section::Keys => t(&words::GIST_KEYS),
        }
    }

    /// The name a test finds the section's entry in the list by.
    pub fn entry(self) -> &'static str {
        match self {
            Section::Appearance => "section-appearance",
            Section::Sidebar => "section-sidebar",
            Section::Agents => "section-agents",
            Section::Providers => "section-providers",
            Section::Accounts => "section-accounts",
            Section::Dictation => "section-dictation",
            Section::Tasks => "section-tasks",
            Section::PullRequests => "section-pull-requests",
            Section::Keys => "section-keys",
        }
    }
}

/// The dictation key's choices, as the page offers them: each key this system hears, then none.
pub fn dictation_keys() -> Vec<Option<atelier_voice::hotkey::Key>> {
    atelier_voice::hotkey::Key::ALL.iter().copied().map(Some).chain([None]).collect()
}
