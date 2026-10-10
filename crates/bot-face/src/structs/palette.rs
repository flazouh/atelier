use gpui_kit::Rgba;

/// The colours a caller gives the painter. The body colour is the bot's own, after it is mixed with grey. The ink is the
/// outline, which the theme sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub body: Rgba,
    pub shade: Rgba,
    pub light: Rgba,
    pub eye: Rgba,
    /// The outline. The theme's ink: dark on a light page, light on a dark one.
    pub ink: Rgba,
}
