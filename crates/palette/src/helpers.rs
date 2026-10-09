use super::types::Hue;
/// The hue at `index` of the palette, which wraps round.
pub fn hue_at(index: usize) -> Hue {
    Hue::ALL[index % Hue::ALL.len()]
}
/// The colour of a kind of release note, in `theme`: green for added, purple for improved, orange for faster, red for fixed,
/// blue for changed and pink for design, each as dark or light as the page needs for it to read.
pub fn kind_color(kind: atelier_ui::ReleaseKind, theme: &atelier_ui::theme::Theme) -> gpui_kit::Hsla {
    use atelier_ui::ReleaseKind::{Added, Changed, Design, Faster, Fixed, Improved};
    match kind {
        Added => Hue::Green,
        Improved => Hue::Purple,
        Faster => Hue::Orange,
        Fixed => Hue::Red,
        Changed => Hue::Blue,
        Design => Hue::Pink,
    }
    .on(theme.popover)
}
