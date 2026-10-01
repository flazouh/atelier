//! A project in a folder on this machine.

use std::{
    fs,
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use ignore::{WalkBuilder, gitignore::Gitignore};
use notify::{EventKind, RecursiveMode, Watcher};
use regex::RegexBuilder;

use atelier_tracker::{LocalTracker, Tracker, TrackerError, TrackerResult, prefix_for};

use crate::{
    Change, ChangeKind, ChangeSink, Command, DataEntry, Entry, GitOutput, Match, Process, Project, Query, Watch,
    TrackerSlot, data::DataFolder, host_path, process::LocalChild,
};

/// Writes `target` whole, through a temporary file beside it, so a reader never sees half of it. The
/// file keeps its permissions, so saving a script keeps it runnable.
pub(crate) fn write_whole(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let temporary = target.with_file_name(format!(".{name}.atelier-save"));
    fs::write(&temporary, bytes)?;
    if let Ok(meta) = fs::metadata(target) {
        fs::set_permissions(&temporary, meta.permissions())?;
    }
    fs::rename(&temporary, target).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

/// How long a watch gathers changes before it sends them, so a save that touches a file three times,
/// or a checkout that touches a thousand, arrives as one batch.
pub const WATCH_BATCH: Duration = Duration::from_millis(50);

/// A file longer than this is not searched: it is data, not source.
const SEARCH_MAX_BYTES: u64 = 4 << 20;

pub struct LocalProject {
    root: PathBuf,
    data: Option<DataFolder>,
    tracker: TrackerSlot,
}

impl LocalProject {
    /// A project at `root`, which must be a folder.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = fs::canonicalize(root.into())?;
        if !root.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotADirectory, format!("{} is not a folder", root.display())));
        }
        let data = DataFolder::for_root(&root, None);
        Ok(Self { root, data, tracker: TrackerSlot::default() })
    }

    /// The same project with its data folder under `dir`, as a test wants.
    pub fn with_data_dir(mut self, dir: &Path) -> Self {
        self.data = DataFolder::for_root(&self.root, Some(dir));
        self
    }

    fn data(&self) -> io::Result<&DataFolder> {
        self.data.as_ref().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "this machine has no data folder"))
    }

    fn walker(&self) -> WalkBuilder {
        let mut walk = WalkBuilder::new(&self.root);
        // Dotfiles show, as editors show them; `.git` itself never does. A .gitignore counts even
        // before the folder is a git repository.
        walk.hidden(false).require_git(false).filter_entry(|e| e.file_name() != ".git");
        walk
    }

    fn relative(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.root).ok()?;
        let parts: Vec<&str> = rel.iter().filter_map(|p| p.to_str()).collect();
        (!parts.is_empty()).then(|| parts.join("/"))
    }
}

impl Project for LocalProject {
    fn root(&self) -> &Path {
        &self.root
    }

    fn list(&self) -> io::Result<Vec<Entry>> {
        let (tx, rx) = mpsc::channel();
        self.walker().build_parallel().run(|| {
            let tx = tx.clone();
            Box::new(move |entry| {
                if let Ok(entry) = entry {
                    let dir = entry.file_type().is_some_and(|t| t.is_dir());
                    let _ = tx.send((entry.into_path(), dir));
                }
                ignore::WalkState::Continue
            })
        });
        drop(tx);
        let mut entries: Vec<Entry> =
            rx.into_iter().filter_map(|(path, dir)| Some(Entry { path: self.relative(&path)?, dir })).collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        fs::read(host_path(&self.root, path)?)
    }

