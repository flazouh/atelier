use std::collections::BTreeMap;

use serde::Deserialize;

use super::PartDef;

/// One bot as the data writes it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BotDef {
    pub id: String,
    pub name: String,
    pub role: String,
    pub colour: String,
    pub eye_y: f32,
    #[serde(default)]
    pub cheeks: Option<Vec<[f32; 4]>>,
    pub parts: Vec<PartDef>,
    pub eyes: BTreeMap<String, String>,
}
