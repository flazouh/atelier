use gpui_kit::Hsla;
/// The least contrast for ink on a page: WCAG AA for normal text.
const READABLE: f32 = 4.5;
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
    /// The hue as ink on `background`: the hue itself when it reads there (4.5 to 1, as text needs), else the same hue made
    /// darker a step at a time until it does. The palette is made for a dark page; on a light one the pale hues need this.
    pub fn on(self, background: Hsla) -> Hsla {
        let mut ink = self.hsla();
        while atelier_ui::theme::contrast(ink, background) < READABLE && ink.l > 0.05 {
            ink.l -= 0.01;
        }
        ink
    }
}
