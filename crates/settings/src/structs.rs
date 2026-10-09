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
    /// The parts of a pull request chip's card the reader hid (`atelier_ui::PrPart::key`).
    pub pr_card_off: Vec<String>,
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
    /// The provider new sessions start on, as `account:<name>` or `openrouter`. Unset: the agent's usual account.
    pub default_provider: Option<String>,
    /// The microphone chosen for dictation, by `atelier_voice::Device::id`. Unset: the system's default.
    pub dictation_device: Option<String>,
    /// Dictation records only while the microphone button is held down, and stops when it is let go.
    pub dictation_hold: Option<bool>,
    /// The key that dictates while held: `fn` (the default), `right-option`, `left-option`, or `off` for none.
    pub dictation_key: Option<String>,
    /// The language of the interface, by its tag (`atelier_i18n::Locale::tag`). Unset: the system's.
    pub language: Option<String>,
    /// How much of the agent's tool calls a session shows: "grouped", "lines" or "detailed". Unset: grouped.
    pub tool_density: Option<String>,
    /// How the sidebar looks, as the Settings page sets it; a field left out is the default.
    pub sidebar_layout: SidebarSaved,
    /// What the app's capabilities do for agents: `{ "capabilities": { "agent_tools": false } }`.
    pub capabilities: CapabilitiesSaved,
    /// The accounts the reader connected, as plain facts. Never a key: those are in the keychain.
    pub accounts: AccountsSaved,
    /// What the reader chose for each agent's models, and the list the agent last reported, by the agent's backend name.
    pub agent_models: std::collections::BTreeMap<String, AgentModels>,
    /// The changelog of the update the app downloaded last, kept so that the first start of that version can show it once.
    pub whats_new: Option<WhatsNew>,
    /// Keys a newer or older atelier wrote, kept as they are.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// The `capabilities` settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapabilitiesSaved {
    /// Agents get the app's tools, such as the tasks, through the gateway. Unset: on.
    pub agent_tools: Option<bool>,
}

/// The `accounts` settings: which outside services the reader connected for their tasks. A key is not here; the Linear
/// entry only says that one is kept, so the app reads the keychain only for a reader who connected Linear.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountsSaved {
    pub linear: Option<LinearSaved>,
    pub github_issues: Option<GithubIssuesSaved>,
}

/// Linear, connected with an API key kept in the keychain (see `secrets::LINEAR_KEY`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinearSaved {
    /// The name of the person the key belongs to, as Linear last said it. Shown while the app checks again.
    pub person: Option<String>,
}

/// GitHub Issues of one repository, read with the reader's `gh` login.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GithubIssuesSaved {
    /// `owner/repo`.
    pub repo: String,
    /// The `gh` login's name, as GitHub last said it.
    pub person: Option<String>,
}

/// An update the app downloaded: its version and its changelog.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhatsNew {
    pub version: String,
    pub notes: String,
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
    /// The provider the session ran on, as the settings keep a provider choice; none for a session saved before this was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl Settings {
    /// Puts `location` first in the recent list, once, keeping at most [`RECENT_LIMIT`].
    pub fn opened(&mut self, location: Location) {
        self.recent.retain(|l| *l != location);
        self.recent.insert(0, location);
        self.recent.truncate(RECENT_LIMIT);
    }
}

/// An agent's models as the reader arranged them: the one new sessions start on, the order they are listed in, and the list the
/// agent last reported (its models change with its releases, so they are not written into the app).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentModels {
    /// The id of the model new sessions start on. Unset: the agent's own default.
    pub default: Option<String>,
    /// Ids, first to last. A model not in it is listed after these, in the agent's order.
    pub order: Vec<String>,
    /// `(id, name)` for each model the agent last reported; empty until it has.
    pub known: Vec<(String, String)>,
    /// Ids the reader left out of the picker. The list in Settings keeps them, to be shown again.
    pub hidden: Vec<String>,
}

impl AgentModels {
    /// The list to offer: what the agent last reported, else `offered` (its built-in list), in the reader's order.
    pub fn arranged(&self, offered: &[(String, String)]) -> Vec<(String, String)> {
        let base: &[(String, String)] = if self.known.is_empty() { offered } else { &self.known };
        let mut out: Vec<(String, String)> = self.order.iter().filter_map(|id| base.iter().find(|(b, _)| b == id).cloned()).collect();
        out.extend(base.iter().filter(|(id, _)| !self.order.contains(id)).cloned());
        out
    }

    /// `list` without what the reader hid. The default is never left out, and a list that would be empty stays whole.
    pub fn visible(&self, list: &[(String, String)]) -> Vec<(String, String)> {
        let shown: Vec<(String, String)> =
            list.iter().filter(|(id, _)| !self.hidden.contains(id) || self.default.as_ref() == Some(id)).cloned().collect();
        if shown.is_empty() { list.to_vec() } else { shown }
    }

    /// The default, when it is a model of `list`.
    pub fn default_in(&self, list: &[(String, String)]) -> Option<String> {
        self.default.clone().filter(|d| list.iter().any(|(id, _)| id == d))
    }
}
