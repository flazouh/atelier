use std::collections::BTreeSet;

use crate::{Status, Task};
use super::types::{Rule, Signal};

/// Which rules are on. All are, until a team turns one off.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleSet {
    pub(super) off: BTreeSet<Rule>,
}

impl RuleSet {
    /// Every rule on, but the ones named in `disabled` (the ids the settings keep). An id that is no rule
    /// any more is skipped, so a setting from a newer version does not break an older one.
    pub fn from_disabled<'a>(disabled: impl IntoIterator<Item = &'a str>) -> Self {
        Self { off: disabled.into_iter().filter_map(Rule::from_id).collect() }
    }

    pub fn is_on(&self, rule: Rule) -> bool {
        !self.off.contains(&rule)
    }

    pub fn set(&mut self, rule: Rule, on: bool) {
        if on {
            self.off.remove(&rule);
        } else {
            self.off.insert(rule);
        }
    }

    /// The ids of the rules that are off, for the settings.
    pub fn disabled(&self) -> Vec<&'static str> {
        self.off.iter().map(|r| r.id()).collect()
    }

    /// What, if anything, the signal does to a task now in `status`. A closed task stays closed, and a task
    /// already past the step stays where it is.
    pub fn decide(&self, status: Status, signal: &Signal) -> Option<Decision> {
        let (rule, from, to): (Rule, &[Status], Status) = match signal {
            Signal::SessionStarted { .. } => {
                (Rule::SessionStartMovesToInProgress, &[Status::Backlog, Status::Todo], Status::InProgress)
            }
            Signal::SessionFinished { ok: true, .. } => (Rule::AgentFinishMovesToInReview, &[Status::InProgress], Status::InReview),
            Signal::PrMerged { .. } => (
                Rule::MergeMovesToDone,
                &[Status::Backlog, Status::Todo, Status::InProgress, Status::InReview],
                Status::Done,
            ),
            Signal::SessionResumed { .. } => (Rule::SessionResumeMovesToInProgress, &[Status::InReview], Status::InProgress),
            Signal::SessionFinished { ok: false, .. } | Signal::PrOpened { .. } | Signal::Committed { .. } => return None,
        };
        (self.is_on(rule) && from.contains(&status)).then_some(Decision { rule, to })
    }
}

/// A rule's answer: move the task to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub rule: Rule,
    pub to: Status,
}

/// What [`handle`] did to one task.
#[derive(Clone, Debug, PartialEq)]
pub struct Handled {
    /// The task as it is now.
    pub task: Task,
    /// The rule that moved it, if one did.
    pub moved: Option<Decision>,
}
