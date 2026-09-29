//! Sessions push events as fast as an agent streams them; the UI draws once a frame. The queue joins
//! the deltas of one block while events wait, and wakes the UI once per batch, so a stream of
//! thousands of tokens costs one repaint a frame.
use std::sync::{Arc, Mutex};

use super::{backend::EventSink, event::Event};

struct Shared {
    pending: Mutex<Vec<Event>>,
    wake: Box<dyn Fn() + Send + Sync>,
}

/// Events waiting for the UI.
#[derive(Clone)]
pub struct EventQueue(Arc<Shared>);

impl EventQueue {
    /// `wake` runs when an event lands in an empty queue. It must be cheap and not block.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(Shared { pending: Mutex::new(Vec::new()), wake: Box::new(wake) }))
    }

    /// The sink to give a session.
    pub fn sink(&self) -> EventSink {
        let queue = self.clone();
        Arc::new(move |event| queue.push(event))
    }

    pub fn push(&self, event: Event) {
        let was_empty = {
            let mut pending = self.0.pending.lock().unwrap_or_else(|e| e.into_inner());
            let was_empty = pending.is_empty();
            join_or_push(&mut pending, event);
            was_empty
        };
        if was_empty {
            (self.0.wake)();
        }
    }

    /// Everything waiting, in order. Call it once per frame.
    pub fn drain(&self) -> Vec<Event> {
        std::mem::take(&mut *self.0.pending.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

fn join_or_push(pending: &mut Vec<Event>, event: Event) {
    match (pending.last_mut(), event) {
        (Some(Event::Text { block, delta }), Event::Text { block: next, delta: more }) if *block == next => {
            delta.push_str(&more)
        }
        (Some(Event::Thinking { block, delta }), Event::Thinking { block: next, delta: more }) if *block == next => {
            delta.push_str(&more)
        }
        (_, event) => pending.push(event),
    }
}
