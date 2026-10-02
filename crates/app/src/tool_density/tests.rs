use super::*;

#[test]
fn a_key_names_its_density_and_anything_else_is_the_default() {
    for density in ToolDensity::ALL {
        assert_eq!(ToolDensity::from_key(Some(density.key())), density, "{density:?}");
    }
    assert_eq!(ToolDensity::from_key(None), ToolDensity::Grouped);
    assert_eq!(ToolDensity::from_key(Some("loud")), ToolDensity::Grouped);
}

#[test]
fn each_density_has_its_own_words_and_key() {
    let words: std::collections::HashSet<_> = ToolDensity::ALL.iter().map(|d| d.word()).collect();
    let keys: std::collections::HashSet<_> = ToolDensity::ALL.iter().map(|d| d.key()).collect();
    assert_eq!((words.len(), keys.len()), (3, 3));
}

#[gpui_kit::test]
fn the_density_is_the_global_or_the_default(cx: &mut gpui_kit::TestAppContext) {
    cx.update(|cx| {
        assert_eq!(tool_density(cx), ToolDensity::Grouped);
        cx.set_global(ToolDensity::Detailed);
        assert_eq!(tool_density(cx), ToolDensity::Detailed);
    });
}