    fn write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        write_whole(&host_path(&self.root, path)?, bytes)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        fs::remove_file(host_path(&self.root, path)?)
    }

    fn data_read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.data()?.read(path)
    }

    fn data_write(&self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.data()?.write(path, bytes)
    }

    fn data_list(&self, prefix: &str) -> io::Result<Vec<DataEntry>> {
        self.data()?.list(prefix)
    }
    fn read_dir(&self, dir: &str) -> io::Result<Vec<crate::DirEntry>> {
        crate::read_local_dir(dir)
    }

    fn data_path(&self) -> Option<PathBuf> {
        self.data.as_ref().map(|d| d.path().to_path_buf())
    }

    fn tracker(&self) -> TrackerResult<Arc<dyn Tracker>> {
        self.tracker.get_or_open(|| {
            let folder = self.data_path().ok_or_else(|| TrackerError::Unsupported("keep tasks with no data folder".into()))?;
            let name = self.root.file_name().and_then(|n| n.to_str()).unwrap_or("project");
            Ok(Arc::new(LocalTracker::open(&folder.join(crate::TRACKER_FILE), &prefix_for(name))?))
        })
    }

    fn watch(&self, sink: ChangeSink) -> io::Result<Watch> {
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(tx).map_err(io::Error::other)?;
        watcher.watch(&self.root, RecursiveMode::Recursive).map_err(io::Error::other)?;
        let (ignore, _) = Gitignore::new(self.root.join(".gitignore"));
        let root = self.root.clone();
        thread::Builder::new().name("atelier-watch".into()).spawn(move || {
            // Ends when the watcher drops with the Watch, which closes the channel.
            while let Ok(first) = rx.recv() {
                let mut batch = Vec::new();
                let mut take = |event: notify::Result<notify::Event>| {
                    let Ok(event) = event else { return };
                    let kind = match event.kind {
                        EventKind::Create(_) => ChangeKind::Created,
                        EventKind::Remove(_) => ChangeKind::Removed,
                        EventKind::Modify(_) => ChangeKind::Changed,
                        _ => return,
                    };
                    for path in event.paths {
                        let Ok(rel) = path.strip_prefix(&root) else { continue };
                        let parts: Vec<&str> = rel.iter().filter_map(|p| p.to_str()).collect();
                        let ignored = ignore.matched_path_or_any_parents(rel, path.is_dir()).is_ignore();
                        if parts.is_empty() || parts[0] == ".git" || ignored || parts.last().is_some_and(|n| n.ends_with(".atelier-save")) {
                            continue;
                        }
                        // A rename or a save that replaces the file reads as a change of it.
                        let kind = if kind != ChangeKind::Removed && !path.exists() { ChangeKind::Removed } else { kind };
                        let change = Change { path: parts.join("/"), kind };
                        if !batch.contains(&change) {
                            batch.push(change);
                        }
                    }
                };
                take(first);
                while let Ok(more) = rx.recv_timeout(WATCH_BATCH) {
                    take(more);
                }
                if !batch.is_empty() {
                    sink(batch);
                }
            }
        })?;
        Ok(Watch::new(watcher))
    }

    fn search(&self, query: &Query) -> io::Result<Vec<Match>> {
        let pattern = if query.regex { query.pattern.clone() } else { regex::escape(&query.pattern) };
        let matcher = RegexBuilder::new(&pattern)
            .case_insensitive(!query.case_sensitive)
            .build()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        let mut found = Vec::new();
        // In path order, so the same search gives the same answer, and a limit cuts it the same way.
        for entry in self.walker().sort_by_file_name(|a, b| a.cmp(b)).build().flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) || entry.metadata().is_ok_and(|m| m.len() > SEARCH_MAX_BYTES) {
                continue;
            }
            let Some(path) = self.relative(entry.path()) else { continue };
            let Ok(file) = fs::File::open(entry.path()) else { continue };
            let mut reader = BufReader::new(file);
            // A NUL near the start is a binary file.
            if reader.fill_buf().is_ok_and(|head| head.contains(&0)) {
                continue;
            }
            let mut bytes = Vec::new();
            if reader.read_to_end(&mut bytes).is_err() {
                continue;
            }
            for (line, text) in String::from_utf8_lossy(&bytes).lines().enumerate() {
                if matcher.is_match(text) {
                    found.push(Match { path: path.clone(), line, text: text.to_string() });
                    if found.len() >= query.limit {
                        return Ok(found);
                    }
                }
            }
        }
        Ok(found)
    }

    fn spawn(&self, command: &Command) -> io::Result<Process> {
        let mut child = std::process::Command::new(&command.program)
            .args(&command.args)
            .current_dir(command.cwd.as_deref().unwrap_or(&self.root))
            .envs(command.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        Ok(Process { stdin: Box::new(stdin), stdout: Box::new(stdout), control: Box::new(LocalChild::new(child)) })
    }

    fn git(&self, args: &[&str]) -> io::Result<GitOutput> {
        let output = std::process::Command::new("git").arg("-C").arg(&self.root).args(args).stdin(Stdio::null()).output()?;
        Ok(GitOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests;
