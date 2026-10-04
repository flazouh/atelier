use super::*;
use gpui_kit::SharedString;

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

fn keys(names: &[&str]) -> Vec<SharedString> {
    names.iter().map(|n| SharedString::from(n.to_string())).collect()
}

#[test]
fn a_new_session_stands_first_and_the_others_keep_their_order() {
    let mut order = Vec::new();
    assert_eq!(newest_first(keys(&["a", "b"]), |k| k, &mut order), keys(&["a", "b"]), "sessions that are all new keep their given order");
    assert_eq!(newest_first(keys(&["a", "b", "c"]), |k| k, &mut order), keys(&["c", "a", "b"]));
    assert_eq!(newest_first(keys(&["a", "c", "d", "b"]), |k| k, &mut order), keys(&["d", "c", "a", "b"]), "the projects' own order changes nothing");
    assert_eq!(newest_first(keys(&["d", "a"]), |k| k, &mut order), keys(&["d", "a"]));
    assert_eq!(order, keys(&["d", "a"]), "a closed session is forgotten");
}
