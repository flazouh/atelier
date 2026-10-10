use serde::{Deserialize, Serialize};

/// Where one step of a run is. A step that waits, with every step before it over, is ready to work.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StepState {
    Waiting,
    Working,
    /// The step waits for a person. With no session yet, it asks before it starts.
    NeedsPerson {
        question: String,
    },
    Done,
    Failed {
        reason: String,
    },
    /// A person chose that the step does no work in this run.
    Skipped,
}
