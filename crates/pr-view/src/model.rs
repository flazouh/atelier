//! What the pull request view knows and decides, without a window: the data as it has arrived, the files
//! git found, which are seen, where the reader is, and the rows each part of the rail shows. The view
//! reads it and does the I/O; a test can walk a whole review here.
use std::collections::{HashMap, HashSet};

use atelier_ui::{CheckRun, RemarkSummary, ReviewProgress, ThreadSummary, conversation::open_first, file_tree::FileTree, merge::MergeFacts, review::step};
use gpui_kit::SharedString;
use atelier_forge::{ForgeError, Pull, PullRef, PullState};

use crate::{
    base::{Base, BaseChoice},
    checks::{JobLog, check_runs},
    data::{Part, PartKind, PullData},
    git::{Commit, FileEntry},
    present,
    state::is_seen,
    sync::keep_place,
};

/// What the rail's conversation holds now: a page of it, and the numbers of all of it.
pub struct Page {
    pub threads: Vec<ThreadSummary>,
    pub remarks: Vec<RemarkSummary>,
    pub open: usize,
    pub resolved: usize,
    pub remark_total: usize,
    pub hidden_threads: usize,
    pub hidden_remarks: usize,
}

/// The file on screen: a changed file, or one brought in to read beside them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// Relative to the repository.
    pub path: String,
    pub brought_in: bool,
}

/// A mark to write down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenChange {
    pub path: String,
    pub version: String,
    pub seen: bool,
}

/// What taking a part in changed, for the view to act on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    /// The head is not the one git was prepared for: prepare again.
    pub head_moved: bool,
}

pub struct PrModel {
    pub reference: PullRef,
    /// The reader's login.
    pub me: String,
    pub read_only: bool,
    pub data: PullData,
    /// The parts that could not be read, and why.
    pub errors: Vec<(PartKind, ForgeError)>,
    pub commits: Vec<Commit>,
    /// The files git found for the base on screen. `None` until git has answered.
    pub entries: Option<Vec<FileEntry>>,
    pub base: Option<Base>,
    pub choice: BaseChoice,
    /// The head git was prepared for.
    pub prepared_head: Option<String>,
    /// Files seen: path to the version marked.
    pub marks: HashMap<String, String>,
    pub place: Option<Place>,
    /// Where Escape and Previous go back to, newest last, with the caret there.
    pub back: Vec<(Place, Option<(u32, u32)>)>,
    pub jobs: HashMap<u64, JobLog>,
    /// Why git could not be used, in words.
    pub git_error: Option<String>,
}

impl PrModel {
    pub fn new(reference: PullRef, me: impl Into<String>, data: PullData) -> Self {
        Self {
            reference,
            me: me.into(),
            read_only: false,
            data,
            errors: Vec::new(),
            commits: Vec::new(),
            entries: None,
            base: None,
            choice: BaseChoice::Whole,
            prepared_head: None,
            marks: HashMap::new(),
            place: None,
            back: Vec::new(),
            jobs: HashMap::new(),
            git_error: None,
        }
    }

    pub fn pull(&self) -> Option<&Pull> {
        self.data.pull.as_ref()
    }

    /// The pull request has been opened before, or its header has arrived: there is something to draw.
    pub fn ready(&self) -> bool {
        self.data.pull.is_some()
    }

    /// Takes a part in. A part that failed is remembered; one that arrived clears the failure of its kind.
    pub fn apply_part(&mut self, part: Part, now: u64) -> Effect {
        match &part {
            Part::Failed { part: kind, error } => {
                self.errors.retain(|(k, _)| k != kind);
                self.errors.push((*kind, error.clone()));
                return Effect::default();
            }
            Part::Pull(_) => self.errors.retain(|(k, _)| *k != PartKind::Pull),
            Part::Files(_) => self.errors.retain(|(k, _)| *k != PartKind::Files),
            Part::Threads(_) => self.errors.retain(|(k, _)| *k != PartKind::Threads),
            Part::Remarks(_) => self.errors.retain(|(k, _)| *k != PartKind::Remarks),
            Part::Checks(_) => self.errors.retain(|(k, _)| *k != PartKind::Checks),
            Part::ReviewPoint(_) => self.errors.retain(|(k, _)| *k != PartKind::ReviewPoint),
            Part::Held(_) => self.errors.retain(|(k, _)| *k != PartKind::Held),
        }
        let before = self.before_paths();
        self.data.apply(part, now);
        let head_moved = self.pull().is_some_and(|p| self.prepared_head.as_deref() != Some(p.head_sha.as_str()));
        // Until git answers, the forge's file list is what there is, and a change of it keeps the place.
        if self.entries.is_none() {
            self.keep_place(before);
        }
        Effect { head_moved }
    }

