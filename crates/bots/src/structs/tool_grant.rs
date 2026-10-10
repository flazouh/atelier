use serde::{Deserialize, Serialize};

use crate::enums::Access;

/// One connector a bot may use, and how.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ToolGrant {
    pub connector: String,
    pub access: Access,
}
