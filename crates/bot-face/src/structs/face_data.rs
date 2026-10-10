use std::collections::BTreeMap;

use serde::Deserialize;

use super::{BotDef, StateDef};

/// The whole `faces.v1.json` file.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FaceData {
    pub version: u32,
    pub view_box: [f32; 4],
    pub ground_y: f32,
    pub grey: String,
    pub colour_mix_with_grey: f32,
    pub states: BTreeMap<String, StateDef>,
    pub bots: Vec<BotDef>,
}
