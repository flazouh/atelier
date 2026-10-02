use crate::{Entry, Patch, TaskId, Tracker, TrackerResult};
use super::structs::{Handled, RuleSet};
use super::types::Signal;

/// Applies a signal: logs it on the tasks it concerns, then lets the rules move each task. Does nothing for
/// a signal that concerns no task.
pub fn handle(tracker: &dyn Tracker, rules: &RuleSet, signal: &Signal) -> TrackerResult<Vec<Handled>> {
    match signal {
        Signal::SessionStarted { task, session } => {
            tracker.record(task, &Entry::SessionStarted(session.clone()), &session.agent)?;
            Ok(vec![decide_and_move(tracker, rules, task, signal)?])
        }
        Signal::Committed { session_id, sha, subject, by } => {
            let entry = Entry::Commit { sha: sha.clone(), subject: subject.clone() };
            tracker
                .tasks_of_session(session_id)?
                .iter()
                .map(|task| {
                    tracker.record(task, &entry, by)?;
                    decide_and_move(tracker, rules, task, signal)
                })
                .collect()
        }
        Signal::PrOpened { task, pr, by } => {
            tracker.record(task, &Entry::PrOpened(pr.clone()), by)?;
            Ok(vec![decide_and_move(tracker, rules, task, signal)?])
        }
        Signal::SessionFinished { session_id, .. } | Signal::SessionResumed { session_id } => tracker
            .tasks_of_session(session_id)?
            .iter()
            .map(|task| decide_and_move(tracker, rules, task, signal))
            .collect(),
        Signal::PrMerged { number, by } => tracker
            .tasks_of_pr(*number)?
            .iter()
            .map(|id| {
                // The app sees a merged pull request at every launch: it is logged the first time only.
                let told = tracker
                    .activity(id)?
                    .iter()
                    .any(|a| matches!(&a.kind, crate::ActivityKind::PrMerged { pr } if pr.number == *number));
                if !told && let Some(pr) = tracker.get(id)?.and_then(|t| t.prs.into_iter().find(|p| p.number == *number)) {
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
