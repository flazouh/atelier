use gpui_kit::Rgba;

/// The colours a caller gives the painter. The body colour is the bot's own, after it is mixed with grey.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub body: Rgba,
    pub shade: Rgba,
    pub light: Rgba,
    pub eye: Rgba,
}
