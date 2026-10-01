use std::{
    io::{PipeWriter, Read, Write},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use atelier_project::{ChangeKind, Command, Link, Project, Query};

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
    host_with_data(None)
}

/// A host whose projects keep their data folders under `data`.
fn host_with_data(data: Option<std::path::PathBuf>) -> (Host, Dial) {
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
        let data = data.clone();
        thread::spawn(move || crate::server::serve_with_data(host_reader, cut, data));
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
    let local = atelier_project::LocalProject::open(dir.path()).unwrap();
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
    assert!(remote.spawn(&Command::new("atelier-no-such-program")).is_err());
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

#[test]
fn a_remote_project_removes_files_and_keeps_its_data_on_the_host() {
    let dir = folder(&[("a.txt", "a")]);
    let data = tempfile::tempdir().unwrap();
    let (_host, dial) = host_with_data(Some(data.path().to_path_buf()));
    let remote = connect(&dir, dial);
    remote.remove("a.txt").unwrap();
    assert!(!dir.path().join("a.txt").exists());
    assert_eq!(remote.remove("a.txt").unwrap_err().kind(), io::ErrorKind::NotFound);
    remote.data_write("review/one.json", b"{}").unwrap();
    assert_eq!(remote.data_read("review/one.json").unwrap(), b"{}");
    let listed: Vec<String> = remote.data_list("review").unwrap().into_iter().map(|e| e.path).collect();
    assert_eq!(listed, ["review/one.json"]);
    assert_eq!(remote.data_write("../x", b"").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert!(std::fs::read_dir(data.path().join("projects")).unwrap().next().is_some(), "the data is on the host, under its data folder");
    assert!(!dir.path().join("review").exists());
}

/// A remote project names its data folder on the host, as the host's own project does, and asks once.
#[test]
fn a_remote_project_names_its_data_folder_on_the_host() {
    let dir = folder(&[("a.txt", "a")]);
    let data = tempfile::tempdir().unwrap();
    let (_host, dial) = host_with_data(Some(data.path().to_path_buf()));
    let remote = connect(&dir, dial);
    let local = atelier_project::LocalProject::open(dir.path()).unwrap().with_data_dir(data.path());
    assert_eq!(remote.data_path(), local.data_path());
    assert!(remote.data_path().is_some_and(|p| p.starts_with(data.path())));
}

/// A process that ended gives back its pipes at once: a burst of short commands (a commit runs one per
/// file) must not run the host out of open files while each is kept a while for a last Stderr ask.
#[cfg(target_os = "linux")]
#[test]
fn ended_processes_give_back_their_pipes() {
    let dir = folder(&[("a.txt", "a")]);
    let (_host, dial) = host();
    let remote = connect(&dir, dial);
    let open_files = || std::fs::read_dir("/proc/self/fd").unwrap().count();
    let run = || {
        let mut p = remote.spawn(&atelier_project::Command::new("true")).unwrap();
        drop(p.stdin);
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut p.stdout, &mut out).unwrap();
        p.control.wait().unwrap();
    };
    run();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let before = open_files();
    for _ in 0..60 {
        run();
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    let after = open_files();
    assert!(after <= before + 10, "{before} open files before 60 commands, {after} after");
}

/// A remote project lists a folder of its host, inside the project or above it, as the host's own project does.
#[test]
fn a_remote_project_lists_a_folder_of_its_host_even_outside_the_project() {
    let dir = folder(&[("src/a.rs", "a"), ("notes.txt", "n")]);
    let (_host, dial) = host_with_data(None);
    let remote = connect(&dir, dial);
    let local = atelier_project::LocalProject::open(dir.path()).unwrap();
    let parent = dir.path().parent().unwrap().to_str().unwrap();
    let at = dir.path().to_str().unwrap();
    assert_eq!(remote.read_dir(at).unwrap(), local.read_dir(at).unwrap(), "the same answer as on the host");
    let name = dir.path().file_name().unwrap().to_str().unwrap();
    assert!(remote.read_dir(parent).unwrap().iter().any(|e| e.dir && e.name == name), "the folder above the project lists it");
    let names: Vec<_> = remote.read_dir(dir.path().to_str().unwrap()).unwrap().into_iter().map(|e| (e.name, e.dir)).collect();
    assert_eq!(names, [("src".to_string(), true), ("notes.txt".to_string(), false)], "folders first");
    assert_eq!(remote.read_dir("relative/path").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert_eq!(remote.read_dir("/no/such/folder/anywhere").unwrap_err().kind(), io::ErrorKind::NotFound);
}

/// A host that answers the hello with `words`, as another build of the helper does.
fn helper_saying(words: &'static str) -> Dial {
    Box::new(move || {
        let (app_reader, mut host_writer) = io::pipe()?;
        let (mut host_reader, app_writer) = io::pipe()?;
        thread::spawn(move || {
            if let Ok(Some(Frame::Request { id, .. })) = read_frame(&mut host_reader) {
                let failure = Failure { kind: crate::protocol::FailureKind::InvalidInput, message: words.into() };
                let _ = write_frame(&mut host_writer, &Frame::Response { id, result: Err(failure) });
            }
        });
        Ok(Connection {
            reader: Box::new(app_reader),
            writer: Box::new(app_writer),
            close: Box::new(|| {}),
            last_words: Box::new(String::new),
        })
    })
}

#[test]
fn an_old_helper_says_update_atelier_and_no_protocol_numbers() {
    let dir = folder(&[("a.txt", "a")]);
    let dial = helper_saying("the app speaks version 5, this atelier-remote 4");
    let error = RemoteProject::connect("test", dir.path().display().to_string(), dial, Timeouts::default()).err().unwrap();
    let words = error.to_string();
    assert!(words.contains("update atelier on this host"), "{words}");
    assert!(!words.chars().any(|c| c.is_ascii_digit()), "{words}");
}

#[test]
fn a_helper_tells_an_older_app_to_update_atelier() {
    let dir = folder(&[("a.txt", "a")]);
    let (host_reader, mut app_writer) = io::pipe().unwrap();
    let (mut app_reader, host_writer) = io::pipe().unwrap();
    thread::spawn(move || crate::server::serve(host_reader, host_writer));
    let hello = Call::Hello { version: VERSION - 1, root: dir.path().display().to_string() };
    write_frame(&mut app_writer, &Frame::Request { id: 1, call: hello }).unwrap();
    let Some(Frame::Response { result: Err(failure), .. }) = read_frame(&mut app_reader).unwrap() else { panic!("a refusal") };
    assert!(failure.message.contains("update atelier"), "{}", failure.message);
    assert!(!failure.message.chars().any(|c| c.is_ascii_digit()), "{}", failure.message);
}

#[test]
fn a_remote_project_keeps_its_tasks_on_the_host() {
    use atelier_tracker::{Entry as Log, NewTask, Patch, PrLink, Query as Tasks, SessionLink, Status};
    let dir = folder(&[("a.txt", "a")]);
    let data = tempfile::tempdir().unwrap();
    let (_host, dial) = host_with_data(Some(data.path().to_path_buf()));
    let remote = connect(&dir, dial);
    let tracker = remote.tracker().unwrap();
    assert_eq!(tracker.name(), "local");
    let task = tracker.create(&NewTask::titled("Keep tasks on the host"), "alex").unwrap();
    let two = tracker.create_many(&[NewTask { labels: vec!["ssh".into()], ..NewTask::titled("Two") }], "alex").unwrap();
    let moved = tracker.update(&task.id, &Patch::status(Status::InProgress), "alex").unwrap();
    assert_eq!(moved.status, Status::InProgress);
    let session = SessionLink { session_id: "s1".into(), title: "Keep".into(), agent: "Claude".into() };
    tracker.record(&task.id, &Log::SessionStarted(session), "alex").unwrap();
    tracker.record(&task.id, &Log::PrOpened(PrLink { number: 7, repo: "o/r".into() }), "alex").unwrap();
    assert_eq!(tracker.tasks_of_session("s1").unwrap(), vec![task.id.clone()]);
    assert_eq!(tracker.tasks_of_pr(7).unwrap(), vec![task.id.clone()]);
    assert_eq!(tracker.labels().unwrap(), vec!["ssh".to_string()]);
    assert_eq!(tracker.list(&Tasks::default()).unwrap().len(), 2);
    assert!(tracker.activity(&task.id).unwrap().len() >= 3);
    assert_eq!(tracker.get(&two[0].id).unwrap().map(|t| t.title), Some("Two".to_string()));
    let missing = atelier_tracker::TaskId("no-such".into());
    assert_eq!(tracker.update(&missing, &Patch::status(Status::Done), "alex"), Err(atelier_tracker::TrackerError::NotFound(missing)));
    // The store is the host's own file, in the project's data folder there.
    let on_host = atelier_project::LocalProject::open(dir.path()).unwrap().with_data_dir(data.path());
    assert_eq!(on_host.tracker().unwrap().get(&task.id).unwrap().map(|t| t.status), Some(Status::InProgress));
}

#[test]
fn a_remote_tracker_hears_of_tasks_changed_elsewhere() {
    use atelier_tracker::{Event as Told, NewTask, Patch, Status};
    let dir = folder(&[("a.txt", "a")]);
    let data = tempfile::tempdir().unwrap();
    let (_host, dial) = host_with_data(Some(data.path().to_path_buf()));
    let remote = connect(&dir, dial).poll_tasks_every(Duration::from_millis(50));
    let events = remote.tracker().unwrap().subscribe();
    let on_host = atelier_project::LocalProject::open(dir.path()).unwrap().with_data_dir(data.path());
    let elsewhere = on_host.tracker().unwrap();
    let task = elsewhere.create(&NewTask::titled("Made on the host"), "someone").unwrap();
    match events.recv_timeout(Duration::from_secs(5)).unwrap() {
        Told::Created(t) => assert_eq!(t.id, task.id),
        other => panic!("{other:?}"),
    }
    std::thread::sleep(Duration::from_millis(1100));
    elsewhere.update(&task.id, &Patch::status(Status::Done), "someone").unwrap();
    match events.recv_timeout(Duration::from_secs(5)).unwrap() {
        Told::Updated(t) => assert_eq!(t.status, Status::Done),
        other => panic!("{other:?}"),
    }
}

/// The app's writes to the host, counted in bytes.
struct Counted(Arc<std::sync::atomic::AtomicUsize>, PipeWriter);

impl Write for Counted {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let n = self.1.write(bytes)?;
        self.0.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.1.flush()
    }
}

#[test]
fn a_dropped_subscription_stops_the_poll_before_its_next_list() {
    let dir = folder(&[("a.txt", "a")]);
    let data = tempfile::tempdir().unwrap();
    let sent = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = sent.clone();
    let dial: Dial = Box::new(move || {
        let (app_reader, host_writer) = io::pipe()?;
        let (host_reader, app_writer) = io::pipe()?;
        let data = Some(data.path().to_path_buf());
        thread::spawn(move || crate::server::serve_with_data(host_reader, host_writer, data));
        Ok(Connection {
            reader: Box::new(app_reader),
            writer: Box::new(Counted(counted.clone(), app_writer)),
            close: Box::new(|| {}),
            last_words: Box::new(String::new),
        })
    });
    let remote = connect(&dir, dial).poll_tasks_every(Duration::from_millis(20));
    let tracker = remote.tracker().unwrap();
    let subscription = tracker.subscribe();
    std::thread::sleep(Duration::from_millis(100));
    let polling = sent.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(100));
    assert!(sent.load(Ordering::Relaxed) > polling, "the poll lists while the subscription lives");
    drop(subscription);
    // One tick may be under way; after it, nothing more goes to the host.
    std::thread::sleep(Duration::from_millis(60));
    let stopped = sent.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(sent.load(Ordering::Relaxed), stopped, "no list after the subscription dropped");
}
