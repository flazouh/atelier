use serde::{Deserialize, Serialize};

use super::{BotId, RunStep};

/// One playbook at work on a brief. It keeps its own copy of the steps, so a later edit of the playbook does not
/// change a run. Its state is not kept: `state()` reads it from the steps.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Run {
    /// The name of its file. The app picks it.
    pub id: BotId,
    pub playbook: BotId,
    /// The text a person gave.
    pub brief: String,
    /// When it started, in milliseconds since 1970.
    pub started_ms: i64,
    /// A person stopped it. A stopped run keeps its steps as they were, to show where it stopped.
    #[serde(default)]
    pub stopped: bool,
    pub steps: Vec<RunStep>,
}
