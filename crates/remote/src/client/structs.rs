use std::{
    collections::HashMap,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, Weak, atomic::{AtomicU64, Ordering}, mpsc},
    time::Duration,
};

use atelier_project::{
    ChangeSink, Command, Control, DataEntry, Entry, GitOutput, Link, LinkSink, Match, Process,
    Project, Query, Watch,
};

use crate::protocol::{Call, Event, Frame, Pid, Reply, VERSION, write_frame};
use super::types::{Answer, Dial};
use super::helpers::{attach, lock, not_connected, span, unexpected};
use super::tracker;

/// One live connection to `atelier-remote`: what the host says, what to send it, and a way to end it.
pub struct Connection {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    /// Ends the connection's transport, such as the `ssh` process. It may be called twice.
    pub close: Box<dyn FnMut() + Send>,
    /// The transport's own last words, such as ssh's stderr, for when it ends.
    pub last_words: Box<dyn Fn() -> String + Send>,
}

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
    pub(super) fn of(&self, call: &Call) -> Duration {
        match call {
            Call::List | Call::Search { .. } | Call::Git { .. } => self.slow,
            Call::At { call, .. } => self.of(call),
            _ => self.quick,
        }
    }
}

/// A process on the host, as the client sees it.
pub(super) struct Pipes {
    output: Option<mpsc::Sender<Vec<u8>>>,
    pub(super) reader: Option<mpsc::Receiver<Vec<u8>>>,
    pub(super) exit: Arc<(Mutex<Option<Option<i32>>>, Condvar)>,
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
    pub(super) fn end(&mut self, code: Option<i32>) {
        self.output = None;
        let (lock, done) = &*self.exit;
        let mut exit = lock.lock().unwrap_or_else(|p| p.into_inner());
        if exit.is_none() {
            *exit = Some(code);
        }
        done.notify_all();
    }
}

pub(super) struct Shared {
    pub(super) host: String,
    pub(super) timeouts: Timeouts,
    /// The folder the app asked for, as it asked (`~/code/atelier`), for each hello.
    asked_root: String,
    pub(super) dial: Dial,
    pub(super) writer: Mutex<Option<Box<dyn Write + Send>>>,
    pub(super) closer: Mutex<Option<Box<dyn FnMut() + Send>>>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Answer>>>,
    next_id: AtomicU64,
    pub(super) processes: Mutex<HashMap<Pid, Pipes>>,
    /// Each sink with the folder it watches: `None` for the project's, a worktree's by its name.
    pub(super) watchers: Mutex<HashMap<u64, (Option<String>, ChangeSink)>>,
    pub(super) links: Mutex<Vec<LinkSink>>,
    pub(super) down: Mutex<Option<String>>,
}

impl Shared {
    pub(super) fn send(&self, frame: &Frame) -> io::Result<()> {
        let mut writer = lock(&self.writer);
        let writer = writer.as_mut().ok_or_else(|| not_connected(&self.host, "reconnecting"))?;
        write_frame(writer, frame)
    }

    pub(super) fn request(&self, call: Call) -> io::Result<Reply> {
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
    pub(super) fn tell(&self, call: Call) -> io::Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.send(&Frame::Request { id, call })
    }

    pub(super) fn report(&self, link: Link) {
        for sink in lock(&self.links).iter() {
            sink(link.clone());
        }
    }

    pub(super) fn hello(&self) -> io::Result<String> {
        let answer = self.request(Call::Hello { version: VERSION, root: self.asked_root.clone() }).map_err(|error| {
            // A helper of another protocol: a new one says so in words, an old one with its numbers.
            let words = error.to_string();
            if error.kind() == io::ErrorKind::InvalidInput && (words.contains("speaks version") || words.contains("update atelier")) {
                io::Error::new(io::ErrorKind::InvalidInput, "the atelier helper on this host is a different version: update atelier on this host")
            } else {
                error
            }
        });
        match answer? {
            Reply::Hello { root } => Ok(root),
            other => Err(io::Error::other(format!("a hello answered with {other:?}"))),
        }
    }

