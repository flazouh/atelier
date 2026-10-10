use gpui_kit::Rgba;

use crate::consts::DARK_INK;
use crate::enums::Token;
use crate::structs::Palette;

impl Palette {
    /// The usual palette for a bot colour: black at 30% for the shade, white at 40% for the light, a dark eye and the
    /// dark ink for the outline. Give the theme's ink for a dark page.
    pub fn standard(body: Rgba) -> Palette {
        Palette {
            body,
            shade: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.3,
            },
            light: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.4,
            },
            eye: Rgba {
                r: 0.08,
                g: 0.08,
                b: 0.075,
                a: 1.0,
            },
            ink: DARK_INK,
        }
    }

    pub fn colour(&self, token: Token) -> Rgba {
        match token {
            Token::Body => self.body,
            Token::Shade => self.shade,
            Token::Light => self.light,
            Token::Eye => self.eye,
            Token::Ink => self.ink,
        }
    }

    /// The colour moved `share` of the way toward the grey.
    pub fn mixed(colour: Rgba, grey: Rgba, share: f32) -> Rgba {
        let mix = |a: f32, b: f32| a * (1.0 - share) + b * share;
        Rgba {
            r: mix(colour.r, grey.r),
            g: mix(colour.g, grey.g),
            b: mix(colour.b, grey.b),
            a: colour.a,
        }
    }
}
