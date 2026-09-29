//! The app's side: [`RemoteProject`] is a [`Project`] whose every call is a request to
//! `lathe-remote` on the host, answered within a timeout.
//!
//! A connection comes from a [`Dial`], so the same client runs over the user's `ssh` in the app and
//! over an in-process pipe in the tests. When the connection drops, every call waiting on it fails at
//! once, the link reports [`Link::Down`], and the client dials again, backing off, for as long as it
//! lives. Once it is back it says hello again, restores the watch, and reports [`Link::Up`]. What the
//! app has not saved lives in the app, so a drop loses nothing.

use std::{
    collections::HashMap,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use lathe_project::{
    ChangeSink, Command, Control, Entry, GitOutput, Link, LinkSink, Match, Process, Project, Query, Watch,
};

use crate::protocol::{Call, Event, Failure, Frame, Pid, Reply, VERSION, read_frame, write_frame};

/// One live connection to `lathe-remote`: what the host says, what to send it, and a way to end it.
pub struct Connection {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    /// Ends the connection's transport, such as the `ssh` process. It may be called twice.
    pub close: Box<dyn FnMut() + Send>,
    /// The transport's own last words, such as ssh's stderr, for when it ends.
    pub last_words: Box<dyn Fn() -> String + Send>,
}

/// Makes a new connection: the first one, and each one after a drop.
pub type Dial = Box<dyn Fn() -> io::Result<Connection> + Send + Sync>;

/// How long a call may wait for its answer.
#[derive(Clone, Copy, Debug)]
pub struct Timeouts {
    /// A call that walks the tree or runs git: a listing, a search.
    pub slow: Duration,
    /// Every other call.
    pub quick: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self { slow: Duration::from_secs(60), quick: Duration::from_secs(30) }
    }
}

impl Timeouts {
    fn of(&self, call: &Call) -> Duration {
        match call {
            Call::List | Call::Search { .. } | Call::Git { .. } => self.slow,
            _ => self.quick,
        }
    }
}

/// The waits between dials after a drop: a quick retry first, then up to 30 s apart.
fn backoff(attempt: u32) -> Duration {
    Duration::from_millis((500u64 << attempt.min(6)).min(30_000))
}

type Answer = Result<Reply, Failure>;

/// A wait in words: "30 s", or "200 ms" under a second.
fn span(wait: Duration) -> String {
    match wait.as_secs() {
        0 => format!("{} ms", wait.as_millis()),
        s => format!("{s} s"),
    }
}

/// A process on the host, as the client sees it.
struct Pipes {
    output: Option<mpsc::Sender<Vec<u8>>>,
    reader: Option<mpsc::Receiver<Vec<u8>>>,
    exit: Arc<(Mutex<Option<Option<i32>>>, Condvar)>,
}

impl Default for Pipes {
    /// A new process's pipes: its stdout's channel open, its exit not known.
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { output: Some(tx), reader: Some(rx), exit: Arc::default() }
    }
}

impl Pipes {
    /// The process is gone: its stdout ends, and waiting on it returns `code`.
    fn end(&mut self, code: Option<i32>) {
        self.output = None;
        let (lock, done) = &*self.exit;
        let mut exit = lock.lock().unwrap_or_else(|p| p.into_inner());
        if exit.is_none() {
            *exit = Some(code);
        }
        done.notify_all();
    }
}

struct Shared {
    host: String,
    timeouts: Timeouts,
    /// The folder the app asked for, as it asked (`~/code/lathe`), for each hello.
    asked_root: String,
    dial: Dial,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    closer: Mutex<Option<Box<dyn FnMut() + Send>>>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Answer>>>,
    next_id: AtomicU64,
    processes: Mutex<HashMap<Pid, Pipes>>,
    watchers: Mutex<HashMap<u64, ChangeSink>>,
    links: Mutex<Vec<LinkSink>>,
    down: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn not_connected(host: &str, why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, format!("not connected to {host}: {why}"))
}

impl Shared {
    fn send(&self, frame: &Frame) -> io::Result<()> {
        let mut writer = lock(&self.writer);
        let writer = writer.as_mut().ok_or_else(|| not_connected(&self.host, "reconnecting"))?;
        write_frame(writer, frame)
    }

    fn request(&self, call: Call) -> io::Result<Reply> {
        if let Some(why) = lock(&self.down).clone() {
            return Err(not_connected(&self.host, &why));
        }
        let timeout = self.timeouts.of(&call);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = mpsc::channel();
        lock(&self.pending).insert(id, tx);
        if let Err(error) = self.send(&Frame::Request { id, call }) {
            lock(&self.pending).remove(&id);
            return Err(error);
        }
        match rx.recv_timeout(timeout) {
            Ok(answer) => answer.map_err(io::Error::from),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                lock(&self.pending).remove(&id);
                Err(io::Error::new(io::ErrorKind::TimedOut, format!("{} did not answer in {}", self.host, span(timeout))))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(not_connected(&self.host, "the connection dropped")),
        }
    }

