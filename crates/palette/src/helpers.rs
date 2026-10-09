use super::types::Hue;
/// The hue at `index` of the palette, which wraps round.
pub fn hue_at(index: usize) -> Hue {
    Hue::ALL[index % Hue::ALL.len()]
}
