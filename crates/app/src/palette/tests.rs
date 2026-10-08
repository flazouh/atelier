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
