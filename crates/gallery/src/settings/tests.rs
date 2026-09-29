use super::*;

#[test]
fn a_saved_theme_loads_back_and_a_missing_file_is_the_default() {
    let path = std::env::temp_dir().join(format!("lathe-settings-{}/settings.json", std::process::id()));
    assert_eq!(load(&path), Settings::default(), "no file yet");
    let settings = Settings { theme: Some("Catppuccin Mocha".into()) };
    save(&path, &settings).unwrap();
    assert_eq!(load(&path), settings);
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load(&path), Settings::default(), "a broken file falls back");
    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}
