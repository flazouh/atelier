use std::sync::{Arc, Mutex};

use super::super::{backend::EventSink, event::Event};
use super::helpers::join_or_push;

pub(super) struct Shared {
    pub(super) pending: Mutex<Vec<Event>>,
    wake: Box<dyn Fn() + Send + Sync>,
}

/// Events waiting for the UI.
#[derive(Clone)]
pub struct EventQueue(pub(super) Arc<Shared>);

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
