use serde::{Deserialize, Serialize};

use super::{BotId, PlaybookStep};

/// A named order of bots for one kind of job. Each step hands over through the ticket.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Playbook {
    pub id: BotId,
    pub name: String,
    pub steps: Vec<PlaybookStep>,
}