    /// A request whose answer nobody waits for, such as bytes for a process's stdin: the host keeps
    /// their order.
    fn tell(&self, call: Call) -> io::Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.send(&Frame::Request { id, call })
    }

    fn report(&self, link: Link) {
        for sink in lock(&self.links).iter() {
            sink(link.clone());
        }
    }

    fn hello(&self) -> io::Result<String> {
        match self.request(Call::Hello { version: VERSION, root: self.asked_root.clone() })? {
            Reply::Hello { root } => Ok(root),
            other => Err(io::Error::other(format!("a hello answered with {other:?}"))),
        }
    }

    fn handle(&self, frame: Frame) {
        match frame {
            Frame::Response { id, result } => {
                if let Some(tx) = lock(&self.pending).remove(&id) {
                    let _ = tx.send(result);
                }
            }
            Frame::Event(Event::Changes(changes)) => {
                for sink in lock(&self.watchers).values() {
                    sink(changes.clone());
                }
            }
            Frame::Event(Event::Output { pid, bytes }) => {
                let mut processes = lock(&self.processes);
                let pipes = processes.entry(pid).or_default();
                if let Some(tx) = &pipes.output {
                    let _ = tx.send(bytes);
                }
            }
            Frame::Event(Event::Exited { pid, code }) => lock(&self.processes).entry(pid).or_default().end(code),
            Frame::Request { .. } => {}
        }
    }

    /// The connection is gone: everything waiting on it fails now, and the link says why.
    fn dropped(&self, why: String) {
        *lock(&self.writer) = None;
        if let Some(mut close) = lock(&self.closer).take() {
            close();
        }
        *lock(&self.down) = Some(why.clone());
        lock(&self.pending).clear();
        for pipes in lock(&self.processes).values_mut() {
            pipes.end(None);
        }
        self.report(Link::Down(why));
    }
}

/// Reads frames until the connection ends, then starts dialing again.
fn read_loop(shared: Weak<Shared>, mut reader: Box<dyn Read + Send>, last_words: Box<dyn Fn() -> String + Send>) {
    let ended = loop {
        match read_frame(&mut reader) {
            Ok(Some(frame)) => match shared.upgrade() {
                Some(shared) => shared.handle(frame),
                None => return,
            },
            Ok(None) => break "the connection closed".to_string(),
            Err(error) => break error.to_string(),
        }
    };
    let Some(shared) = shared.upgrade() else { return };
    let words = last_words();
    let why = match words.trim() {
        "" => ended,
        words => format!("{ended}: {}", words.lines().last().unwrap_or(words)),
    };
    shared.dropped(why);
    let weak = Arc::downgrade(&shared);
    drop(shared);
    thread::spawn(move || redial(weak));
}

fn attach(shared: &Arc<Shared>, connection: Connection) {
    *lock(&shared.writer) = Some(connection.writer);
    *lock(&shared.closer) = Some(connection.close);
    let weak = Arc::downgrade(shared);
    let (reader, last_words) = (connection.reader, connection.last_words);
    thread::spawn(move || read_loop(weak, reader, last_words));
}

/// Dials until a connection says hello, for as long as the project lives.
fn redial(shared: Weak<Shared>) {
    let mut attempt = 0;
    loop {
        thread::sleep(backoff(attempt));
        attempt += 1;
        let Some(shared) = shared.upgrade() else { return };
        let Ok(connection) = (shared.dial)() else { continue };
        attach(&shared, connection);
        // Calls wait on the hello, so the link is marked up for it alone.
        let was = lock(&shared.down).take();
        match shared.hello() {
            Ok(_) => {
                if !lock(&shared.watchers).is_empty() {
                    let _ = shared.request(Call::Watch);
                }
                shared.report(Link::Up);
                return;
            }
            Err(_) => *lock(&shared.down) = was,
        }
    }
}

pub struct RemoteProject {
    shared: Arc<Shared>,
    /// The folder on the host, as the host resolved it.
    root: PathBuf,
}

impl RemoteProject {
    /// Dials, says hello for `root` on `host`, and returns the project once the host has opened it.
    pub fn connect(host: impl Into<String>, root: impl Into<String>, dial: Dial, timeouts: Timeouts) -> io::Result<Self> {
        let connection = dial()?;
        let shared = Arc::new(Shared {
            host: host.into(),
            timeouts,
            asked_root: root.into(),
            dial,
            writer: Mutex::default(),
            closer: Mutex::default(),
            pending: Mutex::default(),
            next_id: AtomicU64::new(0),
            processes: Mutex::default(),
            watchers: Mutex::default(),
            links: Mutex::default(),
            down: Mutex::default(),
        });
        attach(&shared, connection);
        let root = shared.hello()?;
        Ok(Self { shared, root: PathBuf::from(root) })
    }

    fn call(&self, call: Call) -> io::Result<Reply> {
        self.shared.request(call)
    }
}

impl Drop for RemoteProject {
    fn drop(&mut self) {
        *lock(&self.shared.writer) = None;
        if let Some(mut close) = lock(&self.shared.closer).take() {
            close();
        }
    }
}

