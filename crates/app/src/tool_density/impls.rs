use crate::settings_pane::strings as words;

use super::types::ToolDensity;

impl ToolDensity {
    pub const ALL: [ToolDensity; 3] = [ToolDensity::Grouped, ToolDensity::Lines, ToolDensity::Detailed];

    pub fn word(self) -> &'static str {
        match self {
            ToolDensity::Grouped => atelier_i18n::t(&words::TOOL_DENSITY_GROUPED),
            ToolDensity::Lines => atelier_i18n::t(&words::TOOL_DENSITY_LINES),
            ToolDensity::Detailed => atelier_i18n::t(&words::TOOL_DENSITY_DETAILED),
        }
    }

    /// As the settings file keeps it.
    pub fn key(self) -> &'static str {
        match self {
            ToolDensity::Grouped => "grouped",
            ToolDensity::Lines => "lines",
            ToolDensity::Detailed => "detailed",
        }
    }

    /// The density a kept key names; a missing or unknown key gives the default.
    pub fn from_key(key: Option<&str>) -> Self {
        Self::ALL.into_iter().find(|d| Some(d.key()) == key).unwrap_or_default()
    }
}

impl gpui_kit::Global for ToolDensity {}
