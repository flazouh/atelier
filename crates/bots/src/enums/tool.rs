use serde::{Deserialize, Serialize};

/// The tool a bot carries. It is for the face. Skills and connectors are separate.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Arm,
    Lens,
    Periscope,
    Parcel,
    Antennae,
    Flag,
    Key,
}