fn unexpected(reply: Reply) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("an unexpected answer: {reply:?}"))
}

impl Project for RemoteProject {
    fn root(&self) -> &Path {
        &self.root
    }

    fn list(&self) -> io::Result<Vec<Entry>> {
        match self.call(Call::List)? {
            Reply::Entries(entries) => Ok(entries),
            other => Err(unexpected(other)),
        }
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        match self.call(Call::Read { path: path.into() })? {
            Reply::Bytes(bytes) => Ok(bytes),
            other => Err(unexpected(other)),
        }
    }

    fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        match self.call(Call::Write { path: path.into(), bytes: bytes.to_vec() })? {
            Reply::Done => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    fn watch(&self, sink: ChangeSink) -> io::Result<Watch> {
        let first = lock(&self.shared.watchers).is_empty();
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.shared.watchers).insert(id, sink);
        if first && let Err(error) = self.call(Call::Watch) {
            lock(&self.shared.watchers).remove(&id);
            return Err(error);
        }
        // Dropping the handle stops this sink; the host keeps watching for any others.
        struct Unwatch(Weak<Shared>, u64);
        impl Drop for Unwatch {
            fn drop(&mut self) {
                if let Some(shared) = self.0.upgrade() {
                    lock(&shared.watchers).remove(&self.1);
                }
            }
        }
        Ok(Watch::new(Unwatch(Arc::downgrade(&self.shared), id)))
    }

    fn search(&self, query: &Query) -> io::Result<Vec<Match>> {
        match self.call(Call::Search { query: query.clone() })? {
            Reply::Matches(matches) => Ok(matches),
            other => Err(unexpected(other)),
        }
    }

    fn spawn(&self, command: &Command) -> io::Result<Process> {
        let pid = match self.call(Call::Spawn { command: command.clone() })? {
            Reply::Spawned { pid } => pid,
            other => return Err(unexpected(other)),
        };
        // Output may have arrived before the answer did: it waits in the same pipes.
        let (reader, exit) = {
            let mut processes = lock(&self.shared.processes);
            let pipes = processes.entry(pid).or_default();
            (pipes.reader.take().expect("a process is spawned once"), pipes.exit.clone())
        };
        let weak = Arc::downgrade(&self.shared);
        Ok(Process {
            stdin: Box::new(RemoteStdin { shared: weak.clone(), pid }),
            stdout: Box::new(RemoteStdout { rx: reader, left: Vec::new() }),
            control: Box::new(RemoteControl { shared: weak, pid, exit }),
        })
    }

    fn git(&self, args: &[&str]) -> io::Result<GitOutput> {
        match self.call(Call::Git { args: args.iter().map(|a| a.to_string()).collect() })? {
            Reply::Git(out) => Ok(out),
            other => Err(unexpected(other)),
        }
    }

    fn on_link(&self, sink: LinkSink) {
        lock(&self.shared.links).push(sink);
    }

    fn host(&self) -> Option<&str> {
        Some(&self.shared.host)
    }
}

struct RemoteStdin {
    shared: Weak<Shared>,
    pid: Pid,
}

impl Write for RemoteStdin {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let shared = self.shared.upgrade().ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "the project closed"))?;
        shared.tell(Call::Input { pid: self.pid, bytes: bytes.to_vec() })?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for RemoteStdin {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            let _ = shared.tell(Call::CloseInput { pid: self.pid });
        }
    }
}

struct RemoteStdout {
    rx: mpsc::Receiver<Vec<u8>>,
    left: Vec<u8>,
}

impl Read for RemoteStdout {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.left.is_empty() {
            match self.rx.recv() {
                Ok(bytes) => self.left = bytes,
                // The process ended, or the connection did.
                Err(_) => return Ok(0),
            }
        }
        let n = self.left.len().min(out.len());
        out[..n].copy_from_slice(&self.left[..n]);
        self.left.drain(..n);
        Ok(n)
    }
}

struct RemoteControl {
    shared: Weak<Shared>,
    pid: Pid,
    exit: Arc<(Mutex<Option<Option<i32>>>, Condvar)>,
}

impl Control for RemoteControl {
    fn kill(&mut self) -> io::Result<()> {
        let shared = self.shared.upgrade().ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "the project closed"))?;
        shared.request(Call::Kill { pid: self.pid }).map(|_| ())
    }

    fn wait(&mut self) -> io::Result<Option<i32>> {
        let (lock_, done) = &*self.exit;
        let mut exit = lock(lock_);
        while exit.is_none() {
            exit = done.wait(exit).unwrap_or_else(|p| p.into_inner());
        }
        Ok(exit.expect("set before the wait ends"))
    }

    fn running(&mut self) -> bool {
        lock(&self.exit.0).is_none()
    }

    fn stderr(&self) -> String {
        let Some(shared) = self.shared.upgrade() else { return String::new() };
        match shared.request(Call::Stderr { pid: self.pid }) {
            Ok(Reply::Text(text)) => text,
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests;
