use super::*;
#[test]
fn the_palette_keeps_its_order_and_its_colours() {
    let rgb: Vec<u32> = Hue::ALL.iter().map(|h| h.rgb()).collect();
    assert_eq!(rgb, [0xFF5C59, 0xFF8D22, 0xFBD73C, 0x15DB95, 0x4ACFFF, 0x9758FF, 0xFF78F7]);
}
#[test]
fn an_index_past_the_end_wraps_round() {
    assert_eq!(hue_at(0), Hue::Red);
    assert_eq!(hue_at(7), Hue::Red);
    assert_eq!(hue_at(11), Hue::Blue);
}
#[test]
fn a_hue_on_a_light_page_is_made_darker_until_it_reads_and_on_a_dark_one_it_is_left_alone() {
    let light = atelier_ui::theme::Theme::light().background;
    let dark = atelier_ui::theme::Theme::dark().background;
    for hue in Hue::ALL {
        assert!(atelier_ui::theme::contrast(hue.on(light), light) >= 4.5, "{hue:?} reads on a light page");
        assert!(atelier_ui::theme::contrast(hue.on(dark), dark) >= 4.5, "{hue:?} reads on a dark page");
        assert_eq!(hue.on(dark), hue.hsla(), "{hue:?} is as it is on a dark page");
    }
}
