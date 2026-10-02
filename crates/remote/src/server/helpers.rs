use std::{
    io::{self, Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex, atomic::Ordering, mpsc},
    thread,
    time::Duration,
};

use atelier_project::{Control, LocalProject, Project};

use crate::protocol::{Call, Event, Failure, Frame, Pid, Reply, VERSION, read_frame, write_frame};
use super::structs::{Running, State};
use super::types::{Out, UPDATE_WORDS};
use super::tracker;

pub(super) fn send(out: &Out, frame: &Frame) {
    let mut out = out.lock().unwrap_or_else(|p| p.into_inner());
    // A write that fails means the app went away; the read loop ends on its own.
    let _ = write_frame(&mut *out, frame);
}

/// Serves requests from `input` until it closes, answering on `output`. Every process it started is
/// killed when the app goes away.
pub fn serve(input: impl Read, output: impl Write + Send + 'static) -> io::Result<()> {
    serve_with_data(input, output, None)
}

/// [`serve`], with the projects' data folders under `data_dir`, as a test wants.
pub fn serve_with_data(mut input: impl Read, output: impl Write + Send + 'static, data_dir: Option<PathBuf>) -> io::Result<()> {
    let out: Out = Arc::new(Mutex::new(Box::new(output)));
    let state = Arc::new(State { data_dir, ..State::default() });
    while let Some(frame) = read_frame(&mut input)? {
        let Frame::Request { id, call } = frame else { continue };
        match call {
            // In order, on this thread: bytes for one process must not overtake each other.
            Call::Input { pid, bytes } => {
                let result = with_running(&state, pid, |r| match &r.input {
                    Some(tx) => tx.send(bytes).map(|_| Reply::Done).map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed")),
                    None => Err(io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed")),
                });
                send(&out, &Frame::Response { id, result: result.map_err(|e| Failure::from(&e)) });
            }
            Call::CloseInput { pid } => {
                let result = with_running(&state, pid, |r| {
                    r.input = None;
                    Ok(Reply::Done)
                });
                send(&out, &Frame::Response { id, result: result.map_err(|e| Failure::from(&e)) });
            }
            call => {
                let (state, out) = (state.clone(), out.clone());
                thread::spawn(move || {
                    let result = answer(call, &state, &out).map_err(|e| Failure::from(&e));
                    send(&out, &Frame::Response { id, result });
                });
            }
        }
    }
    for (_, running) in state.running.lock().unwrap_or_else(|p| p.into_inner()).drain() {
        let _ = running.control.lock().unwrap_or_else(|p| p.into_inner()).kill();
    }
    Ok(())
}

pub(super) fn with_running(state: &State, pid: Pid, f: impl FnOnce(&mut Running) -> io::Result<Reply>) -> io::Result<Reply> {
    let mut running = state.running.lock().unwrap_or_else(|p| p.into_inner());
    let r = running.get_mut(&pid).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no process {pid}")))?;
    f(r)
}

pub(super) fn project(state: &State) -> io::Result<Arc<LocalProject>> {
    state.project.lock().unwrap_or_else(|p| p.into_inner()).clone().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no hello yet"))
}

/// `~` and `~/…` as the host's home folder.
pub(super) fn expand(root: &str) -> PathBuf {
    match (root.strip_prefix('~'), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => PathBuf::from(home).join(rest.trim_start_matches('/')),
        _ => PathBuf::from(root),
    }
}

pub(super) fn answer(call: Call, state: &Arc<State>, out: &Out) -> io::Result<Reply> {
    match call {
        Call::Hello { version, root } => {
            if version != VERSION {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, UPDATE_WORDS));
            }
            let project = LocalProject::open(expand(&root))?;
            let project = match &state.data_dir {
                Some(dir) => project.with_data_dir(dir),
                None => project,
            };
            let root = project.root().display().to_string();
            *state.project.lock().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(project));
            Ok(Reply::Hello { root })
        }
        Call::List => Ok(Reply::Entries(project(state)?.list()?)),
        Call::Read { path } => Ok(Reply::Bytes(project(state)?.read(&path)?)),
        Call::Write { path, bytes } => project(state)?.write(&path, &bytes).map(|()| Reply::Done),
        Call::Remove { path } => project(state)?.remove(&path).map(|()| Reply::Done),
        Call::DataRead { path } => Ok(Reply::Bytes(project(state)?.data_read(&path)?)),
        Call::DataWrite { path, bytes } => project(state)?.data_write(&path, &bytes).map(|()| Reply::Done),
        Call::DataList { prefix } => Ok(Reply::DataEntries(project(state)?.data_list(&prefix)?)),
        Call::ReadDir { dir } => Ok(Reply::DirEntries(project(state)?.read_dir(&dir)?)),
        Call::DataPath => match project(state)?.data_path() {
            Some(path) => Ok(Reply::Text(path.display().to_string())),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "this host has no data folder")),
        },
        Call::Watch => {
            let out = out.clone();
            let watch = project(state)?.watch(Box::new(move |changes| send(&out, &Frame::Event(Event::Changes(changes)))))?;
            *state.watch.lock().unwrap_or_else(|p| p.into_inner()) = Some(watch);
            Ok(Reply::Done)
        }
        Call::Tracker(call) => Ok(Reply::Tracker(Box::new(project(state)?.tracker().and_then(|t| tracker::answer(&*t, call))))),
        Call::Search { query } => Ok(Reply::Matches(project(state)?.search(&query)?)),
        Call::Git { args } => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            Ok(Reply::Git(project(state)?.git(&args)?))
        }
        Call::Spawn { command } => spawn(state, out, &command),
        Call::Kill { pid } => {
            let control = with_control(state, pid)?;
            control.lock().unwrap_or_else(|p| p.into_inner()).kill().map(|()| Reply::Done)
        }
        Call::Stderr { pid } => {
            let control = with_control(state, pid)?;
            let text = control.lock().unwrap_or_else(|p| p.into_inner()).stderr();
            Ok(Reply::Text(text))
        }
        Call::Input { .. } | Call::CloseInput { .. } => unreachable!("handled in order on the read loop"),
    }
}

