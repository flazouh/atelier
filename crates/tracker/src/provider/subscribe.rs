use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
};

use atelier_capabilities::{Actor, StopFlag, tasks as v1};

use super::helpers::{activity_of, task_with_parent};
use crate::{Event, Subscription, Tracker};

/// How often the bridge looks at its stop flag when no change comes.
const TICK: Duration = Duration::from_millis(250);

/// Carries the tracker's changes to a tasks subscriber, as v1 events. It ends when the subscriber drops, or the tracker does.
pub(super) fn bridge(
    tracker: Arc<dyn Tracker>,
    account: String,
    actors: Arc<Mutex<HashMap<String, Actor>>>,
    from: Subscription,
    to: Sender<v1::Event>,
    stop: StopFlag,
) {
    thread::spawn(move || {
        while !stop.is_stopped() {
            let event = match from.recv_timeout(TICK) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            let told = match event {
                Event::Created(task) => Some(v1::Event {
                    kind: v1::EventKind::Created,
                    task: task_with_parent(tracker.as_ref(), &account, &task),
                    activity: None,
                }),
                Event::Updated(task) => Some(v1::Event {
                    kind: v1::EventKind::Updated,
                    task: task_with_parent(tracker.as_ref(), &account, &task),
                    activity: None,
                }),
                Event::Activity(activity) => {
                    tracker.get(&activity.task).ok().flatten().map(|task| {
                        let by = actors
                            .lock()
                            .ok()
                            .and_then(|a| a.get(&activity.by).cloned())
                            .unwrap_or_else(|| Actor::person(&activity.by, &activity.by));
                        v1::Event {
                            kind: v1::EventKind::Activity,
                            activity: Some(activity_of(&account, &task.key, &activity, by)),
                            task: task_with_parent(tracker.as_ref(), &account, &task),
                        }
                    })
                }
            };
            if let Some(event) = told
                && to.send(event).is_err()
            {
                return;
            }
        }
    });
}
