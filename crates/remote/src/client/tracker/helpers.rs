use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak, },
    thread,
    time::Duration,
};

use atelier_tracker::{Event, Query, Task, TaskId, TrackerError, TrackerResult};

use super::super::{Shared, lock};
use crate::protocol::{
    Call,
    Reply,
    tracker::{TrackerCall, TrackerReply},
};
use super::structs::Listeners;

pub(super) fn ask(shared: &Shared, call: TrackerCall) -> TrackerResult<TrackerReply> {
    match shared.request(Call::Tracker(call)) {
        Ok(Reply::Tracker(answer)) => *answer,
        Ok(_) => Err(unexpected()),
        Err(error) => Err(TrackerError::Storage(error.to_string())),
    }
}

pub(super) fn unexpected() -> TrackerError {
    TrackerError::Storage("the host gave an unexpected answer".into())
}

/// What changed since `seen`: a task it had not seen is new, one with another change time is updated.
pub(super) fn changes(seen: &mut HashMap<TaskId, i64>, tasks: &[Task]) -> Vec<Event> {
    let mut events = Vec::new();
    for task in tasks {
        match seen.insert(task.id.clone(), task.updated_at) {
            None => events.push(Event::Created(task.clone())),
            Some(at) if at != task.updated_at => events.push(Event::Updated(task.clone())),
            Some(_) => {}
        }
    }
    events
}

/// Lists the host's tasks every `every` and tells each receiver what changed, until no receiver is left
/// or the connection is gone. A receiver that was dropped is found at the next change it is told of.
pub(super) fn poll(shared: Weak<Shared>, listeners: Arc<Mutex<Listeners>>, every: Duration) {
    loop {
        // Nobody listens any more: the poll ends before it reads again.
        {
            let mut listeners = lock(&listeners);
            listeners.all.retain(|l| !l.stop.is_stopped());
            if listeners.all.is_empty() {
                listeners.polling = false;
                return;
            }
        }
        let Some(shared) = shared.upgrade() else { break };
        let listed = ask(&shared, TrackerCall::List { query: Query::default() });
        drop(shared);
        {
            let mut listeners = lock(&listeners);
            // A failed listing (the link is down) waits for the next turn.
            if let Ok(TrackerReply::Tasks(tasks)) = &listed {
                listeners.all.retain_mut(|listener| match &mut listener.seen {
                    None => {
                        listener.seen = Some(tasks.iter().map(|t| (t.id.clone(), t.updated_at)).collect());
                        true
                    }
                    Some(seen) => changes(seen, tasks).into_iter().all(|event| listener.send.send(event).is_ok()),
                });
            }
            if listeners.all.is_empty() {
                listeners.polling = false;
                return;
            }
        }
        thread::sleep(every);
    }
    lock(&listeners).polling = false;
}
