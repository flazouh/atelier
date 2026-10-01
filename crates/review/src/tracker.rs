//! The record of one turn: the text of each file the agent touches, taken before its edit lands, and at
//! the turn's end the files that changed and their hunks. Two things name a touched file: a tool call
//! that says which file it edits, and git, which finds what a shell command changed.
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::ErrorKind,
};

use atelier_agents::session::{Event, ToolId, ToolKind};
use atelier_project::Project;

use crate::{
    file_review::{Change, FileReview},
    git_state::{self, State},
    turn::TurnReview,
};

/// A file as it was when the agent first touched it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Baseline {
    Text(String),
    /// The file did not exist.
    Absent,
    /// Not text, with a hash of its bytes (0 when they are not known).
    Binary(u64),
}

/// A file as it is now.
enum Now {
    Text(String),
    Absent,
    Binary(u64),
}

/// Records one turn. `begin` at its start, `observe` for each event of the session while it runs, and
/// `finish` at its end. Every method that takes a project reads from it and may be slow: call them off
/// the UI thread.
pub struct TurnTracker {
    root: String,
    start: Option<State>,
    baselines: BTreeMap<String, Baseline>,
    kinds: HashMap<ToolId, ToolKind>,
}

impl TurnTracker {
    pub fn begin(project: &dyn Project) -> Self {
        Self {
            root: project.root().to_string_lossy().trim_end_matches('/').to_string(),
            start: git_state::snapshot(project),
            baselines: BTreeMap::new(),
            kinds: HashMap::new(),
        }
    }

    /// Takes the baseline of a file when an event says a tool is about to change it. The earliest word is
    /// enough: a call names its file as its input streams in, before it runs.
    pub fn observe(&mut self, project: &dyn Project, event: &Event) {
        match event {
            Event::ToolStarted(call) => {
                self.kinds.insert(call.id.clone(), call.kind);
                if let Some(file) = &call.file {
                    self.touch(project, call.kind, file);
                }
            }
            Event::ToolTarget { id, file } | Event::ToolInput { id, file: Some(file), .. } => {
                if let Some(kind) = self.kinds.get(id).copied() {
                    self.touch(project, kind, file);
                }
            }
            Event::Permission(request) => {
                if let Some(file) = &request.call.file {
                    self.touch(project, request.call.kind, file);
                }
            }
            _ => {}
        }
    }

    fn touch(&mut self, project: &dyn Project, kind: ToolKind, file: &str) {
        if !matches!(kind, ToolKind::Edit | ToolKind::Write) {
            return;
        }
        let Some(path) = self.relative(file) else { return };
        if self.baselines.contains_key(&path) {
            return;
        }
        let baseline = match read(project, &path) {
            Some(Now::Text(text)) => Baseline::Text(text),
            Some(Now::Absent) => Baseline::Absent,
            Some(Now::Binary(hash)) => Baseline::Binary(hash),
            None => return,
        };
        self.baselines.insert(path, baseline);
    }

    /// The path of a file the agent named, relative to the project; `None` outside it.
    fn relative(&self, file: &str) -> Option<String> {
        let rest = file.strip_prefix(self.root.as_str())?.strip_prefix('/')?;
        (!rest.is_empty() && !rest.split('/').any(|part| part == ".." || part == ".")).then(|| rest.to_string())
    }

    /// The turn's files, with their hunks.
    pub fn finish(self, project: &dyn Project) -> TurnReview {
        let end = git_state::snapshot(project);
        let mut paths: BTreeSet<String> = self.baselines.keys().cloned().collect();
        // A change the user had made and a command undid: the file is clean again, and the text the
        // user had is not known.
        let mut undone: BTreeSet<String> = BTreeSet::new();
        if let (Some(start), Some(end)) = (&self.start, &end) {
            paths.extend(end.entries.keys().filter(|path| changed_since(start, end, path)).cloned());
            undone.extend(start.entries.keys().filter(|path| !end.entries.contains_key(*path)).cloned());
        }
        // The last commit's text of every file no tool named, in one git process.
        let from_git: Vec<&str> = paths.iter().filter(|p| !self.baselines.contains_key(*p)).map(String::as_str).collect();
        let heads = git_state::head_files(project, &from_git);
        let files: Vec<FileReview> = paths
            .iter()
            .filter_map(|path| self.review_of(project, path, &heads))
            .chain(undone.iter().filter(|path| !self.baselines.contains_key(*path)).filter_map(|path| match read(project, path)? {
                Now::Text(text) => Some(FileReview::unknown(path, Some(text))),
                Now::Absent => Some(FileReview::unknown(path, None)),
                Now::Binary(_) => Some(FileReview::binary(path, Change::Modified)),
            }))
            .collect();
        TurnReview::new(pair_renames(files))
    }

