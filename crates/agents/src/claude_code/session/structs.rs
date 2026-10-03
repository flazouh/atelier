use std::{
    sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc},
    thread,
    time::Instant,
};

use atelier_project::{Control, Process};

use super::super::{control, map::{ClaudeLineMapper, LineMapper}};
use crate::{
    session::{Command, EventSink, Session, SessionError},
    subprocess,
};
use super::helpers::{lock, write_lines};

pub(in super::super) struct ClaudeSession {
    pub(super) lines: Option<mpsc::Sender<String>>,
    mapper: Arc<Mutex<dyn LineMapper + Send>>,
    control: Arc<Mutex<Box<dyn Control>>>,
    closing: Arc<AtomicBool>,
    sink: EventSink,
    next_request: AtomicU64,
    message_ids: control::MessageIds,
}

impl ClaudeSession {
    pub(in super::super) fn run(process: Process, sink: EventSink) -> Self {
        let Process { stdin, stdout, control } = process;
        let mapper = Arc::new(Mutex::new(ClaudeLineMapper::new()));
        let control = Arc::new(Mutex::new(control));
        let closing = Arc::new(AtomicBool::new(false));

        let (lines, queued) = mpsc::channel::<String>();
        thread::spawn(move || write_lines(stdin, queued));

        let (mapper_in, control_in, closing_in, sink_in) = (mapper.clone(), control.clone(), closing.clone(), sink.clone());
        thread::spawn(move || {
            for line in subprocess::lines(stdout) {
                if closing_in.load(Ordering::SeqCst) {
                    return;
                }
                let events = lock(&mapper_in).line(&line, Instant::now());
                events.into_iter().for_each(|event| sink_in(event));
            }
            let code = lock(&control_in).wait().ok().flatten();
            // A session atelier closed says so itself, in `drop`, without waiting for this thread.
            if closing_in.load(Ordering::SeqCst) {
                return;
            }
            // `wait` returns with the stderr complete, so the tail says why the process stopped.
            let stderr = lock(&control_in).stderr();
            let events = lock(&mapper_in).exited(code, &stderr);
            events.into_iter().for_each(|event| sink_in(event));
        });

        Self { lines: Some(lines), mapper, control, closing, sink, next_request: AtomicU64::new(1), message_ids: control::MessageIds::new() }
    }

    fn request_id(&self) -> String {
        format!("atelier-{}", self.next_request.fetch_add(1, Ordering::Relaxed))
    }

    fn write(&self, line: String) -> Result<(), SessionError> {
        self.lines.as_ref().ok_or(SessionError::Closed)?.send(line).map_err(|_| SessionError::Closed)
    }
}

impl Session for ClaudeSession {
    fn send(&self, command: Command) -> Result<(), SessionError> {
        let line = match command {
            Command::Send { text, attachments } => {
                let id = self.message_ids.next();
                lock(&self.mapper).user_sent(id.clone());
                control::user_message(&id, &text, &attachments)
            }
            Command::Answer { request, choice } => {
                let answer = lock(&self.mapper).answer(&request, &choice);
                answer.ok_or(SessionError::Unsupported("an answer to a request that is not waiting"))?
            }
            Command::Interrupt => control::interrupt(&self.request_id()),
            Command::SetModel { model } => control::set_model(&self.request_id(), &model),
            Command::SetPermissionMode { mode } => control::set_permission_mode(&self.request_id(), mode),
        };
        self.write(line)
    }
}

impl Drop for ClaudeSession {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::SeqCst);
        // Closing stdin asks `claude` to stop; the kill covers one that does not listen.
        self.lines = None;
        let _ = lock(&self.control).kill();
        // The process may have children that hold its pipes open, so the reader can wait long for an
        // end of stream. The session ends now.
        let events = lock(&self.mapper).closed();
        events.into_iter().for_each(|event| (self.sink)(event));
    }
}
