use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
};

/// The changes a provider tells, until this is dropped. It reads as the receiver it holds (`recv`, `recv_timeout`,
/// `try_recv`). Dropping it tells the provider to stop: one that polls a service stops at its next tick.
pub struct Subscription<T> {
    events: Receiver<T>,
    stop: Arc<AtomicBool>,
}

/// What the provider keeps of a [`Subscription`]: it says whether the reader is gone.
#[derive(Clone, Debug)]
pub struct StopFlag(Arc<AtomicBool>);

impl StopFlag {
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl<T> Subscription<T> {
    /// The reader's end, and the flag for the provider that sends to `events`.
    pub fn new(events: Receiver<T>) -> (Self, StopFlag) {
        let stop = Arc::new(AtomicBool::new(false));
        (Self { events, stop: stop.clone() }, StopFlag(stop))
    }
}

impl<T> Deref for Subscription<T> {
    type Target = Receiver<T>;
    fn deref(&self) -> &Receiver<T> {
        &self.events
    }
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn dropping_the_reader_stops_the_sender() {
        let (tx, rx) = channel::<u8>();
        let (sub, flag) = Subscription::new(rx);
        tx.send(1).unwrap();
        assert_eq!(sub.recv().unwrap(), 1);
        assert!(!flag.is_stopped());
        drop(sub);
        assert!(flag.is_stopped());
    }
}
