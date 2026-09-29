use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lathe-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("settings.json")
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
