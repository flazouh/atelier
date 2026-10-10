use serde::{Deserialize, Serialize};

/// What a bot may do with a connector. A write asks the person each time.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Read,
    Write,
}
