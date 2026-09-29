use std::{
    io::{PipeWriter, Read, Write},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use lathe_project::{ChangeKind, Command, Link, Project, Query};

use super::*;

/// The host's writer, which a test can cut: the app then reads the end of the connection.
#[derive(Clone, Default)]
struct Cut(Arc<Mutex<Option<PipeWriter>>>);

impl Write for Cut {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match lock(&self.0).as_mut() {
            Some(w) => w.write(bytes),
            None => Err(io::Error::new(io::ErrorKind::BrokenPipe, "cut")),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        lock(&self.0).as_mut().map_or(Ok(()), |w| w.flush())
    }
}

/// A host in this process: each dial serves the folder over new pipes. `cuts` hands the test each
/// connection's cut, and `refuse` makes the next dials fail, as an unreachable host does.
struct Host {
    cuts: mpsc::Receiver<Cut>,
    refuse: Arc<Mutex<bool>>,
}

fn host() -> (Host, Dial) {
    let (tx, cuts) = mpsc::channel();
    let refuse = Arc::new(Mutex::new(false));
    let refusing = refuse.clone();
    let dial: Dial = Box::new(move || {
        if *lock(&refusing) {
            return Err(io::Error::new(io::ErrorKind::ConnectionRefused, "ssh: connect to host test port 22: Connection refused"));
        }
        let (app_reader, host_writer) = io::pipe()?;
        let (host_reader, app_writer) = io::pipe()?;
        let cut = Cut(Arc::new(Mutex::new(Some(host_writer))));
        let _ = tx.send(cut.clone());
        thread::spawn(move || crate::server::serve(host_reader, cut));
        Ok(Connection {
            reader: Box::new(app_reader),
            writer: Box::new(app_writer),
            close: Box::new(|| {}),
            last_words: Box::new(String::new),
        })
    });
    (Host { cuts, refuse }, dial)
}

fn folder(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let at = dir.path().join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    dir
}

fn connect(dir: &tempfile::TempDir, dial: Dial) -> RemoteProject {
    RemoteProject::connect("test", dir.path().display().to_string(), dial, Timeouts::default()).unwrap()
}

#[test]
fn files_search_and_git_answer_as_they_do_locally() {
    let dir = folder(&[("src/a.rs", "fn detach() {}\n"), (".gitignore", "target/\n"), ("target/x", "")]);
    let (_host, dial) = host();
    let remote = connect(&dir, dial);
    let local = lathe_project::LocalProject::open(dir.path()).unwrap();
    assert_eq!(remote.root(), local.root());
    assert_eq!(remote.list().unwrap(), local.list().unwrap());
    remote.write("src/b.rs", b"fn b() {}\n").unwrap();
    assert_eq!(remote.read("src/b.rs").unwrap(), b"fn b() {}\n");
    assert_eq!(remote.read("missing.rs").unwrap_err().kind(), io::ErrorKind::NotFound);
    assert_eq!(remote.read("../escape").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    let query = Query { pattern: "detach".into(), regex: false, case_sensitive: true, limit: 10 };
    assert_eq!(remote.search(&query).unwrap(), local.search(&query).unwrap());
    assert!(!remote.git(&["status"]).unwrap().ok(), "no repository");
}

#[test]
fn a_process_on_the_host_talks_over_its_pipes() {
    let dir = folder(&[]);
    let (_host, dial) = host();
    let remote = connect(&dir, dial);
    let mut cat = remote.spawn(&Command::new("cat")).unwrap();
    cat.stdin.write_all(b"one\n").unwrap();
    cat.stdin.write_all(b"two\n").unwrap();
    drop(cat.stdin);
    let mut out = String::new();
    cat.stdout.read_to_string(&mut out).unwrap();
    assert_eq!(out, "one\ntwo\n", "in order, and it ends when stdin closes");
    assert_eq!(cat.control.wait().unwrap(), Some(0));
    let mut failing = remote.spawn(&Command::new("sh").args(["-c", "echo nope >&2; exit 4"])).unwrap();
    assert_eq!(failing.control.wait().unwrap(), Some(4));
    let deadline = Instant::now() + Duration::from_secs(2);
    while failing.control.stderr().is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(failing.control.stderr(), "nope\n");
    let mut sleeper = remote.spawn(&Command::new("sleep").args(["30"])).unwrap();
    assert!(sleeper.control.running());
    sleeper.control.kill().unwrap();
    sleeper.control.wait().unwrap();
    assert!(remote.spawn(&Command::new("lathe-no-such-program")).is_err());
    drop((cat.control, failing.control, sleeper.control));
    assert!(lock(&remote.shared.processes).is_empty(), "a process is forgotten once its control is gone");
}

#[test]
fn a_watch_on_the_host_reports_here() {
    let dir = folder(&[("a.txt", "a")]);
    let (_host, dial) = host();
    let remote = connect(&dir, dial);
    let (tx, rx) = mpsc::channel();
    let _watch = remote.watch(Box::new(move |batch| drop(tx.send(batch)))).unwrap();
    thread::sleep(Duration::from_millis(200));
    std::fs::write(dir.path().join("a.txt"), "b").unwrap();
    let batch = rx.recv_timeout(Duration::from_secs(3)).expect("a batch arrives");
    assert!(batch.iter().any(|c| c.path == "a.txt" && c.kind != ChangeKind::Removed), "{batch:?}");
}

#[test]
fn a_dropped_connection_fails_calls_at_once_then_comes_back() {
    let dir = folder(&[("a.txt", "a")]);
    let (host, dial) = host();
    let remote = connect(&dir, dial);
    let (tx, links) = mpsc::channel();
    remote.on_link(Box::new(move |link| drop(tx.send(link))));
    let (changes_tx, changes) = mpsc::channel();
    let _watch = remote.watch(Box::new(move |batch| drop(changes_tx.send(batch)))).unwrap();
    let first = host.cuts.recv().unwrap();
    // The host stays away for a while: dials fail.
    *lock(&host.refuse) = true;
    lock(&first.0).take();
    let down = links.recv_timeout(Duration::from_secs(2)).expect("the drop is reported");
    assert!(matches!(down, Link::Down(_)), "{down:?}");
    let at = Instant::now();
    let error = remote.read("a.txt").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotConnected);
    assert!(at.elapsed() < Duration::from_millis(100), "a call fails at once, not after a timeout");
    *lock(&host.refuse) = false;
    assert_eq!(links.recv_timeout(Duration::from_secs(10)).expect("it comes back"), Link::Up);
    assert_eq!(remote.read("a.txt").unwrap(), b"a");
    // The watch came back with it.
    thread::sleep(Duration::from_millis(200));
    std::fs::write(dir.path().join("a.txt"), "c").unwrap();
    assert!(changes.recv_timeout(Duration::from_secs(3)).is_ok(), "the watch reports after the reconnect");
}

#[test]
fn a_host_that_never_answers_times_out() {
    let (app_reader, _host_writer) = io::pipe().unwrap();
    let silent: Dial = Box::new(move || {
        Ok(Connection {
            reader: Box::new(app_reader.try_clone()?),
            writer: Box::new(io::sink()),
            close: Box::new(|| {}),
            last_words: Box::new(String::new),
        })
    });
    let at = Instant::now();
    let timeouts = Timeouts { slow: Duration::from_millis(200), quick: Duration::from_millis(200) };
    let error = RemoteProject::connect("slow", "/", silent, timeouts).err().expect("no answer");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(error.to_string(), "slow did not answer in 200 ms");
    assert!(at.elapsed() < Duration::from_secs(2));
}

/// After a reconnect the host numbers its processes from the start again; a new process must not
/// meet the old one's pipes.
#[test]
fn a_process_starts_after_a_reconnect() {
    let dir = folder(&[]);
    let (host, dial) = host();
    let remote = connect(&dir, dial);
    let (tx, links) = mpsc::channel();
    remote.on_link(Box::new(move |link| drop(tx.send(link))));
    let mut before = remote.spawn(&Command::new("sleep").args(["30"])).unwrap();
    lock(&host.cuts.recv().unwrap().0).take();
    assert!(matches!(links.recv_timeout(Duration::from_secs(2)).unwrap(), Link::Down(_)));
    assert_eq!(before.control.wait().unwrap(), None, "the old process ended with its connection");
    assert_eq!(links.recv_timeout(Duration::from_secs(10)).unwrap(), Link::Up);
    let mut after = remote.spawn(&Command::new("echo").args(["again"])).unwrap();
    let mut out = String::new();
    after.stdout.read_to_string(&mut out).unwrap();
    assert_eq!(out, "again\n");
}