    pub fn error_of(&self, part: PartKind) -> Option<&ForgeError> {
        self.errors.iter().find(|(k, _)| *k == part).map(|(_, e)| e)
    }

    fn before_paths(&self) -> Vec<String> {
        self.files().iter().map(|f| f.path.to_string()).collect()
    }

    /// The files for the tree: git's when it has answered (it finds renames), else the forge's.
    pub fn files(&self) -> Vec<atelier_ui::ChangedFile> {
        match &self.entries {
            Some(entries) => present::changed_files(entries),
            None => present::forge_files(&self.data.files),
        }
    }

    /// The paths in the order the tree shows them, which is the order Next and Previous walk.
    pub fn order(&self) -> Vec<SharedString> {
        FileTree::new(&self.files()).file_order()
    }

    pub fn entry(&self, path: &str) -> Option<&FileEntry> {
        self.entries.as_ref()?.iter().find(|e| e.path == path)
    }

    /// git has answered for a base: the files, the commits, and the base itself. The place is kept where it
    /// can be.
    pub fn set_git(&mut self, entries: Vec<FileEntry>, commits: Vec<Commit>, base: Base, head: String) {
        let before = self.before_paths();
        self.entries = Some(entries);
        self.commits = commits;
        self.choice = base.choice.clone();
        self.base = Some(base);
        self.prepared_head = Some(head);
        self.git_error = None;
        self.keep_place(before);
    }

    /// The paths in git's order, with the place kept: the same file if it is still there, else the one that
    /// stands where it stood, else the first.
    fn keep_place(&mut self, before: Vec<String>) {
        let after: Vec<String> = self.order().iter().map(|p| p.to_string()).collect();
        // The tree's order is what the reader sees, so `before` is put in it too.
        let before = if before.len() == after.len() { after.clone() } else { before };
        if self.place.as_ref().is_some_and(|p| p.brought_in) {
            return;
        }
        let current = self.place.as_ref().map(|p| p.path.as_str());
        self.place = keep_place(current, &before, &after).or_else(|| after.first().cloned()).map(|path| Place { path, brought_in: false });
        if after.is_empty() {
            self.place = None;
        }
    }

    /// Opens a changed file from the tree or the keys: a fresh start, so the way back is cleared.
    pub fn open(&mut self, path: &str) -> bool {
        if self.place.as_ref().is_some_and(|p| p.path == path && !p.brought_in) {
            return false;
        }
        self.back.clear();
        self.place = Some(Place { path: path.to_string(), brought_in: false });
        true
    }

    /// Goes to another file from this one (a jump): the way back remembers this one and its caret.
    pub fn jump_to(&mut self, path: &str, caret: Option<(u32, u32)>) {
        let changed = self.entry(path).is_some() || self.data.files.iter().any(|f| f.path == path);
        if let Some(here) = self.place.clone() {
            self.back.push((here, caret));
        }
        self.place = Some(Place { path: path.to_string(), brought_in: !changed });
    }

    /// Back along the stack, with the caret that was there.
    pub fn go_back(&mut self) -> Option<Option<(u32, u32)>> {
        let (place, caret) = self.back.pop()?;
        self.place = Some(place);
        Some(caret)
    }

    /// Next (`by` 1) and Previous (`by` -1) over the changed files. From a file brought in, the walk goes on
    /// from the changed file it was reached from.
    pub fn step(&mut self, by: isize) -> bool {
        let order = self.order();
        let from = self
            .place
            .iter()
            .chain(self.back.iter().rev().map(|(p, _)| p))
            .find(|p| !p.brought_in)
            .map(|p| SharedString::from(p.path.clone()));
        match step(&order, from.as_ref(), by) {
            Some(path) => self.open(&path),
            None => false,
        }
    }

    /// Whether the changed file at `path` is seen: marked at the version it has now.
    pub fn is_seen(&self, path: &str) -> bool {
        self.entry(path).is_some_and(|e| is_seen(&self.marks, path, e.version()))
    }

