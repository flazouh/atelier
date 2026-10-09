use serde::{Deserialize, Serialize};

/// How a number or a time prints. `percent` takes percentage points (42 prints "42%").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Count,
    RelativeTime,
    AbsoluteTime,
    DurationMs,
    Bytes,
    Percent,
}

impl Format {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "count" => Self::Count,
            "relative_time" => Self::RelativeTime,
            "absolute_time" => Self::AbsoluteTime,
            "duration_ms" => Self::DurationMs,
            "bytes" => Self::Bytes,
            "percent" => Self::Percent,
            _ => return None,
        })
    }
}

/// A colour by meaning. The theme maps a tone to a colour, so a card cannot set a raw one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Row,
    #[default]
    Column,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gap {
    Xs,
    #[default]
    Sm,
    Md,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    #[default]
    Body,
    Title,
    Muted,
    Code,
}
