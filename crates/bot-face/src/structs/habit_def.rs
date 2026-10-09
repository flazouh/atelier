use serde::Deserialize;

use crate::enums::HabitKind;

/// A habit as the data writes it. A kind reads only the fields it needs.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct HabitDef {
    pub kind: HabitKind,
    #[serde(default)]
    pub amp_deg: Option<f32>,
    #[serde(default)]
    pub every_s: Option<f32>,
    #[serde(default)]
    pub dx: Option<f32>,
    #[serde(default)]
    pub dy: Option<f32>,
    #[serde(default)]
    pub rot_deg: Option<f32>,
    #[serde(default)]
    pub hz: Option<f32>,
    #[serde(default)]
    pub squash: Option<f32>,
    #[serde(default)]
    pub amp: Option<f32>,
    #[serde(default)]
    pub deg_per_s: Option<f32>,
    #[serde(default)]
    pub stretch: Option<f32>,
    #[serde(default)]
    pub phase: Option<f32>,
}
