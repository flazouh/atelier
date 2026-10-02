use std::{
    io::Write,
    sync::{Mutex, MutexGuard, mpsc},
};

/// The state of a session is plain data that a panic on the other thread cannot leave half-written,
/// so a poisoned lock is still good to read.
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
