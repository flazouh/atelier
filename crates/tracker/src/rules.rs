//! The automation that moves a task along, from the neutral events of sessions and pull requests. Each rule
//! is data: a name, a plain sentence for the settings, and an on/off switch in a [`RuleSet`]. The decision
//! ([`RuleSet::decide`]) is pure; [`handle`] reads the task, decides, and writes through a [`Tracker`].
use std::collections::BTreeSet;

use crate::{Entry, Patch, PrLink, SessionLink, Status, Task, TaskId, Tracker, TrackerResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// A session started from a task moves it to In Progress.
    SessionStartMovesToInProgress,
    /// The agent finishing its work moves the task to In Review.
    AgentFinishMovesToInReview,
    /// The pull request merging moves the task to Done.
    MergeMovesToDone,
}

impl Rule {
    pub const ALL: [Rule; 3] = [Self::SessionStartMovesToInProgress, Self::AgentFinishMovesToInReview, Self::MergeMovesToDone];

    /// The id kept in the settings, and written in the activity log as `rule:<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Self::SessionStartMovesToInProgress => "session-start",
            Self::AgentFinishMovesToInReview => "agent-finish",
            Self::MergeMovesToDone => "merge",
        }
    }

    pub fn from_id(id: &str) -> Option<Rule> {
        Self::ALL.into_iter().find(|r| r.id() == id)
    }

    /// What the rule does, for the settings.
    pub fn words(self) -> &'static str {
        match self {
            Self::SessionStartMovesToInProgress => "Starting a session on a task moves it to In Progress",
            Self::AgentFinishMovesToInReview => "The agent finishing its work moves the task to In Review",
            Self::MergeMovesToDone => "Merging the pull request moves the task to Done",
        }
    }
}

/// Which rules are on. All are, until a team turns one off.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleSet {
    off: BTreeSet<Rule>,
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
            Signal::SessionFinished { ok: false, .. } | Signal::PrOpened { .. } => return None,
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

/// What happened, in the neutral words of the session and forge events.
#[derive(Clone, Debug, PartialEq)]
pub enum Signal {
    /// A session was started from this task.
    SessionStarted { task: TaskId, session: SessionLink },
    /// A session ended. `ok` is false when it failed, which moves nothing.
    SessionFinished { session_id: String, ok: bool },
    /// A pull request was opened for this task, by `by`.
    PrOpened { task: TaskId, pr: PrLink, by: String },
    /// A pull request was merged, by `by`. Every task it is linked to hears of it.
    PrMerged { number: u64, by: String },
}

/// What [`handle`] did to one task.
#[derive(Clone, Debug, PartialEq)]
pub struct Handled {
    /// The task as it is now.
    pub task: Task,
    /// The rule that moved it, if one did.
    pub moved: Option<Decision>,
}

/// Applies a signal: logs it on the tasks it concerns, then lets the rules move each task. Does nothing for
/// a signal that concerns no task.
pub fn handle(tracker: &dyn Tracker, rules: &RuleSet, signal: &Signal) -> TrackerResult<Vec<Handled>> {
    match signal {
        Signal::SessionStarted { task, session } => {
            tracker.record(task, &Entry::SessionStarted(session.clone()), &session.agent)?;
            Ok(vec![decide_and_move(tracker, rules, task, signal)?])
        }
        Signal::PrOpened { task, pr, by } => {
            tracker.record(task, &Entry::PrOpened(pr.clone()), by)?;
            Ok(vec![decide_and_move(tracker, rules, task, signal)?])
        }
        Signal::SessionFinished { session_id, .. } => tracker
            .tasks_of_session(session_id)?
            .iter()
            .map(|task| decide_and_move(tracker, rules, task, signal))
            .collect(),
        Signal::PrMerged { number, by } => tracker
            .tasks_of_pr(*number)?
            .iter()
            .map(|id| {
                if let Some(pr) = tracker.get(id)?.and_then(|t| t.prs.into_iter().find(|p| p.number == *number)) {
                    tracker.record(id, &Entry::PrMerged(pr), by)?;
                }
                decide_and_move(tracker, rules, id, signal)
            })
            .collect(),
    }
}

fn decide_and_move(tracker: &dyn Tracker, rules: &RuleSet, id: &TaskId, signal: &Signal) -> TrackerResult<Handled> {
    let task = tracker.get(id)?.ok_or_else(|| crate::TrackerError::NotFound(id.clone()))?;
    match rules.decide(task.status, signal) {
        Some(decision) => {
            let by = format!("rule:{}", decision.rule.id());
            let task = tracker.update(id, &Patch::status(decision.to), &by)?;
            Ok(Handled { task, moved: Some(decision) })
        }
        None => Ok(Handled { task, moved: None }),
    }
}

#[cfg(test)]
mod tests;
