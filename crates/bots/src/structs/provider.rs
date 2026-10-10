use serde::{Deserialize, Serialize};

/// The model service a bot uses. No model means the harness picks its own.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Provider {
    pub service: String,
    #[serde(default)]
    pub model: Option<String>,
}
