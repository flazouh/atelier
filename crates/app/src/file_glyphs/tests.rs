use super::*;

#[test]
fn a_rust_file_wears_the_filled_rust_glyph_in_orange() {
    assert_eq!(glyph("crates/app/src/main.rs"), Glyph { char: '\u{f4ae}', tone: Tone::Orange });
    assert_eq!(glyph("MAIN.RS"), glyph("main.rs"));
}

#[test]
fn a_whole_name_comes_before_the_extension() {
    assert_eq!(glyph("Cargo.lock"), glyph("lib.rs"));
    assert_ne!(glyph("yarn.lock"), glyph("other.lock"));
}

#[test]
fn the_longest_extension_wins() {
    assert_eq!(glyph("a.css.map"), glyph("a.css"));
    assert_eq!(glyph("x.y.ts"), glyph("x.ts"));
}

#[test]
fn env_plan_rules_and_vscode_files_have_their_own() {
    assert_eq!(glyph(".env.local"), table::ENV);
    assert_eq!(glyph("plans/x.plan.md"), table::PLAN);
    assert_eq!(glyph(".cursorrules"), table::RULES);
    assert_eq!(glyph(".vscode/settings.json"), table::VSCODE);
    assert_ne!(glyph("settings.json"), table::VSCODE);
}

#[test]
fn an_unknown_file_is_the_plain_file() {
    for path in ["notes.zzzz", "Makefile.", "LICENSE-nothing"] {
        assert_eq!(glyph(path), table::FILE, "{path}");
    }
}

#[test]
fn colours_follow_light_and_dark() {
    let (dark, light) = (Theme::dark(), Theme::light());
    assert_ne!(Tone::Orange.color(&dark), Tone::Orange.color(&light));
    assert_eq!(Tone::Secondary.color(&dark), dark.foreground.opacity(0.66));
}