    pub(super) fn handle(&self, frame: Frame) {
        match frame {
            Frame::Response { id, result } => {
                if let Some(tx) = lock(&self.pending).remove(&id) {
                    let _ = tx.send(result);
                }
            }
            Frame::Event(Event::Changes(changes)) => self.changed(None, changes),
            Frame::Event(Event::ChangesAt { root, changes }) => self.changed(Some(&root), changes),
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

    fn changed(&self, root: Option<&str>, changes: Vec<atelier_project::Change>) {
        for (_, sink) in lock(&self.watchers).values().filter(|(at, _)| at.as_deref() == root) {
            sink(changes.clone());
        }
    }

    /// The connection is gone: everything waiting on it fails now, and the link says why.
    pub(super) fn dropped(&self, why: String) {
        *lock(&self.writer) = None;
        if let Some(mut close) = lock(&self.closer).take() {
            close();
        }
        *lock(&self.down) = Some(why.clone());
        lock(&self.pending).clear();
        // The host's processes ended with it, and the next host numbers its own from the start.
        for (_, mut pipes) in lock(&self.processes).drain() {
            pipes.end(None);
        }
        self.report(Link::Down(why));
    }
}

/// Ends the connection when the project and every worktree seen from it are gone.
pub(super) struct Open(Arc<Shared>);

impl Drop for Open {
    fn drop(&mut self) {
        *lock(&self.0.writer) = None;
        if let Some(mut close) = lock(&self.0.closer).take() {
            close();
        }
    }
}

pub struct RemoteProject {
    pub(super) shared: Arc<Shared>,
    _open: Arc<Open>,
    /// The folder on the host, as the host resolved it.
    pub(super) root: PathBuf,
    /// `None` for the project's own folder; a worktree's name on the host, which each call is sent `At`.
    scope: Option<String>,
    /// The data folder on the host, once the host has said: it never moves, so it is asked once. A call
    /// that failed (the link was down) is asked again next time. Shared with the project's worktrees.
    pub(super) data_path: Arc<Mutex<Option<Option<PathBuf>>>>,
    pub(super) tracker: Arc<atelier_project::TrackerSlot>,
    /// How often the tracker asks the host for changes made elsewhere, while someone listens.
    poll_tasks: Duration,
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
        Ok(Self {
            _open: Arc::new(Open(shared.clone())),
            shared,
            root: PathBuf::from(root),
            scope: None,
            data_path: Arc::default(),
            tracker: Arc::default(),
            poll_tasks: tracker::POLL,
        })
    }

    pub(super) fn call(&self, call: Call) -> io::Result<Reply> {
        match &self.scope {
            Some(root) => self.shared.request(Call::At { root: root.clone(), call: Box::new(call) }),
            None => self.shared.request(call),
        }
    }

    /// A tracker that asks the host for changes this often, as a test wants.
    #[cfg(test)]
    pub(crate) fn poll_tasks_every(mut self, every: Duration) -> Self {
        self.poll_tasks = every;
        self
    }
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

    fn remove(&self, path: &str) -> io::Result<()> {
        match self.call(Call::Remove { path: path.into() })? {
            Reply::Done => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    fn apply(&self, op: &atelier_project::FsOp) -> io::Result<()> {
        match self.call(Call::Fs { op: op.clone() })? {
            Reply::Done => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    fn data_read(&self, path: &str) -> io::Result<Vec<u8>> {
        match self.call(Call::DataRead { path: path.into() })? {
            Reply::Bytes(bytes) => Ok(bytes),
            other => Err(unexpected(other)),
        }
    }

    fn data_write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        match self.call(Call::DataWrite { path: path.into(), bytes: bytes.to_vec() })? {
            Reply::Done => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    fn data_list(&self, prefix: &str) -> io::Result<Vec<DataEntry>> {
        match self.call(Call::DataList { prefix: prefix.into() })? {
            Reply::DataEntries(entries) => Ok(entries),
            other => Err(unexpected(other)),
        }
    }

    fn read_dir(&self, dir: &str) -> io::Result<Vec<atelier_project::DirEntry>> {
        match self.call(Call::ReadDir { dir: dir.into() })? {
            Reply::DirEntries(entries) => Ok(entries),
            other => Err(unexpected(other)),
        }
    }

    fn data_path(&self) -> Option<PathBuf> {
        if let Some(known) = lock(&self.data_path).clone() {
            return known;
        }
        let answer = match self.call(Call::DataPath) {
            Ok(Reply::Text(path)) => Some(PathBuf::from(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            // The link is down, or an old host: not an answer to keep.
            _ => return None,
        };
        *lock(&self.data_path) = Some(answer.clone());
        answer
    }

    fn tracker(&self) -> atelier_tracker::TrackerResult<Arc<dyn atelier_tracker::Tracker>> {
        self.tracker.get_or_open(|| Ok(Arc::new(tracker::RemoteTracker::open(self.shared.clone(), self.poll_tasks)?)))
    }

    fn watch(&self, sink: ChangeSink) -> io::Result<Watch> {
        let first = !lock(&self.shared.watchers).values().any(|(at, _)| *at == self.scope);
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.shared.watchers).insert(id, (self.scope.clone(), sink));
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
            let reader = pipes.reader.take().ok_or_else(|| io::Error::other(format!("process {pid} was handed out already")))?;
            (reader, pipes.exit.clone())
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

    fn at(&self, folder: &Path) -> io::Result<Arc<dyn Project>> {
        let root = match self.shared.request(Call::Open { root: folder.to_string_lossy().into_owned() })? {
            Reply::Hello { root } => root,
            other => return Err(unexpected(other)),
        };
        Ok(Arc::new(Self {
            shared: self.shared.clone(),
            _open: self._open.clone(),
            root: PathBuf::from(&root),
            scope: Some(root),
            data_path: self.data_path.clone(),
            tracker: self.tracker.clone(),
            poll_tasks: self.poll_tasks,
        }))
    }
}

struct RemoteStdin {
    pub(super) shared: Weak<Shared>,
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
    pub(super) rx: mpsc::Receiver<Vec<u8>>,
    pub(super) left: Vec<u8>,
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
    pub(super) shared: Weak<Shared>,
    pid: Pid,
    pub(super) exit: Arc<(Mutex<Option<Option<i32>>>, Condvar)>,
}

impl Drop for RemoteControl {
    /// Nobody can ask about the process any more: its pipes are forgotten.
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            lock(&shared.processes).remove(&self.pid);
        }
    }
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
