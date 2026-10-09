//! Changes from now on. GitHub pushes nothing to a desktop app, so one thread asks for the newest issues every
//! `poll_every` with `If-None-Match`: a 304 costs no rate limit. The thread ends when the last subscriber drops.
use std::{
    sync::{Arc, Mutex, mpsc::channel},
    thread,
    time::Duration,
};

use atelier_capabilities::{
    CapError, CapResult, Subscription,
    tasks::{Event, EventKind, Task},
};
use serde_json::Value;

use super::structs::Hub;
use crate::{
    errors,
    runner::{Call, Gh},
    scope::Scope,
};

/// How often the thread looks at the subscribers' stop flags while it waits for the next poll.
const TICK: Duration = Duration::from_millis(200);

fn watch_path(scope: &Scope) -> String {
    format!(
        "{}?state=all&sort=updated&direction=desc&per_page=100",
        scope.issues_path()
    )
}

fn poisoned<T>(_: T) -> CapError {
    CapError::Storage {
        message: "the subscription state is broken".into(),
    }
}

pub(super) fn start(
    gh: &Arc<dyn Gh>,
    scope: &Arc<Scope>,
    hub: &Arc<Mutex<Hub>>,
) -> CapResult<Subscription<Event>> {
    let (events, receiver) = channel();
    let (subscription, stop) = Subscription::new(receiver);
    let mut guard = hub.lock().map_err(poisoned)?;
    if !guard.running {
        // The first look is taken here, before the caller can change anything, so that a change made right after
        // `subscribe` is a change and not part of the start. A failure (offline, signed out) is the caller's.
        let listed = gh
            .send(&Call::get(watch_path(scope)))
            .map_err(errors::from_failure)?;
        if listed.status >= 400 {
            return Err(errors::from_reply(&listed, "the issues"));
        }
        let newest = gh
            .send(&Call::get(format!(
                "{}?state=all&sort=created&direction=desc&per_page=1",
                scope.issues_path()
            )))
            .map_err(errors::from_failure)?;
        if newest.status >= 400 {
            return Err(errors::from_reply(&newest, "the issues"));
        }
        *guard = Hub::default();
        guard.etag = listed.header("etag").map(str::to_string);
        for row in rows(&listed.body) {
            if let Some((number, updated)) = number_and_time(&row) {
                guard.known.insert(number, updated);
            }
        }
        guard.newest = rows(&newest.body)
            .iter()
            .filter_map(|r| r["number"].as_u64())
            .max()
            .unwrap_or_default()
            .max(guard.known.keys().copied().max().unwrap_or_default());
        guard.running = true;
        let (gh, scope, hub) = (gh.clone(), scope.clone(), hub.clone());
        thread::spawn(move || poll(gh, scope, hub));
    }
    guard.subscribers.push((events, stop));
    Ok(subscription)
}

/// Remembers a change this provider made itself, so a poll does not tell it a second time.
pub(super) fn note(hub: &mut Hub, task: &Task) {
    if let Ok(number) = task.reference.id.parse::<u64>() {
        hub.known.insert(number, task.version.clone());
        hub.newest = hub.newest.max(number);
    }
}

/// Sends an event to every subscriber that is still there.
pub(super) fn tell(hub: &mut Hub, event: &Event) {
    hub.subscribers
        .retain(|(events, stop)| !stop.is_stopped() && events.send(event.clone()).is_ok());
}

fn rows(body: &str) -> Vec<Value> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

fn number_and_time(row: &Value) -> Option<(u64, String)> {
    if row.get("pull_request").is_some() {
        return None;
    }
    Some((
        row["number"].as_u64()?,
        row["updated_at"].as_str()?.to_string(),
    ))
}

/// Whether anyone listens. When nobody does, the state is cleared and the thread ends.
fn alive(hub: &Mutex<Hub>) -> bool {
    let Ok(mut guard) = hub.lock() else {
        return false;
    };
    guard.subscribers.retain(|(_, stop)| !stop.is_stopped());
    if guard.subscribers.is_empty() {
        *guard = Hub::default();
        return false;
    }
    true
}

fn poll(gh: Arc<dyn Gh>, scope: Arc<Scope>, hub: Arc<Mutex<Hub>>) {
    let every = scope.options.poll_every;
    loop {
        let mut waited = Duration::ZERO;
        while waited < every {
            let step = TICK.min(every - waited);
            thread::sleep(step);
            waited += step;
            if !alive(&hub) {
                return;
            }
        }
        if !alive(&hub) {
            return;
        }
        let etag = hub.lock().ok().and_then(|h| h.etag.clone());
        // A failed poll is tried again at the next tick: offline or rate limited is not a reason to end a watch.
        let Ok(reply) = gh.send(&Call::get(watch_path(&scope)).with_etag(etag)) else {
            continue;
        };
        if reply.status >= 300 {
            continue;
        }
        let Ok(mut guard) = hub.lock() else { return };
        guard.etag = reply.header("etag").map(str::to_string);
        // The newest change last, so the events come in the order the changes were made.
        for row in rows(&reply.body).into_iter().rev() {
            let Ok((issue, raw)) = Scope::read_issue(row) else {
                continue;
            };
            if issue.pull_request.is_some() {
                continue;
            }
            // A poll that began before an own write can hold an older state of that issue: only a newer one counts.
            let kind = match guard.known.get(&issue.number) {
                None if issue.number > guard.newest => EventKind::Created,
                None => EventKind::Updated,
                Some(seen) if issue.updated_at > *seen => EventKind::Updated,
                Some(_) => continue,
            };
            let Ok(task) = scope.task(&issue, raw) else {
                continue;
            };
            note(&mut guard, &task);
            tell(
                &mut guard,
                &Event {
                    kind,
                    task,
                    activity: None,
                },
            );
        }
    }
}
