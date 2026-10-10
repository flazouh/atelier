use serde::{Deserialize, Serialize};

use super::{BotId, FaceChoice, Provider, ToolGrant};
use crate::enums::{Harness, Voice};

/// A named worker. It belongs to one person.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Bot {
    pub id: BotId,
    pub name: String,
    pub role: String,
    /// One line: what the bot does.
    pub job: String,
    pub face: FaceChoice,
    pub harness: Harness,
    pub provider: Provider,
    /// Skill names. A name may end in `*` to take a family (`content-*`).
    pub skills: Vec<String>,
    pub tools: Vec<ToolGrant>,
    pub voice: Voice,
    /// Each edit makes a new version. Starts at 1.
    pub version: u32,
}