    pub fn seen_set(&self) -> HashSet<SharedString> {
        self.entries.iter().flatten().filter(|e| is_seen(&self.marks, &e.path, e.version())).map(|e| SharedString::from(e.path.clone())).collect()
    }

    /// Marks the file on screen seen, or takes the mark back. `None` for a file brought in or one git has
    /// not listed yet.
    pub fn toggle_seen(&mut self) -> Option<SeenChange> {
        let place = self.place.as_ref().filter(|p| !p.brought_in)?;
        let entry = self.entry(&place.path)?;
        let (path, version) = (entry.path.clone(), entry.version().to_string());
        if version.is_empty() {
            return None;
        }
        let seen = !is_seen(&self.marks, &path, &version);
        if seen {
            self.marks.insert(path.clone(), version.clone());
        } else {
            self.marks.remove(&path);
        }
        Some(SeenChange { path, version, seen })
    }

    /// Puts every file back to unseen.
    pub fn put_back(&mut self) -> bool {
        let any = !self.marks.is_empty();
        self.marks.clear();
        any
    }

    pub fn progress(&self) -> ReviewProgress {
        let files = self.files();
        let (added, removed) = atelier_ui::changed_files::totals(&files);
        ReviewProgress { files: files.len(), reviewed: self.seen_set().len(), added, removed }
    }

    pub fn check_rows(&self) -> Vec<CheckRun> {
        check_runs(&self.data.checks, &self.jobs)
    }

    /// The conversation as the rail lists it: open threads first, then the remarks.
    pub fn conversation(&self, now: u64) -> (Vec<ThreadSummary>, Vec<RemarkSummary>) {
        let threads = self.data.threads.iter().map(|t| present::thread_summary(t, now)).collect();
        (open_first(threads), self.data.remarks.iter().map(|r| present::remark_summary(r, now)).collect())
    }

    /// A page of the conversation: the first `threads` threads in list order and the first `remarks`
    /// remarks, with the counts of all of it. A long conversation costs only the page.
    pub fn conversation_page(&self, now: u64, threads: usize, remarks: usize) -> Page {
        let ordered = self.threads_in_list_order();
        let resolved = ordered.iter().filter(|t| t.resolved).count();
        Page {
            threads: ordered.iter().take(threads).map(|t| present::thread_summary(t, now)).collect(),
            remarks: self.data.remarks.iter().take(remarks).map(|r| present::remark_summary(r, now)).collect(),
            open: ordered.len() - resolved,
            resolved,
            remark_total: self.data.remarks.len(),
            hidden_threads: ordered.len().saturating_sub(threads),
            hidden_remarks: self.data.remarks.len().saturating_sub(remarks),
        }
    }

    /// The threads in the order the conversation list shows them: open first, each group in its own order.
    /// The list reports a press by its place in this order.
    pub fn threads_in_list_order(&self) -> Vec<&atelier_forge::Thread> {
        let (resolved, open): (Vec<_>, Vec<_>) = self.data.threads.iter().partition(|t| t.resolved);
        open.into_iter().chain(resolved).collect()
    }

    /// What merging needs. `conflicting` are the files the reader found in a local merge, when known.
    pub fn merge_facts(&self, conflicting: &[String]) -> Option<MergeFacts> {
        let pull = self.pull()?;
        Some(atelier_forge::present::merge_facts(pull, Some(&self.data.checks), conflicting))
    }

    /// Comments written in an open review and not sent.
    pub fn unsent(&self) -> usize {
        self.data.held.len()
    }

    /// Whether the reader has a review open: some comment is held.
    pub fn in_review(&self) -> bool {
        !self.data.held.is_empty()
    }

    pub fn mine(&self) -> bool {
        self.pull().is_some_and(|p| p.author == self.me)
    }

    pub fn closed(&self) -> bool {
        self.pull().is_some_and(|p| matches!(p.state, PullState::Merged | PullState::Closed))
    }

    /// The failing checks whose jobs have not been read yet, at most `max`.
    pub fn jobs_to_read(&self, max: usize) -> Vec<atelier_forge::JobRef> {
        self.data.checks.iter().filter_map(crate::checks::wants_log).filter(|j| !self.jobs.contains_key(&j.id)).take(max).cloned().collect()
    }
}
