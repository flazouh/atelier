use serde::{Deserialize, Serialize};

use super::BotId;

/// One step of a playbook: a bot, and whether it stops to ask the person before it starts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlaybookStep {
    pub bot: BotId,
    #[serde(default)]
    pub asks_first: bool,
}
