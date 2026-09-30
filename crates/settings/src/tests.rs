use super::*;

thread_local! {
    static OWNED: std::cell::RefCell<Vec<tempfile::TempDir>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A settings file in a folder that goes when the test thread ends.
fn scratch(_name: &str) -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    OWNED.with(|owned| owned.borrow_mut().push(dir));
    path
}

#[test]
fn a_saved_theme_loads_back_and_a_missing_or_broken_file_is_the_default() {
    let path = scratch("theme");
    assert_eq!(load(&path), Settings::default(), "no file yet");
    update(&path, |s| s.theme = Some("Catppuccin Mocha".into())).unwrap();
    assert_eq!(load(&path).theme.as_deref(), Some("Catppuccin Mocha"));
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load(&path), Settings::default(), "a broken file falls back");
}

#[test]
fn setting_the_theme_keeps_the_recent_projects_and_unknown_keys() {
    let path = scratch("keep");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"theme":"lathe Dark","recent":[{"kind":"local","path":"/work/lathe"}],"font_size":14}"#).unwrap();
    update(&path, |s| s.theme = Some("lathe Light".into())).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let back = load(&path);
    assert_eq!(back.theme.as_deref(), Some("lathe Light"));
    assert_eq!(back.recent, [Location::Local { path: "/work/lathe".into() }]);
    assert!(text.contains("\"font_size\": 14"), "an unknown key survives: {text}");
}

#[test]
fn opening_a_project_moves_it_first_once_and_the_list_stays_short() {
    let mut s = Settings::default();
    let local = |n: usize| Location::Local { path: format!("/p/{n}").into() };
    for n in 0..12 {
        s.opened(local(n));
    }
    assert_eq!(s.recent.len(), RECENT_LIMIT);
    assert_eq!(s.recent[0], local(11));
    s.opened(local(5));
    assert_eq!(s.recent[0], local(5));
    assert_eq!(s.recent.iter().filter(|l| **l == local(5)).count(), 1);
    let remote = Location::Ssh { host: "hp-agent".into(), path: "/home/alex/code/lathe".into() };
    s.opened(remote.clone());
    assert_eq!(s.recent[0], remote);
    assert_eq!(remote.name(), "lathe");
    assert_eq!(remote.place(), "hp-agent:/home/alex/code/lathe");
}

#[test]
fn session_names_and_panels_come_back() {
    let path = scratch("panels");
    update(&path, |s| {
        s.session_names.insert("abc".into(), "Fix the flaky test".into());
        s.panels = Panels { single: true, grouped: true, widths: vec![("abc".into(), 520.)] };
    })
    .unwrap();
    let back = load(&path);
    assert_eq!(back.session_names.get("abc").map(String::as_str), Some("Fix the flaky test"));
    assert!(back.panels.single && back.panels.grouped);
    assert_eq!(back.panels.widths, [("abc".to_string(), 520.)]);
}

#[test]
fn the_mode_and_the_primary_colour_are_kept_and_an_old_file_without_them_still_loads() {
    let path = scratch("mode");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, r#"{"theme": "lathe Dark", "recent": []}"#).unwrap();
    let old = load(&path);
    assert_eq!((old.mode, old.primary), (None, None));
    update(&path, |s| {
        s.mode = Some("system".into());
        s.primary = Some([2, 133, 247]);
    })
    .unwrap();
    let back = load(&path);
    assert_eq!((back.mode.as_deref(), back.primary, back.theme.as_deref()), (Some("system"), Some([2, 133, 247]), Some("lathe Dark")));
    update(&path, |s| s.primary = None).unwrap();
    assert_eq!(load(&path).primary, None, "back to the default");
}
/// The sessions open at quit, and the one in front, come back for the next launch.
#[test]
fn the_open_sessions_come_back() {
    let path = scratch("open");
    let here = Location::Local { path: "/work/lathe".into() };
    update(&path, |s| {
        s.open = vec![
            OpenSession { location: here.clone(), id: "s1".into(), title: "Fix the lease".into() },
            OpenSession { location: here.clone(), id: "s2".into(), title: "Add a test".into() },
        ];
        s.front = Some("s2".into());
    })
    .unwrap();
    let back = load(&path);
    assert_eq!(back.open.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), ["s1", "s2"]);
    assert_eq!(back.open[0].location, here);
    assert_eq!(back.front.as_deref(), Some("s2"));
}

#[test]
fn the_task_rules_the_reader_turned_off_are_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{\"theme\":\"lathe Light\"}").unwrap();
    assert!(old.task_rules_off.is_empty(), "a file from before the rules loads");
    let kept = Settings { task_rules_off: vec!["merge".into()], ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.task_rules_off, ["merge"]);
}

#[test]
fn the_design_choices_are_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!((old.design_toggle, old.design_tabs), (None, None));
    let kept = Settings { design_toggle: Some(3), design_tabs: Some(1), ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!((round.design_toggle, round.design_tabs), (Some(3), Some(1)));
}

#[test]
fn the_elevation_choice_is_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.design_elevation, None);
    let kept = Settings { design_elevation: Some(3), ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.design_elevation, Some(3));
}

#[test]
fn the_elevation_strength_is_kept_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.design_strength, None);
    let kept = Settings { design_strength: Some(70), ..Settings::default() };
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.design_strength, Some(70));
}

#[test]
fn a_project_keeps_its_badge_colour_and_icon_and_an_old_file_has_none() {
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert!(old.project_colors.is_empty() && old.project_icons.is_empty());
    let mut kept = Settings::default();
    kept.project_colors.insert("~/code/x".into(), 7);
    kept.project_icons.insert("~/code/x".into(), "/data/project-icons/ab.svg".into());
    let round: Settings = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
    assert_eq!(round.project_colors["~/code/x"], 7);
    assert_eq!(round.project_icons["~/code/x"], "/data/project-icons/ab.svg");
}
