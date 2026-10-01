//! The host's side of a tracker call: one call of the trait on the store the project keeps there.

use atelier_tracker::{Tracker, TrackerResult};

use crate::protocol::tracker::{TrackerCall, TrackerReply};

pub(super) fn answer(tracker: &dyn Tracker, call: TrackerCall) -> TrackerResult<TrackerReply> {
    Ok(match call {
        TrackerCall::Name => TrackerReply::Name(tracker.name().to_string()),
        TrackerCall::List { query } => TrackerReply::Tasks(tracker.list(&query)?),
        TrackerCall::Get { id } => TrackerReply::Found(tracker.get(&id)?),
        TrackerCall::Create { new, by } => TrackerReply::Task(tracker.create(&new, &by)?),
        TrackerCall::CreateMany { new, by } => TrackerReply::Tasks(tracker.create_many(&new, &by)?),
        TrackerCall::Update { id, patch, by } => TrackerReply::Task(tracker.update(&id, &patch, &by)?),
        TrackerCall::Record { id, entry, by } => TrackerReply::Activity(tracker.record(&id, &entry, &by)?),
        TrackerCall::Activity { id } => TrackerReply::Log(tracker.activity(&id)?),
        TrackerCall::TasksOfSession { session_id } => TrackerReply::Ids(tracker.tasks_of_session(&session_id)?),
        TrackerCall::TasksOfPr { number } => TrackerReply::Ids(tracker.tasks_of_pr(number)?),
        TrackerCall::Labels => TrackerReply::Labels(tracker.labels()?),
    })
}
