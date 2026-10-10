use serde::{Deserialize, Serialize};

use super::BotId;
use crate::enums::StepState;

/// One step of a run: a bot and how far its work is.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RunStep {
    pub bot: BotId,
    /// Copied from the playbook when the run starts: the step asks a person when its turn comes.
    #[serde(default)]
    pub asks_first: bool,
    pub state: StepState,
    /// The session the app started for this step. The app picks the text, and nothing here reads it.
    #[serde(default)]
    pub session: Option<String>,
    /// What the bot wrote for the steps after it. Only a step that is done has one.
    #[serde(default)]
    pub hand_over: Option<String>,
    /// What a person last told this step.
    #[serde(default)]
    pub answer: Option<String>,
}
