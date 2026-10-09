use serde::Deserialize;

/// Where a part sits in the draw order.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Back,
    Body,
    Front,
}
