//! File icons as glyphs of one icon font, one for each file type, in one of nine colours. The glyphs are
//! Anysphere's (`assets/FileGlyphs.ttf`, only the ones the table names). Folders keep atelier-ui's icons.
//!
//! A file's glyph comes from, in order: a `.plan.md` file, the agent rules files, the VS Code files in
//! `.vscode`, the whole name, `.env` files, then the longest extension (`d.ts` before `ts`).

mod table;

use std::borrow::Cow;

use atelier_ui::{
    file_icon::{IconFor, set_source},
    theme::Theme,
};
use gpui_kit::{AnyElement, App, Hsla, IntoElement, ParentElement, Pixels, Styled, div, rgb};

const FONT_FAMILY: &str = "Atelier File Glyphs";

/// One glyph of the font, in its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub char: char,
    pub tone: Tone,
}

/// The icon colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Blue,
    Cyan,
    Green,
    Magenta,
    Orange,
    Purple,
    Red,
    Yellow,
    Secondary,
}

impl Tone {
    /// The colour on `theme`, one set for a dark background and one for a light one.
    pub fn color(self, theme: &Theme) -> Hsla {
        let dark = theme.background.l < 0.5;
        let hex = match (self, dark) {
            (Tone::Secondary, _) => return theme.foreground.opacity(0.66),
            (Tone::Blue, true) => 0x81A1C1,
            (Tone::Cyan, true) => 0x88C0D0,
            (Tone::Green, true) => 0x3FA266,
            (Tone::Magenta, true) => 0xB48EAD,
            (Tone::Orange, true) => 0xD08770,
            (Tone::Purple, true) => 0x9386F2,
            (Tone::Red, true) => 0xE34671,
            (Tone::Yellow, true) => 0xF1B467,
            (Tone::Blue, false) => 0x0064B0,
            (Tone::Cyan, false) => 0x176C74,
            (Tone::Green, false) => 0x00854C,
            (Tone::Magenta, false) => 0x92156A,
            (Tone::Orange, false) => 0xCD4500,
            (Tone::Purple, false) => 0x7565CC,
            (Tone::Red, false) => 0xCE405B,
            (Tone::Yellow, false) => 0xA46700,
        };
        rgb(hex).into()
    }
}

/// The glyph for the file at `path`.
pub fn glyph(path: &str) -> Glyph {
    let mut parts = path.rsplit(['/', '\\']);
    let name = parts.next().unwrap_or(path).to_lowercase();
    if name.ends_with(".plan.md") {
        return table::PLAN;
    }
    if table::is_rules_file(&name) {
        return table::RULES;
    }
    if table::is_vscode_file(&name) && parts.any(|p| p.eq_ignore_ascii_case(".vscode")) {
        return table::VSCODE;
    }
    if let Some(glyph) = table::by_name(&name) {
        return glyph;
    }
    if name == ".env" || name.starts_with(".env.") {
        return table::ENV;
    }
    if let Some(glyph) = table::by_extension(&name) {
        return glyph;
    }
    if name.ends_with('.') {
        return table::FILE;
    }
    name.match_indices('.').find_map(|(at, _)| table::by_extension(&name[at + 1..])).unwrap_or(table::FILE)
}

/// Loads the font and draws every file icon from now on with its glyphs.
pub fn install(cx: &mut App) {
    let font = Cow::Borrowed(include_bytes!("../assets/FileGlyphs.ttf").as_slice());
    if let Err(error) = cx.text_system().add_fonts(vec![font]) {
        eprintln!("could not load the file glyph font, so file icons stay atelier-ui's: {error}");
        return;
    }
    set_source(draw, cx);
}

fn draw(icon: &IconFor, size: Pixels, cx: &App) -> Option<AnyElement> {
    let IconFor::File(path) = icon else { return None };
    let glyph = glyph(path);
    Some(
        div()
            .flex_none()
            .size(size)
            .flex()
            .items_center()
            .justify_center()
            .font_family(FONT_FAMILY)
            .text_size(size)
            .line_height(size)
            .text_color(glyph.tone.color(cx.global::<Theme>()))
            .child(glyph.char.to_string())
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests;
