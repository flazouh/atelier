//! A process a project starts: a language server, a git run, the agent. Its pipes are plain byte
//! streams, so a process on this machine and one on an SSH host (M1b) look the same.
//!
//! Its stderr is kept, not streamed: the last [`STDERR_KEEP`] bytes, read on a thread of its own so a
//! chatty process never blocks on a full pipe. When a process ends early, [`Control::stderr`] says
//! why in its own words, complete the moment [`Control::wait`] returns.

use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, ChildStderr},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

/// What to start.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// A path on the host, or a name its `PATH` finds.
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The host folder it runs in; the project's root when `None`.
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

impl Command {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into(), ..Self::default() }
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}

/// Stops and waits for a process.
pub trait Control: Send {
    fn kill(&mut self) -> io::Result<()>;
    /// Blocks until it ends; its exit code, or `None` when a signal ended it.
    fn wait(&mut self) -> io::Result<Option<i32>>;
    /// Whether it still runs, without blocking.
    fn running(&mut self) -> bool;
    /// The last of what it wrote to stderr, up to [`STDERR_KEEP`] bytes, as text.
    fn stderr(&self) -> String;
}

/// How much of a process's stderr is kept: enough for a crash's message and its backtrace.
pub const STDERR_KEEP: usize = 64 * 1024;

/// How long [`Tail::finish`] waits for the stream to end after its process did: a grandchild that
/// kept the stream open must not hold up a wait for long.
const FINISH_WAIT: Duration = Duration::from_millis(500);

/// The tail of a stream, filled by a thread of its own.
#[derive(Clone, Default)]
pub struct Tail {
    kept: Arc<Mutex<VecDeque<u8>>>,
    reader: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Tail {
    /// Reads `stream` to its end on a new thread, keeping its last [`STDERR_KEEP`] bytes.
    pub fn follow(stream: impl Read + Send + 'static) -> Self {
        let tail = Self::default();
        let keep = tail.clone();
        let reader = thread::Builder::new().name("atelier-stderr".into()).spawn(move || {
            let mut stream = stream;
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stream.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                keep.push(&chunk[..n]);
            }
        });
        *tail.reader.lock().unwrap_or_else(|p| p.into_inner()) = reader.ok();
        tail
    }

    /// Waits for the stream to end, so every byte its process wrote is kept; for at most
    /// [`FINISH_WAIT`].
    pub fn finish(&self) {
        let Some(reader) = self.reader.lock().unwrap_or_else(|p| p.into_inner()).take() else { return };
        let deadline = Instant::now() + FINISH_WAIT;
        while !reader.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        if reader.is_finished() {
            let _ = reader.join();
        }
    }

    pub fn push(&self, bytes: &[u8]) {
        let mut kept = self.kept.lock().unwrap_or_else(|p| p.into_inner());
        kept.extend(bytes);
        let over = kept.len().saturating_sub(STDERR_KEEP);
        kept.drain(..over);
    }

    pub fn text(&self) -> String {
        let kept = self.kept.lock().unwrap_or_else(|p| p.into_inner());
        let (a, b) = kept.as_slices();
        String::from_utf8_lossy(&[a, b].concat()).into_owned()
    }
}

/// How a watchdog ends a child whose app is gone. The child leads a process group of its own, so the
/// watchdog ends the child's whole group, the helpers the child started too. It asks each second whether
/// the app runs (a zombie, killed but not reaped, does not; a host without `ps` cannot tell a zombie, so
/// there the app runs while it exists) and whether the group still has a process; once the app is gone,
/// it stops the group if it has a process, and kills it when it still has one two seconds later. A group
/// id is not reused while the group has a process, so the kill reaches no other process. It ignores the
/// signals a closing terminal sends the app's whole group, so it outlives them to do its work.
const TETHER: &str = r#"(
trap '' HUP INT TERM
up() {
  kill -0 "$1" 2>/dev/null || return 1
  s=$(ps -o stat= -p "$1" 2>/dev/null) || return 0
  case $s in *Z*) return 1 ;; esac
}
while up "$1" && kill -0 "-$2" 2>/dev/null; do sleep 1; done
if ! up "$1" && kill -0 "-$2" 2>/dev/null; then
  kill -TERM "-$2" 2>/dev/null
  sleep 2
  kill -0 "-$2" 2>/dev/null && kill -KILL "-$2" 2>/dev/null
fi
) </dev/null >/dev/null 2>&1 &"#;

/// Ends the child `pid`, the leader of its own process group, and its group when this app ends, however it
/// ends. Closing a child's stdin is not enough: some agents keep running after it, and a crash or a kill
/// gives the app no time to stop them. The watchdog runs detached; the `sh` that starts it ends at once, and
/// a thread of its own waits for it, so a spawn does not wait and leaves no zombie.
#[cfg(unix)]
pub fn tether(pid: u32) -> io::Result<()> {
    tether_with("sh", pid)
}

