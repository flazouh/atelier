use super::*;

#[test]
fn a_badge_takes_the_picked_colour_else_the_places_own() {
    let mut badges = Badges::default();
    let plain = badge_of("~/code/x", "X", &badges);
    assert_eq!(plain.color, project_badge::fallback_color("~/code/x"));
    assert_eq!(plain.label, "X");
    assert!(plain.icon.is_none());
    badges.colors.insert("~/code/x".into(), 4);
    assert_eq!(badge_of("~/code/x", "X", &badges).color, 4);
    assert_eq!(badge_of("~/code/y", "Y", &badges).color, project_badge::fallback_color("~/code/y"), "another place is not touched");
}

#[test]
fn a_badge_shows_its_image_only_while_the_file_is_there() {
    let dir = crate::test_dirs::path();
    let file = dir.join("logo.svg");
    let mut badges = Badges::default();
    badges.icons.insert("~/code/x".into(), file.display().to_string());
    assert!(badge_of("~/code/x", "X", &badges).icon.is_none(), "the file is gone: the letter stands");
    std::fs::write(&file, "<svg/>").unwrap();
    assert_eq!(badge_of("~/code/x", "X", &badges).icon, Some(file));
}
