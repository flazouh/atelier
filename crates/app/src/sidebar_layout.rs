//! The sidebar's layout as the settings file keeps it: the mapping between [`beui::sidebar_layout::SidebarLayout`] and
//! the settings' [`atelier_settings::SidebarSaved`] and mode. The one place that knows how the layout is stored.
use beui::{
    sidebar_layout::{BadgeShow, SidebarLayout},
    sidebar_model::ListMode,
};
use atelier_settings::{Settings, SidebarSaved};

/// The layout the settings kept, with the defaults for what they did not. The filter always starts as Active.
pub fn from_settings(settings: &Settings) -> SidebarLayout {
    let d = SidebarLayout::default();
    let saved = &settings.sidebar_layout;
    SidebarLayout {
        mode: ListMode::from_key(settings.sidebar.as_deref()),
        filter: d.filter,
        project_badge: BadgeShow::from_key(saved.project_badge.as_deref()),
        show_time: saved.show_time.unwrap_or(d.show_time),
        show_agent_icon: saved.show_agent_icon.unwrap_or(d.show_agent_icon),
        fold_after: saved.fold_after.map_or(d.fold_after, usize::from),
        earlier_shown: saved.earlier_shown.map_or(d.earlier_shown, usize::from),
    }
}

/// What the Settings page keeps of `layout`.
pub fn saved_look(layout: &SidebarLayout) -> SidebarSaved {
    SidebarSaved {
        project_badge: Some(layout.project_badge.key().to_string()),
        show_time: Some(layout.show_time),
        show_agent_icon: Some(layout.show_agent_icon),
        fold_after: u8::try_from(layout.fold_after).ok(),
        earlier_shown: u8::try_from(layout.earlier_shown).ok(),
    }
}

#[cfg(test)]
mod tests;
