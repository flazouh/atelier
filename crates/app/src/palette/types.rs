use gpui_kit::Hsla;
/// One colour of the app's palette, in the order they are handed out to things that need a run of different ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
}
impl Hue {
    /// Every hue, in the order of the palette.
    pub const ALL: [Hue; 7] = [Hue::Red, Hue::Orange, Hue::Yellow, Hue::Green, Hue::Blue, Hue::Purple, Hue::Pink];
    pub const fn rgb(self) -> u32 {
        match self {
            Hue::Red => 0xFF5C59,
            Hue::Orange => 0xFF8D22,
            Hue::Yellow => 0xFBD73C,
            Hue::Green => 0x15DB95,
            Hue::Blue => 0x4ACFFF,
            Hue::Purple => 0x9758FF,
            Hue::Pink => 0xFF78F7,
        }
    }
    pub fn hsla(self) -> Hsla {
        gpui_kit::rgb(self.rgb()).into()
    }
}
