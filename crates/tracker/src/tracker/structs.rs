use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
};

use crate::Event;

/// The changes a [`Tracker`](super::Tracker) tells, until this is dropped. It reads as the receiver it holds
/// (`recv`, `recv_timeout`, `try_recv`). Dropping it tells the tracker to stop: a backend that polls a
/// service stops the poll at its next tick, and reads nothing after it.
pub struct Subscription {
    events: Receiver<Event>,
    pub(super) stop: Arc<AtomicBool>,
}

/// What the tracker keeps of a [`Subscription`]: it says whether the reader is gone.
#[derive(Clone, Debug)]
pub struct StopFlag(pub(super) Arc<AtomicBool>);

impl StopFlag {
    /// The subscription was dropped: send nothing more and read nothing more for it.
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Subscription {
    /// The reader's end, and the flag for the backend that sends to `events`.
    pub fn new(events: Receiver<Event>) -> (Self, StopFlag) {
        let stop = Arc::new(AtomicBool::new(false));
        (
            Self {
                events,
                stop: stop.clone(),
            },
            StopFlag(stop),
        )
    }
}

impl Deref for Subscription {
    type Target = Receiver<Event>;
    fn deref(&self) -> &Receiver<Event> {
        &self.events
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
