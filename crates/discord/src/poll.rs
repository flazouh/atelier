use std::{
    collections::HashMap,
    sync::{Arc, mpsc::Sender},
    thread,
    time::{Duration, Instant},
};

use atelier_capabilities::{
    CapError, CapResult, Ref, StopFlag,
    messaging::{Event, EventKind, Message},
};

use crate::core::{Core, PAGE_MAX};

/// The messages posted in `channel` after `last`, oldest first, and the id to ask after next time.
fn since(
    core: &Core,
    channel: &Ref,
    last: Option<&str>,
) -> CapResult<(Vec<Message>, Option<String>)> {
    let id = core.channel_id(channel)?;
    let view = core.read_rows(id, PAGE_MAX, None, last)?;
    let newest = view.rows.last().map(|r| r.id.clone());
    Ok((core.messages_of(channel, &view.rows), newest))
}

/// Looks for new messages every `poll_every` until the subscription is dropped. A failed look is tried again at the
/// next turn, and a rate limit makes it wait first, so a slow service is never pushed.
pub fn poll(
    core: Arc<Core>,
    channels: Vec<Ref>,
    mut last: HashMap<Ref, Option<String>>,
    tx: Sender<Event>,
    stop: StopFlag,
) {
    let every = core.config.poll_every;
    let slice = Duration::from_millis(10);
    loop {
        let until = Instant::now() + every;
        while Instant::now() < until {
            if stop.is_stopped() {
                return;
            }
            thread::sleep(slice.min(every));
        }
        let mut fresh: Vec<Message> = Vec::new();
        for channel in &channels {
            let after = last.get(channel).cloned().flatten();
            match since(&core, channel, after.as_deref()) {
                Ok((messages, newest)) => {
                    if newest.is_some() {
                        last.insert(channel.clone(), newest);
                    }
                    fresh.extend(messages);
                }
                Err(CapError::RateLimited { retry_after_ms }) => {
                    thread::sleep(Duration::from_millis(retry_after_ms.min(60_000)));
                }
                Err(_) => {}
            }
        }
        fresh.sort_by(|a, b| {
            (a.created_at, a.reference.to_string()).cmp(&(b.created_at, b.reference.to_string()))
        });
        for message in fresh {
            let event = Event {
                kind: EventKind::Posted,
                channel: message.channel.clone(),
                message: message.reference.clone(),
                by: Some(message.author.clone()),
                reaction: None,
                data: Some(message),
            };
            if stop.is_stopped() || tx.send(event).is_err() {
                return;
            }
        }
    }
}
