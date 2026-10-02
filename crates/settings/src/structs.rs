use serde::{Deserialize, Serialize};

use super::types::{Location, RECENT_LIMIT};

/// What the Settings page keeps of the sidebar's layout (the mode is `Settings::sidebar`; the filter is not kept).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SidebarSaved {
    /// "auto", "always" or "never".
    pub project_badge: Option<String>,
    pub show_time: Option<bool>,
    pub show_agent_icon: Option<bool>,
    pub fold_after: Option<u8>,
    pub earlier_shown: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The theme's name, as the picker lists it.
    pub theme: Option<String>,
    /// Light, dark, or the system's: `"light"`, `"dark"` or `"system"`. `None`: the theme as it stands.
    pub mode: Option<String>,
    /// The primary colour the reader picked, as red, green and blue bytes. `None`: the theme's own, its ink.
    pub primary: Option<[u8; 3]>,
    /// Newest first.
    pub recent: Vec<Location>,
    /// design preview: remove after Alex picks. The design in force for the editor tabs (0 to 3).
    pub design_tabs: Option<u8>,
    /// design preview: remove after Alex picks. The elevation of floating panels (0 to 3).
    pub design_elevation: Option<u8>,
    /// design preview: remove after Alex picks. How strong the elevation is, 0 to 100.
    pub design_strength: Option<u8>,
    /// The badge colour (an index into the palette) the reader gave a project, by its place. A project with none has
    /// the one its place gives it.
    pub project_colors: std::collections::BTreeMap<String, u8>,
    /// The image file kept for a project's badge, by its place.
    pub project_icons: std::collections::BTreeMap<String, String>,
    /// The ids of the task rules the reader turned off (`atelier_tracker::Rule::id`).
    pub task_rules_off: Vec<String>,
    /// Names the reader gave sessions, by the agent's id for the session.
    pub session_names: std::collections::BTreeMap<String, String>,
    /// The sessions the reader archived, by the agent's id for each. An archived session leaves the list until the
    /// filter asks for it.
    pub archived_sessions: Vec<String>,
    /// How the agent panels were laid out last.
    pub panels: Panels,
    /// The sessions open at quit, in their panels' order, for the next launch to open again.
    pub open: Vec<OpenSession>,
    /// The session in front at quit, by the agent's id.
    pub front: Option<String>,
    /// The window's view at quit: "sessions" or "files".
    pub view: Option<String>,
    /// How the sidebar lists sessions: "projects" or "priority".
    pub sidebar: Option<String>,
    /// How far the interface is zoomed (1 is as designed), set by ⌘+, ⌘− and ⌘0.
    pub ui_zoom: Option<f32>,
    /// A skill picked from the composer's `/` list runs at once; unset or false, it waits in the box.
    pub run_picked_skills: Option<bool>,
    /// The microphone chosen for dictation, by `atelier_voice::Device::id`. Unset: the system's default.
    pub dictation_device: Option<String>,
    /// Dictation records only while the microphone button is held down, and stops when it is let go.
    pub dictation_hold: Option<bool>,
    /// The key that dictates while held: `fn` (the default), `right-option` or `left-option`.
    pub dictation_key: Option<String>,
    /// How much of the agent's tool calls a session shows: "grouped", "lines" or "detailed". Unset: grouped.
    pub tool_density: Option<String>,
    /// How the sidebar looks, as the Settings page sets it; a field left out is the default.
    pub sidebar_layout: SidebarSaved,
    /// Keys a newer or older atelier wrote, kept as they are.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// The agent panels' layout, as the window left it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Panels {
    /// One panel with tabs, rather than side by side.
    pub single: bool,
    /// Grouped by project.
    pub grouped: bool,
    /// Each panel's width, by its session.
    pub widths: Vec<(String, f32)>,
}

/// A session open at quit: its project, the agent's id for it, its title as the panel showed it, and the
/// backend of the agent that runs it (`None` in a file from before, for the default agent).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenSession {
    pub location: Location,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

impl Settings {
    /// Puts `location` first in the recent list, once, keeping at most [`RECENT_LIMIT`].
    pub fn opened(&mut self, location: Location) {
        self.recent.retain(|l| *l != location);
        self.recent.insert(0, location);
        self.recent.truncate(RECENT_LIMIT);
    }
}