    fn review_of(&self, project: &dyn Project, path: &str, heads: &Heads) -> Option<FileReview> {
        let now = read(project, path)?;
        let (baseline, exact) = match self.baselines.get(path) {
            Some(baseline) => (Some(baseline.clone()), true),
            None => self.baseline_from_git(path, heads),
        };
        let Some(baseline) = baseline else {
            return Some(FileReview::unknown(path, if let Now::Text(text) = now { Some(text) } else { None }));
        };
        match (baseline, now) {
            // Bytes that are not text cannot be compared line by line; equal bytes are no change.
            (Baseline::Binary(was), Now::Binary(is)) if was == is && was != 0 => None,
            (Baseline::Binary(_), now) => Some(FileReview::binary(path, if matches!(now, Now::Absent) { Change::Deleted } else { Change::Modified })),
            (baseline, Now::Binary(_)) => {
                Some(FileReview::binary(path, if matches!(baseline, Baseline::Absent) { Change::Added } else { Change::Modified }))
            }
            (baseline, now) => {
                let before = if let Baseline::Text(text) = baseline { Some(text) } else { None };
                let after = if let Now::Text(text) = now { Some(text) } else { None };
                (before != after).then(|| FileReview::from_texts(path, before, after, exact))
            }
        }
    }

    /// The text before the turn of a file no tool named, from git. Exact when the file was clean when
    /// the turn started, since then it held the last commit's text. `None` when nothing is known.
    fn baseline_from_git(&self, path: &str, heads: &Heads) -> (Option<Baseline>, bool) {
        let Some(start) = &self.start else { return (None, false) };
        let head = heads.get(path).and_then(|bytes| bytes.as_deref());
        let as_baseline = |bytes: &[u8]| match std::str::from_utf8(bytes) {
            Ok(text) if !text.contains('\0') => Baseline::Text(text.to_string()),
            _ => Baseline::Binary(0),
        };
        match start.entries.get(path) {
            None => (Some(head.map_or(Baseline::Absent, as_baseline)), true),
            Some(entry) if entry.untracked => (None, false),
            // Dirty when the turn started: the last commit's text stands in for the user's.
            Some(_) => match head.map(as_baseline) {
                Some(Baseline::Text(text)) => (Some(Baseline::Text(text)), false),
                _ => (None, false),
            },
        }
    }
}

/// The last commit's bytes of each file, by path.
type Heads = HashMap<String, Option<Vec<u8>>>;

/// Whether the working tree holds something different at `path` than when the turn started.
fn changed_since(start: &State, end: &State, path: &str) -> bool {
    match start.entries.get(path) {
        None => true,
        Some(was) => {
            let is = &end.entries[path];
            was.deleted != is.deleted || start.blobs.get(path) != end.blobs.get(path)
        }
    }
}

fn read(project: &dyn Project, path: &str) -> Option<Now> {
    match project.read(path) {
        Ok(bytes) if bytes.contains(&0) => Some(Now::Binary(hash_bytes(&bytes))),
        Ok(bytes) => Some(match String::from_utf8(bytes) {
            Ok(text) => Now::Text(text),
            Err(error) => Now::Binary(hash_bytes(error.as_bytes())),
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Some(Now::Absent),
        Err(_) => None,
    }
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish().max(1)
}

/// A file gone and a file new with the same text is one file moved.
fn pair_renames(files: Vec<FileReview>) -> Vec<FileReview> {
    let gone: HashMap<&str, &str> = files
        .iter()
        .filter(|f| f.change == Change::Deleted)
        .filter_map(|f| Some((f.before.as_deref()?, f.path.as_str())))
        .collect();
    let mut moved: HashMap<String, String> = HashMap::new();
    for file in files.iter().filter(|f| f.change == Change::Added) {
        if let Some(from) = file.after.as_deref().and_then(|text| gone.get(text)) {
            moved.insert(file.path.clone(), (*from).to_string());
        }
    }
    let sources: BTreeSet<String> = moved.values().cloned().collect();
    files
        .into_iter()
        .filter(|f| !sources.contains(&f.path))
        .map(|f| match moved.get(&f.path) {
            Some(from) => {
                let text = f.after.clone();
                FileReview::from_texts(f.path.clone(), text.clone(), text, true).renamed(from.clone())
            }
            None => f,
        })
        .collect()
}

#[cfg(test)]
mod tests;