#[cfg(unix)]
fn tether_with(shell: &str, pid: u32) -> io::Result<()> {
    let mut starter = std::process::Command::new(shell)
        .args(["-c", TETHER, "atelier-tether", &std::process::id().to_string(), &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    match thread::Builder::new().name("atelier-tether".into()).spawn(move || starter.wait()) {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
pub fn tether(_pid: u32) -> io::Result<()> {
    Ok(())
}

/// A child process on this machine, the leader of a process group of its own, with its stderr's tail.
pub struct LocalChild {
    pub child: Child,
    pub stderr: Tail,
    /// Whether the child has been waited for. Until then its pid, so its group's id, belongs to it.
    reaped: bool,
}

impl LocalChild {
    pub fn new(mut child: Child) -> Self {
        let stderr = child.stderr.take().map(|e: ChildStderr| Tail::follow(e)).unwrap_or_default();
        Self { child, stderr, reaped: false }
    }
}

/// A started process: write to its stdin, read its stdout; its stderr's tail is on its control.
pub struct Process {
    pub stdin: Box<dyn Write + Send>,
    pub stdout: Box<dyn Read + Send>,
    pub control: Box<dyn Control>,
}

impl Control for LocalChild {
    /// Kills the child's whole group, so the helpers it started end with it and no pipe it shared stays open.
    fn kill(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        if !self.reaped {
            // SAFETY: `kill` takes no pointers. The group's id is the unreaped child's pid, so it is no
            // other process's.
            unsafe { libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL) };
        }
        self.child.kill()
    }

    /// Waits for the process, then for its stderr's last bytes.
    fn wait(&mut self) -> io::Result<Option<i32>> {
        let code = self.child.wait().map(|status| status.code());
        self.reaped |= code.is_ok();
        self.stderr.finish();
        code
    }

    fn running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                self.reaped = true;
                false
            }
            Err(_) => false,
        }
    }

    fn stderr(&self) -> String {
        self.stderr.text()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        os::unix::process::CommandExt,
        process::{Command as Os, Stdio},
        thread,
        time::Duration,
    };

    use super::{Control, LocalChild, TETHER};

    /// A watchdog that cannot start is an error for the spawn to report, not a child left unguarded.
    #[test]
    fn a_watchdog_that_cannot_start_is_an_error() {
        assert!(super::tether_with("/nonexistent/atelier-sh", std::process::id()).is_err());
    }

    fn sleeper() -> std::process::Child {
        Os::new("sleep").arg("30").process_group(0).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap()
    }

    fn alive(pid: u32) -> bool {
        // SAFETY: signal 0 only checks that the process exists.
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    /// A kill that works is a kill that succeeded, with the child still unreaped and after it was reaped too.
    #[test]
    fn killing_a_child_succeeds_before_and_after_it_is_waited_for() {
        let mut child = LocalChild::new(sleeper());
        assert!(child.kill().is_ok());
        assert!(child.wait().is_ok());
        assert!(child.kill().is_ok());
    }

    /// The watchdog asks `ps` whether a zombie app is gone, but a host without `ps` is no host whose app is
    /// gone: the child lives while the app does.
    #[test]
    fn a_watchdog_on_a_host_without_ps_keeps_the_child_while_the_app_runs() {
        let bin = tempfile::tempdir().unwrap();
        let sleep = ["/bin/sleep", "/usr/bin/sleep"].into_iter().find(|p| std::path::Path::new(p).exists()).unwrap();
        std::os::unix::fs::symlink(sleep, bin.path().join("sleep")).unwrap();
        let mut child = sleeper();
        let watchdog = Os::new("/bin/sh")
            .args(["-c", TETHER, "atelier-tether", &std::process::id().to_string(), &child.id().to_string()])
            .env("PATH", bin.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(watchdog.success());
        thread::sleep(Duration::from_millis(3500));
        let running = child.try_wait().unwrap().is_none();
        let _ = unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
        let _ = child.wait();
        assert!(running, "the watchdog ended the child of an app that runs");
    }

    /// Once the child's group is gone too, nothing is sent to its id, which another group may hold by then.
    #[test]
    fn a_watchdog_signals_no_group_that_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log");
        let mut gone = Os::new("true").spawn().unwrap();
        let app = gone.id();
        let _ = gone.wait();
        assert!(!alive(app));
        let script = format!("kill() {{ echo \"$*\" >> '{}'; return 1; }}\n{TETHER}", log.display());
        let status = Os::new("/bin/sh").args(["-c", &script, "atelier-tether", &app.to_string(), "999999"]).status().unwrap();
        assert!(status.success());
        thread::sleep(Duration::from_millis(3500));
        let sent = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(sent.contains("-0 -999999"), "the watchdog asks whether the group exists: {sent}");
        assert!(!sent.contains("TERM"), "the watchdog stops a group that is gone: {sent}");
    }
}
