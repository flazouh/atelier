use super::types::ToolDensity;

/// The density in force: the one the Settings page set, or the default while none is set.
pub fn tool_density(cx: &gpui_kit::App) -> ToolDensity {
    cx.try_global::<ToolDensity>().copied().unwrap_or_default()
}
