//! Helpers for tests that start processes.
//!
//! Linux refuses to run a file some process still has open for writing ("Text file busy", ETXTBSY). A test
//! writes a stand-in script and runs it; if another test thread forks in between, the child holds a copy
//! of the script's write descriptor until it execs, and the run fails. `Command::spawn` returns only after
//! the child has exec'd, so one lock around every write of a script and every spawn closes the window.
use std::{
    io,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use lathe_project::{ChangeSink, Command, GitOutput, Match, Process, Project, Query, Watch};

static SPAWN: Mutex<()> = Mutex::new(());

/// The lock every script write and every spawn in this test binary takes.
pub fn spawn_lock() -> MutexGuard<'static, ()> {
    SPAWN.lock().unwrap_or_else(|p| p.into_inner())
}

/// Writes an executable script, under the lock.
pub fn write_script(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt;
    let _held = spawn_lock();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A project whose spawns take the lock. Everything else is the inner project's.
pub struct Locked(pub Arc<dyn Project>);

impl Project for Locked {
    fn root(&self) -> &Path {
        self.0.root()
    }

    fn list(&self) -> io::Result<Vec<lathe_project::Entry>> {
        self.0.list()
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.0.read(path)
    }

    fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.0.write(path, bytes)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.0.remove(path)
    }

    fn data_read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.0.data_read(path)
    }

    fn data_write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.0.data_write(path, bytes)
    }

    fn data_list(&self, prefix: &str) -> io::Result<Vec<lathe_project::DataEntry>> {
        self.0.data_list(prefix)
    }

    fn watch(&self, sink: ChangeSink) -> io::Result<Watch> {
        self.0.watch(sink)
    }

    fn search(&self, query: &Query) -> io::Result<Vec<Match>> {
        self.0.search(query)
    }

    fn spawn(&self, command: &Command) -> io::Result<Process> {
        let _held = spawn_lock();
        self.0.spawn(command)
    }

    fn git(&self, args: &[&str]) -> io::Result<GitOutput> {
        let _held = spawn_lock();
        self.0.git(args)
    }

    fn host(&self) -> Option<&str> {
        self.0.host()
    }
}
