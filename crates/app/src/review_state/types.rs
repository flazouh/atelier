use std::collections::HashMap;

use atelier_review::Merged;
use serde::{Deserialize, Serialize};

use crate::review_pane::Scope;

/// Each reviewed file as the review left it, by scope and path: its hunks after the reader's decisions
/// and edits (`None` for a file with no text), and the text last written or read on disk.
pub type Decided = HashMap<(Scope, String), (Option<Merged>, Option<String>)>;

/// How the reader answered a call's approval, kept across a resume: the history the agent replays has
/// no approvals in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Approval {
    Approved,
    AlwaysAllowed,
    Denied,
}

impl Approval {
    pub fn of(kind: atelier_agents::session::ChoiceKind) -> Self {
        use atelier_agents::session::ChoiceKind;
        match kind {
            ChoiceKind::Allow => Self::Approved,
            ChoiceKind::AllowAlways => Self::AlwaysAllowed,
            ChoiceKind::Deny => Self::Denied,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(super) enum KindRecord {
    Text,
    /// Not text, and whether it was added, deleted or changed.
    Binary(ChangeRecord),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(super) enum ChangeRecord {
    Modified,
    Added,
    Deleted,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(super) enum ScopeRecord {
    Turn(usize),
    Whole,
}

impl ScopeRecord {
    pub(super) fn of(scope: Scope) -> Self {
        match scope {
            Scope::Turn(turn) => Self::Turn(turn),
            Scope::Whole => Self::Whole,
        }
    }

    pub(super) fn rebuild(self) -> Scope {
        match self {
            Self::Turn(turn) => Scope::Turn(turn),
            Self::Whole => Scope::Whole,
        }
    }
}
