use std::{
    io::Write,
    sync::{Mutex, MutexGuard, mpsc},
};

use super::super::protocol::Step;
use crate::{
    session::{EventSink, SessionError},
};
use super::types::Lines;

/// Writes a step's lines, then hands its events to the sink. The caller holds the protocol's lock, so the
/// lines and events of two steps never interleave. A writer that is gone, because the agent stopped reading
/// or the session closed, is `Closed`.
pub(in super::super) fn deliver(step: Step, lines: &Lines, sink: &EventSink) -> Result<(), SessionError> {
    let written = {
        let mut lines = lock(lines);
        let sent = match lines.as_ref() {
            Some(sender) => step.lines.into_iter().try_for_each(|line| sender.send(line)).is_ok(),
            None => false,
        };
        if !sent {
            *lines = None;
        }
        sent
    };
    step.events.into_iter().for_each(|event| sink(event));
    if written { Ok(()) } else { Err(SessionError::Closed) }
}

/// The protocol is plain data that a panic on the other thread cannot leave half-written, so a poisoned
/// lock is still good to read.
pub(super) fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn write_lines(mut stdin: Box<dyn Write + Send>, queued: mpsc::Receiver<String>) {
    for line in queued {
        if writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_err() {
            return;
        }
    }
}
