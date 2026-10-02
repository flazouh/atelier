use std::{
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc},
    thread,
    time::Instant,
};

use atelier_project::{Control, Process};

use super::super::protocol::Protocol;
use crate::{
    session::{Command, EventSink, Session, SessionError},
    subprocess,
};
use super::types::Lines;
use super::helpers::{deliver, lock, write_lines};

pub(in super::super) struct AcpSession {
    pub(super) lines: Lines,
    pub(super) protocol: Arc<Mutex<Protocol>>,
    pub(super) control: Arc<Mutex<Box<dyn Control>>>,
    pub(super) closing: Arc<AtomicBool>,
    pub(super) sink: EventSink,
}

impl AcpSession {
    /// Drives `process` with `protocol`, writing `first` (the handshake's opening line) at once.
    pub(in super::super) fn run(process: Process, protocol: Protocol, first: Vec<String>, sink: EventSink) -> Self {
        let Process { stdin, stdout, control } = process;
        let protocol = Arc::new(Mutex::new(protocol));
        let control = Arc::new(Mutex::new(control));
        let closing = Arc::new(AtomicBool::new(false));

        let (sender, queued) = mpsc::channel::<String>();
        first.into_iter().for_each(|line| drop(sender.send(line)));
        thread::spawn(move || write_lines(stdin, queued));
        let lines: Lines = Arc::new(Mutex::new(Some(sender)));

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
                let _ = deliver(step, &lines_in, &sink_in);
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

        Self { lines, protocol, control, closing, sink }
    }
}

impl Session for AcpSession {
    fn send(&self, command: Command) -> Result<(), SessionError> {
        if lock(&self.lines).is_none() {
            return Err(SessionError::Closed);
        }
        let mut protocol = lock(&self.protocol);
        let step = protocol.command(command, Instant::now())?;
        deliver(step, &self.lines, &self.sink)
    }
}

impl Drop for AcpSession {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::SeqCst);
        // Closing stdin asks the agent to stop; the kill covers one that does not listen.
        lock(&self.lines).take();
        let _ = lock(&self.control).kill();
        // The process may have children that hold its pipes open, so the reader can wait long for an end
        // of stream. The session ends now.
        let events = lock(&self.protocol).closed();
        events.into_iter().for_each(|event| (self.sink)(event));
    }
}
