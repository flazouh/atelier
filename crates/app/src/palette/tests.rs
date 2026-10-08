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
fn a_hue_reads_on_every_page_it_is_used_on_and_moves_the_way_the_page_needs() {
    let (light, dark) = (atelier_ui::theme::Theme::light(), atelier_ui::theme::Theme::dark());
    for hue in Hue::ALL {
        for page in [light.background, light.popover, light.card, dark.background, dark.popover, dark.card] {
            let ink = hue.on(page);
            assert!(atelier_ui::theme::contrast(ink, page) >= 4.5, "{hue:?} reads on {page:?}");
            if page.l > 0.5 {
                assert!(ink.l <= hue.hsla().l, "{hue:?} is no lighter on a light page");
            } else {
                assert!(ink.l >= hue.hsla().l, "{hue:?} is no darker on a dark page");
            }
        }
    }
}
