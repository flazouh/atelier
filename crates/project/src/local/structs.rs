use std::{
    fs,
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::Stdio,
    collections::HashSet,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use ignore::WalkBuilder;
use notify::{EventKind, RecursiveMode, Watcher};
use regex::RegexBuilder;
use atelier_tracker::{LocalTracker, Tracker, TrackerError, TrackerResult, prefix_for};

use crate::{
    Change,
    ChangeKind,
    ChangeSink,
    Command,
    DataEntry,
    Entry,
    GitOutput,
    Match,
    Process,
    Project,
    Query,
    TrackerSlot,
    Watch,
    data::DataFolder,
    host_path,
    process::{Control as _, LocalChild},
};
use super::types::{SEARCH_MAX_BYTES, WATCH_BATCH};
use super::helpers::{Ignores, follow, write_whole};
use super::types::EACH_FOLDER;

pub struct LocalProject {
    pub(super) root: PathBuf,
    pub(super) data: Option<DataFolder>,
    /// Shared with the project's worktrees, so each of them hands out the same store.
    pub(super) tracker: Arc<TrackerSlot>,
    /// The project's folder name, which names its tasks, the same from every worktree.
    pub(super) name: String,
}

impl LocalProject {
    /// A project at `root`, which must be a folder.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = fs::canonicalize(root.into())?;
        if !root.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotADirectory, format!("{} is not a folder", root.display())));
        }
        let data = DataFolder::for_root(&root, None);
        let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
        Ok(Self { root, data, tracker: Arc::default(), name })
    }

    /// The project seen from `folder`, a worktree of its repository: see [`Project::at`].
    pub fn worktree(&self, folder: &Path) -> io::Result<Self> {
        let root = fs::canonicalize(folder)?;
        let common = |dir: &Path| -> Option<(PathBuf, PathBuf)> {
            let ask = |what: &str| {
                let out = std::process::Command::new("git")
                    .arg("-C")
                    .arg(dir)
                    .args(["rev-parse", "--path-format=absolute", what])
                    .stdin(Stdio::null())
                    .output()
                    .ok()?;
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                (out.status.success() && !path.is_empty()).then(|| fs::canonicalize(path).ok()).flatten()
            };
            Some((ask("--git-common-dir")?, ask("--show-toplevel")?))
        };
        let refuse = || io::Error::new(io::ErrorKind::InvalidInput, format!("{} is not a worktree of {}", root.display(), self.root.display()));
        let (Some((ours, _)), Some((theirs, top))) = (common(&self.root), common(&root)) else { return Err(refuse()) };
        if ours != theirs || top != root {
            return Err(refuse());
        }
        Ok(Self { root, data: self.data.clone(), tracker: self.tracker.clone(), name: self.name.clone() })
    }

    /// The same project with its data folder under `dir`, as a test wants.
    pub fn with_data_dir(mut self, dir: &Path) -> Self {
        self.data = DataFolder::for_root(&self.root, Some(dir));
        self
    }

    pub(super) fn data(&self) -> io::Result<&DataFolder> {
        self.data.as_ref().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "this machine has no data folder"))
    }

    fn walker(&self) -> WalkBuilder {
        super::helpers::walker(&self.root)
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
            Ok(Arc::new(LocalTracker::open(&folder.join(crate::TRACKER_FILE), &prefix_for(&self.name))?))
        })
    }

    fn at(&self, folder: &Path) -> io::Result<Arc<dyn Project>> {
        Ok(Arc::new(self.worktree(folder)?))
    }

    fn watch(&self, sink: ChangeSink) -> io::Result<Watch> {
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(tx).map_err(io::Error::other)?;
        let mode = if EACH_FOLDER { RecursiveMode::NonRecursive } else { RecursiveMode::Recursive };
        watcher.watch(&self.root, mode).map_err(io::Error::other)?;
        let watcher = Arc::new(Mutex::new(watcher));
        let mut ignores = Ignores::new(self.root.clone());
        let (root, held) = (self.root.clone(), Arc::downgrade(&watcher));
        if EACH_FOLDER {
            // Tens of thousands of folders take seconds: the watch is ready once its root is, and the rest follow on one
            // thread, which leaves the other cores to the listing walking the same tree.
            let (root, held) = (root.clone(), held.clone());
            thread::Builder::new().name("atelier-watch-folders".into()).spawn(move || follow(&held, &root))?;
        }
        thread::Builder::new().name("atelier-watch".into()).spawn(move || {
            // Ends when the watcher drops with the Watch, which closes the channel.
            while let Ok(first) = rx.recv() {
                let (mut batch, mut told) = (Vec::new(), HashSet::new());
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
                        let dir = path.is_dir();
                        if parts.is_empty() || ignores.ignored(&path, dir) || parts.last().is_some_and(|n| n.ends_with(".atelier-save")) {
                            continue;
                        }
                        if parts.last() == Some(&".gitignore") {
                            ignores.forget(path.parent().unwrap_or(&root));
                        }
                        if kind == ChangeKind::Created && dir && EACH_FOLDER {
                            follow(&held, &path);
                        }
                        // A rename or a save that replaces the file reads as a change of it.
                        let kind = if kind != ChangeKind::Removed && !path.exists() { ChangeKind::Removed } else { kind };
                        let change = Change { path: parts.join("/"), kind };
                        if told.insert(change.clone()) {
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

    /// Starts `command` as the leader of a process group of its own, so killing it ends what it started, with
    /// a watchdog that ends that group when atelier ends. A child the watchdog cannot guard is not left running.
    fn spawn(&self, command: &Command) -> io::Result<Process> {
        let mut os = std::process::Command::new(&command.program);
        os.args(&command.args)
            .current_dir(command.cwd.as_deref().unwrap_or(&self.root))
            .envs(command.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut os, 0);
        let mut child = os.spawn()?;
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let mut control = LocalChild::new(child);
        if let Err(error) = crate::process::tether(control.child.id()) {
            let _ = control.kill();
            let _ = control.wait();
            return Err(error);
        }
        Ok(Process { stdin: Box::new(stdin), stdout: Box::new(stdout), control: Box::new(control) })
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