pub(super) fn with_control(state: &State, pid: Pid) -> io::Result<Arc<Mutex<Box<dyn Control>>>> {
    let running = state.running.lock().unwrap_or_else(|p| p.into_inner());
    running.get(&pid).map(|r| r.control.clone()).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no process {pid}")))
}

pub(super) fn spawn(state: &Arc<State>, out: &Out, command: &atelier_project::Command) -> io::Result<Reply> {
    let process = project(state)?.spawn(command)?;
    let pid = state.next_pid.fetch_add(1, Ordering::Relaxed) + 1;
    let control = Arc::new(Mutex::new(process.control));
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let mut stdin = process.stdin;
    thread::spawn(move || {
        for bytes in rx {
            if stdin.write_all(&bytes).and_then(|()| stdin.flush()).is_err() {
                break;
            }
        }
        // The channel closed: CloseInput, or the process is gone. Dropping stdin closes it.
    });
    state.running.lock().unwrap_or_else(|p| p.into_inner()).insert(pid, Running { input: Some(tx), control: control.clone() });
    let (out, state) = (out.clone(), state.clone());
    let mut stdout = process.stdout;
    thread::spawn(move || {
        let mut chunk = vec![0u8; 64 * 1024];
        while let Ok(n) = stdout.read(&mut chunk) {
            if n == 0 {
                break;
            }
            send(&out, &Frame::Event(Event::Output { pid, bytes: chunk[..n].to_vec() }));
        }
        // Its end is read: the pipe goes now, not when this thread does, 30 seconds on.
        drop(stdout);
        // stdout closed: wait for the exit without holding the lock a kill needs.
        while control.lock().unwrap_or_else(|p| p.into_inner()).running() {
            thread::sleep(Duration::from_millis(10));
        }
        let code = control.lock().unwrap_or_else(|p| p.into_inner()).wait().ok().flatten();
        // Its stdin goes now, which ends the thread that fed it and closes the pipe: a burst of short
        // commands must not run the host out of open files while each is kept for a last Stderr ask.
        if let Some(running) = state.running.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&pid) {
            running.input = None;
        }
        send(&out, &Frame::Event(Event::Exited { pid, code }));
        // Kept a little while, so a last Stderr request still finds it.
        thread::sleep(Duration::from_secs(30));
        state.running.lock().unwrap_or_else(|p| p.into_inner()).remove(&pid);
    });
    Ok(Reply::Spawned { pid })
}
