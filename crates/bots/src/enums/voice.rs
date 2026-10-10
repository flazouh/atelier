use serde::{Deserialize, Serialize};

/// How a bot talks.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Voice {
    CalmAndClear,
    Cheerful,
    ShortAndDry,
    Thorough,
    Playful,
}
