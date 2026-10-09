use gpui_kit::Rgba;

use super::shape_parse::parse_fragment;
use crate::enums::{Geometry, Mood, Paint, Token};
use crate::structs::{BotDef, BotModel, PartModel, Shape};

impl BotModel {
    /// Parses every part and every eye drawing of a bot.
    pub fn from_def(def: &BotDef) -> Result<BotModel, String> {
        let colour = Rgba::try_from(def.colour.as_str()).map_err(|e| format!("{}: bad colour: {e}", def.id))?;
        let parts = def
            .parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let shapes = parse_fragment(&p.svg).map_err(|e| format!("{} part {i}: {e}", def.id))?;
                Ok(PartModel { layer: p.layer, shapes, pivot: (p.pivot[0], p.pivot[1]), habit: p.habit.clone() })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let eyes = Mood::ALL
            .iter()
            .map(|m| {
                let svg = def.eyes.get(m.name()).ok_or_else(|| format!("{}: no eyes for {}", def.id, m.name()))?;
                parse_fragment(svg).map_err(|e| format!("{} eyes {}: {e}", def.id, m.name()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let cheeks = def.cheeks.iter().flatten().map(|c| cheek(*c)).collect();
        Ok(BotModel { id: def.id.clone(), name: def.name.clone(), role: def.role.clone(), colour, eye_y: def.eye_y, parts, eyes, cheeks })
    }
}

/// A faint rounded bar of light on the cheek. 0.7 of the light token's alpha shows.
fn cheek(c: [f32; 4]) -> Shape {
    Shape {
        geometry: Geometry::Rect { x: c[0], y: c[1], w: c[2], h: c[3], r: c[3] / 2.0 },
        fill: Some(Paint::Token(Token::Light)),
        stroke: None,
        opacity: 0.7,
        spin: None,
    }
}
