use atelier_ui::sidebar_layout::BadgeShow;

use super::*;

/// What the settings never held is the default, and a stranger in the file is too.
#[test]
fn an_empty_settings_file_gives_the_default_layout() {
    assert_eq!(from_settings(&Settings::default()), SidebarLayout::default());
    let strange = Settings { sidebar: Some("sideways".into()), sidebar_layout: SidebarSaved { project_badge: Some("blue".into()), ..Default::default() }, ..Default::default() };
    assert_eq!(from_settings(&strange), SidebarLayout::default());
}

/// What the Settings page keeps comes back as it was; the filter is not kept.
#[test]
fn a_look_is_kept_and_comes_back() {
    let look = SidebarLayout { project_badge: BadgeShow::Always, show_time: false, show_agent_icon: false, fold_after: 12, earlier_shown: 20, ..Default::default() };
    let settings = Settings { sidebar: Some("priority".into()), sidebar_layout: saved_look(&look), ..Default::default() };
    let back = from_settings(&settings);
    assert_eq!((back.project_badge, back.show_time, back.show_agent_icon, back.fold_after, back.earlier_shown), (BadgeShow::Always, false, false, 12, 20));
    assert_eq!(back.mode, atelier_ui::sidebar_model::ListMode::Priority);
    assert_eq!(back.filter, atelier_ui::sidebar_filter::SessionFilter::Active);
}
