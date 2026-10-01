//! A running ACP agent: two threads around the process, so neither reading nor writing ever waits on the
//! other or on the caller. The reader feeds lines to the protocol and its events to the sink; the writer
//! sends what the protocol wrote. Only this file touches a process.
use std::{
    io::Write,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

use atelier_project::{Control, Process};

use super::protocol::{Protocol, Step};
use crate::{
    session::{Command, EventSink, Session, SessionError},
    subprocess,
};

pub(super) struct AcpSession {
    lines: Option<mpsc::Sender<String>>,
    protocol: Arc<Mutex<Protocol>>,
    control: Arc<Mutex<Box<dyn Control>>>,
    closing: Arc<AtomicBool>,
    sink: EventSink,
}

impl AcpSession {
    /// Drives `process` with `protocol`, writing `first` (the handshake's opening line) at once.
    pub(super) fn run(process: Process, protocol: Protocol, first: Vec<String>, sink: EventSink) -> Self {
        let Process { stdin, stdout, control } = process;
        let protocol = Arc::new(Mutex::new(protocol));
        let control = Arc::new(Mutex::new(control));
        let closing = Arc::new(AtomicBool::new(false));

        let (lines, queued) = mpsc::channel::<String>();
        first.into_iter().for_each(|line| drop(lines.send(line)));
        thread::spawn(move || write_lines(stdin, queued));

        let (protocol_in, control_in, closing_in, sink_in, lines_in) =
            (protocol.clone(), control.clone(), closing.clone(), sink.clone(), lines.clone());
        thread::spawn(move || {
            for line in subprocess::lines(stdout) {
                if closing_in.load(Ordering::SeqCst) {
                    return;
                }
                let mut protocol = lock(&protocol_in);
                let step = protocol.line(&line, Instant::now());
                let done = step.done;
                deliver(step, &lines_in, &sink_in);
                drop(protocol);
                if done {
                    let _ = lock(&control_in).kill();
                    return;
                }
            }
            let code = lock(&control_in).wait().ok().flatten();
            // A session atelier closed says so itself, in `drop`, without waiting for this thread.
            if closing_in.load(Ordering::SeqCst) {
                return;
            }
            let stderr = lock(&control_in).stderr();
            let events = lock(&protocol_in).exited(code, &stderr, Instant::now());
            events.into_iter().for_each(|event| sink_in(event));
        });

        Self { lines: Some(lines), protocol, control, closing, sink }
    }
}

/// Writes a step's lines, then hands its events to the sink. The caller holds the protocol's lock, so the
/// lines and events of two steps never interleave.
fn deliver(step: Step, lines: &mpsc::Sender<String>, sink: &EventSink) {
    step.lines.into_iter().for_each(|line| drop(lines.send(line)));
    step.events.into_iter().for_each(|event| sink(event));
}

/// The protocol is plain data that a panic on the other thread cannot leave half-written, so a poisoned
/// lock is still good to read.
fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn write_lines(mut stdin: Box<dyn Write + Send>, queued: mpsc::Receiver<String>) {
    for line in queued {
        if writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_err() {
            return;
        }
    }
}

impl Session for AcpSession {
    fn send(&self, command: Command) -> Result<(), SessionError> {
        let lines = self.lines.as_ref().ok_or(SessionError::Closed)?;
        let mut protocol = lock(&self.protocol);
        let step = protocol.command(command)?;
        deliver(step, lines, &self.sink);
        Ok(())
    }
}

impl Drop for AcpSession {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::SeqCst);
        // Closing stdin asks the agent to stop; the kill covers one that does not listen.
        self.lines = None;
        let _ = lock(&self.control).kill();
        // The process may have children that hold its pipes open, so the reader can wait long for an end
        // of stream. The session ends now.
        let events = lock(&self.protocol).closed();
        events.into_iter().for_each(|event| (self.sink)(event));
    }
}
