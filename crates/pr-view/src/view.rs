//! One pull request on the screen, on real data. The rail holds what is said and decided about the whole
//! pull request (checks, conversation, the verdict, the merge, the commits); the right pane holds the files
//! (the seen bar, the tree, and the diff with the threads in place). [`PrModel`] decides; this reads and
//! writes through the forge and git on background tasks and draws.
//!
//! Every blocking call is made in `background_spawn`. Answers come back as [`Msg`] on a channel and are
//! taken on the UI thread, one at a time, so a slow forge never stops a frame.
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use beui::{
    CommentComposer, CommentComposerEvent, Filter, Finder, FinderEvent, FinderItem, LineComment, LineComposer, LineComposerEvent, MergeBox, MergeBoxEvent,
    ReviewHandlers, VerdictBox, VerdictEvent,
    verdict::Verb,
};
use futures_channel::mpsc;
use futures_util::StreamExt;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, IntoElement, ParentElement, SharedString, Styled,
    Subscription, Task, Window,
    base::input::RowBlock,
    component::input::EditorState,
    div,
};
use lathe_editor::{EditorSession, Elsewhere, Jump};
use lathe_forge::{ForgeError, MergeOutcome, NewLine, PullRef, PullState, Side, ThreadId, Verdict};
use lathe_lsp::canonical;
use lsp_types::Position;

use crate::{
    base::{Base, BaseChoice, opening_choice, resolve},
    checks::{JobLog, MAX_LOGS},
    data::{Part, PartKind, PullData},
    diff::FileView,
    git::{Blob, Commit, FileEntry, Prepared, short},
    layout::{Fit, fit},
    load::load_all,
    model::{Effect, PrModel, SeenChange},
    place::place,
    services::{Services, now},
    sync::{Cadence, Refreshed, delta, refresh},
};

/// What the view tells its owner.
#[derive(Clone, Debug, PartialEq)]
pub enum PullEvent {
    /// The reader asked to open a file in the editor.
    OpenFile { pull: PullRef, path: String, line: Option<u32> },
    /// The pull request's state changed under the reader: merged, closed, opened again.
    StateChanged { pull: PullRef, state: PullState },
}

/// A file's diff, by where it came from: the base, the path, and the version at the head.
/// One block under a row: drawn again on each frame.
type Block = Rc<dyn Fn() -> AnyElement>;

pub(crate) type FileKey = (String, String, String);

#[allow(clippy::large_enum_variant)]
pub(crate) enum Msg {
    Cached { data: Option<PullData>, marks: HashMap<String, String> },
    Part(Part),
    /// Every part of the first read has arrived.
    Loaded,
    Git { epoch: u64, prepared: Prepared, entries: Vec<FileEntry>, commits: Vec<Commit>, base: Base },
    GitFailed { epoch: u64, error: String },
    Checkout { epoch: u64, dir: Result<String, String> },
    File { key: FileKey, view: Arc<FileView> },
    FileFailed { key: FileKey, error: String },
    Brought { path: String, blob: Result<Blob, String> },
    Files { epoch: u64, files: Vec<String> },
    Job { id: u64, job: Option<JobLog> },
    Refreshed(Result<Refreshed, ForgeError>),
    Wrote(Result<String, ForgeError>),
}

/// What the editor holds now, to know when to change it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ShownKey {
    path: String,
    brought_in: bool,
    file: Option<FileKey>,
    checkout: bool,
}

/// A comment being written, and where.
struct Draft {
    row: usize,
    /// The file it belongs to.
    path: String,
    kind: DraftKind,
    composer: Entity<LineComposer>,
    _events: Subscription,
}

#[derive(Clone)]
enum DraftKind {
    Line { line: u32, side: Side },
    Reply(ThreadId),
}

/// A finder over the pull request: the bases the reader can pick, or the files of the head.
struct Lookup {
    finder: Entity<Finder>,
    kind: LookupKind,
    _events: Subscription,
}

enum LookupKind {
    /// What each row means.
    Base(Vec<BaseChoice>),
    /// The paths of the rows, once the listing has arrived.
    Files(Vec<String>),
}

pub struct PullView {
    pub(crate) services: Arc<Services>,
    pub(crate) model: PrModel,
    tx: mpsc::UnboundedSender<Msg>,
    epoch: u64,
    prepared: Option<Prepared>,
    pub(crate) checkout: Option<String>,
    /// The diffs read so far, oldest first; a few files are kept.
    views: Vec<(FileKey, Arc<FileView>)>,
    loading: HashSet<FileKey>,
    failed: HashMap<FileKey, String>,
    brought: HashMap<String, Blob>,
    brought_loading: HashSet<String>,
    pub(crate) editor: Entity<EditorState>,
    shown: Option<ShownKey>,
    pub(crate) session: Option<Entity<EditorSession>>,
    _session: Option<Subscription>,
    pub(crate) composer: Entity<CommentComposer>,
    pub(crate) verdict: Entity<VerdictBox>,
    pub(crate) merge: Entity<MergeBox>,
    draft: Option<Draft>,
    picker: Option<Lookup>,
    /// A line to put the caret on once the file it belongs to is shown.
    pending_caret: Option<Position>,
    /// The words under the header: what changed, what is being sent, what went wrong.
    pub(crate) notice: Option<String>,
    /// The reader picked a base themselves, so the opening choice no longer applies.
    chose_base: bool,
    opening_checked: bool,
    /// A copy of the data before a refresh, to tell what changed.
    before_refresh: Option<PullData>,
    pub(crate) first_load_done: bool,
    /// The notice is the error of a failed read, so the next good read clears it.
    sync_failed: bool,
    cadence: Cadence,
    refreshing: bool,
    stopped: bool,
    pub(crate) focus: FocusHandle,
    pub(crate) details: bool,
    pub(crate) tree: Option<bool>,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) review_mode: bool,
    _subscriptions: Vec<Subscription>,
    _pump: Task<()>,
    _loop: Task<()>,
}

impl EventEmitter<PullEvent> for PullView {}

impl gpui_kit::Focusable for PullView {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

/// How many diffs stay in memory.
const KEPT_FILES: usize = 24;

impl PullView {
    pub fn new(reference: PullRef, services: Arc<Services>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = mpsc::unbounded::<Msg>();
        let pump = cx.spawn_in(window, async move |this, cx| {
            while let Some(msg) = rx.next().await {
                if this.update_in(cx, |view, window, cx| view.take(msg, window, cx)).is_err() {
                    return;
                }
            }
        });
        let me = services.config.me.clone();
        let mut model = PrModel::new(reference.clone(), me.clone(), PullData::new(reference.clone()));
        model.read_only = services.config.read_only;
        let editor = beui::CodeEditor::state("", "", window, cx);
        let composer = cx.new(|cx| CommentComposer::new("On this pull request", me, window, cx));
        let verdict = cx.new(|cx| VerdictBox::new("", false, window, cx));
        let facts = lathe_forge::present::merge_facts(&crate::fixture::sample::pull(&reference, ""), None, &[]);
        let choice = beui::merge::first_choice(&facts, None);
        let merge = cx.new(|cx| MergeBox::new(facts, choice, "", "", window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&composer, window, |view, _, event: &CommentComposerEvent, window, cx| {
                let CommentComposerEvent::Submit(text) = event;
                view.post_remark(text.to_string(), window, cx);
            }),
            cx.subscribe_in(&verdict, window, |view, _, event: &VerdictEvent, window, cx| {
                let VerdictEvent::Send { verb, note, .. } = event;
                view.send_verdict(*verb, note.to_string(), window, cx);
            }),
            cx.subscribe_in(&merge, window, |view, _, event: &MergeBoxEvent, window, cx| {
                if let MergeBoxEvent::Act { action, choice, title, message } = event {
                    view.merge_pull(*action, *choice, title.to_string(), message.to_string(), window, cx);
                }
            }),
        ];
        let mut view = Self {
            cadence: Cadence::new(services.config.refresh),
            services,
            model,
            tx,
            epoch: 0,
            prepared: None,
            checkout: None,
            views: Vec::new(),
            loading: HashSet::new(),
            failed: HashMap::new(),
            brought: HashMap::new(),
            brought_loading: HashSet::new(),
            editor,
            shown: None,
            session: None,
            _session: None,
            composer,
            verdict,
            merge,
            draft: None,
            picker: None,
            pending_caret: None,
            notice: None,
            chose_base: false,
            opening_checked: false,
            before_refresh: None,
            first_load_done: false,
            sync_failed: false,
            refreshing: false,
            stopped: false,
            focus: cx.focus_handle(),
            details: true,
            tree: None,
            width: f32::MAX,
            height: 0.,
            review_mode: false,
            _subscriptions: subscriptions,
            _pump: pump,
            _loop: Task::ready(()),
        };
        view.start(cx);
        view
    }

    pub fn reference(&self) -> &PullRef {
        &self.model.reference
    }

    /// The disk first, then the forge; after that, on the cadence.
    fn start(&mut self, cx: &mut Context<Self>) {
        let (services, tx, reference) = (self.services.clone(), self.tx.clone(), self.model.reference.clone());
        cx.background_spawn(async move {
            let marks = services.reviewed.marks(&reference).unwrap_or_default();
            let _ = tx.unbounded_send(Msg::Cached { data: services.snapshots.load(&reference), marks });
            let _ = services.reviewed.opened(&reference, now());
            let sender = std::sync::Mutex::new(tx.clone());
            load_all(services.forge.as_ref(), &reference, &|part| {
                let _ = sender.lock().unwrap_or_else(|e| e.into_inner()).unbounded_send(Msg::Part(part));
            });
            let _ = tx.unbounded_send(Msg::Loaded);
        })
        .detach();
        self._loop = cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |view, _| view.wait()) else { return };
                let Some(wait) = wait else {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    continue;
                };
                cx.background_executor().timer(wait).await;
                if this.update(cx, |view, cx| view.refresh(cx)).is_err() {
                    return;
                }
            }
        });
    }

    /// How long to wait before the next refresh; `None` when asking cannot help until the reader acts.
    fn wait(&mut self) -> Option<Duration> {
        if self.stopped {
            return None;
        }
        Some(self.cadence.next_wait())
    }

    /// Asks the forge whether anything changed, and reads what did. Nothing happens while a read is out.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing || !self.first_load_done {
            return;
        }
        self.refreshing = true;
        self.before_refresh = Some(self.model.data.clone());
        let (services, tx, known) = (self.services.clone(), self.tx.clone(), self.model.data.clone());
        cx.background_spawn(async move {
            let sender = std::sync::Mutex::new(tx.clone());
            let result = refresh(services.forge.as_ref(), &known.reference, &known, &|part| {
                let _ = sender.lock().unwrap_or_else(|e| e.into_inner()).unbounded_send(Msg::Part(part));
            });
            let _ = tx.unbounded_send(Msg::Refreshed(result));
        })
        .detach();
    }

    /// Reads the parts that change while people work, whatever the header says. After a write.
    fn read_now(&mut self, cx: &mut Context<Self>) {
        let (services, tx, reference) = (self.services.clone(), self.tx.clone(), self.model.reference.clone());
        self.before_refresh = Some(self.model.data.clone());
        cx.background_spawn(async move {
            let sender = std::sync::Mutex::new(tx.clone());
            crate::load::load_parts(services.forge.as_ref(), &reference, &crate::load::LIVE, &|part| {
                let _ = sender.lock().unwrap_or_else(|e| e.into_inner()).unbounded_send(Msg::Part(part));
            });
            let _ = tx.unbounded_send(Msg::Refreshed(Ok(Refreshed { changed: true, head_moved: false })));
        })
        .detach();
    }

    /// The reader asked again, after a failure that stopped the asking.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        self.stopped = false;
        self.sync_failed = false;
        self.notice = None;
        self.refresh(cx);
        cx.notify();
    }

    pub(crate) fn take(&mut self, msg: Msg, window: &mut Window, cx: &mut Context<Self>) {
        match msg {
            Msg::Cached { data, marks } => {
                self.model.marks = marks;
                // The disk draws only until the forge has answered.
                if let Some(data) = data.filter(|_| !self.model.ready()) {
                    self.model.data = data;
                    self.sync_boxes(window, cx);
                    self.prepare_git(cx);
                }
            }
            Msg::Part(part) => {
                let kind = match &part {
                    Part::Pull(_) => Some(PartKind::Pull),
                    Part::Checks(_) => Some(PartKind::Checks),
                    Part::ReviewPoint(_) => Some(PartKind::ReviewPoint),
                    _ => None,
                };
                let Effect { head_moved } = self.model.apply_part(part, now());
                match kind {
                    Some(PartKind::Pull) => {
                        self.sync_boxes(window, cx);
                        if head_moved {
                            self.prepare_git(cx);
                        }
                    }
                    Some(PartKind::Checks) => self.read_jobs(cx),
                    Some(PartKind::ReviewPoint) => self.maybe_open_since_review(cx),
                    _ => {}
                }
                self.show(window, cx);
            }
            Msg::Loaded => {
                self.first_load_done = true;
                self.cadence.after_answer(true);
                self.save_snapshot(cx);
            }
            Msg::Refreshed(result) => {
                self.refreshing = false;
                match result {
                    Ok(refreshed) => {
                        self.cadence.after_answer(refreshed.changed);
                        // A quiet answer leaves the last notice alone, unless that notice was a failed read.
                        if std::mem::take(&mut self.sync_failed) {
                            self.notice = None;
                        }
                        if refreshed.changed {
                            self.after_change(window, cx);
                        }
                    }
                    Err(error) => {
                        self.stopped = self.cadence.after_failure(&error).is_none();
                        self.sync_failed = true;
                        self.notice = Some(error.to_string());
                    }
                }
            }
            Msg::Git { epoch, prepared, entries, commits, base } => {
                if epoch != self.epoch {
                    return;
                }
                let head = prepared.head.clone();
                self.prepared = Some(prepared);
                self.model.set_git(entries, commits, base, head);
                self.model.git_error = None;
                self.prefetch(cx);
                self.show(window, cx);
                cx.notify();
            }
            Msg::GitFailed { epoch, error } => {
                if epoch == self.epoch {
                    self.model.git_error = Some(error);
                    cx.notify();
                }
            }
            Msg::Checkout { epoch, dir } => {
                if epoch != self.epoch {
                    return;
                }
                match dir {
                    Ok(dir) => {
                        self.checkout = Some(dir);
                        self.shown = None;
                        self.show(window, cx);
                    }
                    Err(error) => self.model.git_error = Some(error),
                }
                cx.notify();
            }
            Msg::File { key, view } => {
                self.loading.remove(&key);
                self.failed.remove(&key);
                self.views.retain(|(k, _)| *k != key);
                self.views.push((key, view));
                while self.views.len() > KEPT_FILES {
                    self.views.remove(0);
                }
                self.show(window, cx);
                cx.notify();
            }
            Msg::FileFailed { key, error } => {
                self.loading.remove(&key);
                self.failed.insert(key, error);
                cx.notify();
            }
            Msg::Brought { path, blob } => {
                self.brought_loading.remove(&path);
                self.brought.insert(path, blob.unwrap_or(Blob::Missing));
                self.show(window, cx);
                cx.notify();
            }
            Msg::Files { epoch, files } => {
                if epoch == self.epoch
                    && let Some(Lookup { finder, kind: LookupKind::Files(paths), .. }) = &mut self.picker
                {
                    let changed: HashSet<String> = self.model.files().iter().map(|f| f.path.to_string()).collect();
                    let items = files.iter().map(|f| FinderItem::new(f.clone(), if changed.contains(f) { "changed" } else { "" }).icon(f.clone())).collect();
                    *paths = files;
                    finder.update(cx, |f, cx| f.set_items(items, cx));
                }
            }
            Msg::Job { id, job } => {
                if let Some(job) = job {
                    self.model.jobs.insert(id, job);
                }
                cx.notify();
            }
            Msg::Wrote(result) => match result {
                Ok(words) => {
                    self.notice = Some(words);
                    self.read_now(cx);
                }
                Err(error) => {
                    self.notice = Some(error.to_string());
                    cx.notify();
                }
            },
        }
    }

    /// The data changed under the reader: tell them what, refresh the boxes, keep their place.
    fn after_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(before) = self.before_refresh.take() {
            let change = delta(&before, &self.model.data);
            if let Some(words) = change.words() {
                self.notice = Some(words);
            }
            if let Some(state) = change.state {
                cx.emit(PullEvent::StateChanged { pull: self.model.reference.clone(), state });
            }
        }
        self.sync_boxes(window, cx);
        self.save_snapshot(cx);
        cx.notify();
    }

    fn save_snapshot(&self, cx: &mut Context<Self>) {
        let (services, data) = (self.services.clone(), self.model.data.clone());
        cx.background_spawn(async move {
            let _ = services.snapshots.save(&data);
        })
        .detach();
    }

    /// The verdict and merge boxes follow the pull request's header.
    fn sync_boxes(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(pull) = self.model.pull().cloned() else { return };
        let stated = crate::present::stated_verdict(&pull, &self.model.me);
        let mine = self.model.mine();
        self.verdict.update(cx, |b, cx| {
            b.set_head(pull.head_sha.clone(), cx);
            b.set_mine(mine, cx);
            b.set_stated(stated, cx);
        });
        if let Some(facts) = self.model.merge_facts(&[]) {
            self.merge.update(cx, |b, cx| b.set_facts(facts, cx));
        }
    }

    // ---- git ----

    /// Prepares git for the header the reader has: the cache, the files, the commits and the checkout.
    fn prepare_git(&mut self, cx: &mut Context<Self>) {
        let Some(pull) = self.model.pull().cloned() else { return };
        self.epoch += 1;
        let epoch = self.epoch;
        let choice = self.model.choice.clone();
        let review_point = self.model.data.review_point.clone();
        let (services, tx) = (self.services.clone(), self.tx.clone());
        cx.background_spawn(async move {
            let git = &services.git;
            let answer = (|| {
                let prepared = git.prepare(&pull).map_err(|e| e.to_string())?;
                let base = resolve(git, &prepared, &choice, review_point.as_deref());
                let entries = git.files(&prepared, &base.sha).map_err(|e| e.to_string())?;
                let commits = git.commits(&prepared, &prepared.merge_base).map_err(|e| e.to_string())?;
                Ok::<_, String>((prepared, entries, commits, base))
            })();
            match answer {
                Ok((prepared, entries, commits, base)) => {
                    let _ = tx.unbounded_send(Msg::Git { epoch, prepared: prepared.clone(), entries, commits, base });
                    if services.config.workers.is_some() {
                        let dir = git.checkout(&prepared).map_err(|e| e.to_string());
                        let _ = tx.unbounded_send(Msg::Checkout { epoch, dir });
                    }
                }
                Err(error) => {
                    let _ = tx.unbounded_send(Msg::GitFailed { epoch, error });
                }
            }
        })
        .detach();
    }

    /// The first time the reader's last review point is known, open since it if there is anything new.
    fn maybe_open_since_review(&mut self, cx: &mut Context<Self>) {
        if self.opening_checked || self.chose_base {
            return;
        }
        self.opening_checked = true;
        let Some(head) = self.model.pull().map(|p| p.head_sha.clone()) else { return };
        let choice = opening_choice(self.model.data.review_point.as_deref(), &head);
        if choice != self.model.choice {
            self.model.choice = choice;
            self.prepare_git(cx);
        }
    }

    /// The reader picked a base: from the merge base, from their last review, or from a commit.
    pub fn choose_base(&mut self, choice: BaseChoice, cx: &mut Context<Self>) {
        self.chose_base = true;
        self.model.choice = choice;
        self.picker = None;
        self.prepare_git(cx);
        cx.notify();
    }

    /// Reads the logs of the failing checks, a few at a time, so their Fault shows.
    fn read_jobs(&mut self, cx: &mut Context<Self>) {
        let jobs = self.model.jobs_to_read(MAX_LOGS);
        for job in jobs {
            // A job is asked for once; an empty answer keeps it from being asked again.
            self.model.jobs.entry(job.id).or_insert_with(|| JobLog { job: placeholder_job(&job), log: String::new() });
            let (services, tx) = (self.services.clone(), self.tx.clone());
            cx.background_spawn(async move {
                let read = services.forge.job(&job).and_then(|j| services.forge.job_log(&job).map(|log| JobLog { job: j, log }));
                let _ = tx.unbounded_send(Msg::Job { id: job.id, job: read.ok() });
            })
            .detach();
        }
    }

    // ---- files ----

    fn key_of(&self, path: &str) -> Option<FileKey> {
        let base = self.model.base.as_ref()?;
        let entry = self.model.entry(path)?;
        Some((base.sha.clone(), path.to_string(), entry.version().to_string()))
    }

    pub(crate) fn view_of(&self, key: &FileKey) -> Option<Arc<FileView>> {
        self.views.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    /// The diff of the file on screen, when it has been read.
    pub(crate) fn current_view(&self) -> Option<Arc<FileView>> {
        let place = self.model.place.as_ref().filter(|p| !p.brought_in)?;
        self.view_of(&self.key_of(&place.path)?)
    }

    /// Reads a file's diff in the background, unless it is read or being read.
    fn ensure_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let (Some(prepared), Some(key)) = (self.prepared.clone(), self.key_of(path)) else { return };
        if self.view_of(&key).is_some() || self.loading.contains(&key) || self.failed.contains_key(&key) {
            return;
        }
        let Some(entry) = self.model.entry(path).cloned() else { return };
        self.loading.insert(key.clone());
        let (services, tx) = (self.services.clone(), self.tx.clone());
        cx.background_spawn(async move {
            let shas = [entry.old_blob.as_deref(), entry.new_blob.as_deref()];
            match services.git.blobs(&prepared, &shas) {
                Ok(blobs) => {
                    let view = FileView::build(&entry, &blobs[0], &blobs[1]);
                    let _ = tx.unbounded_send(Msg::File { key, view: Arc::new(view) });
                }
                Err(error) => {
                    let _ = tx.unbounded_send(Msg::FileFailed { key, error: error.to_string() });
                }
            }
        })
        .detach();
    }

    /// Reads the file the reader is likely to open next, so Next feels immediate.
    fn prefetch(&mut self, cx: &mut Context<Self>) {
        let order = self.model.order();
        let Some(current) = self.model.place.as_ref().map(|p| p.path.clone()) else { return };
        let at = order.iter().position(|p| p.as_ref() == current).unwrap_or(0);
        for offset in [0isize, 1, -1, 2] {
            if let Some(path) = usize::try_from(at as isize + offset).ok().and_then(|i| order.get(i)) {
                let path = path.to_string();
                self.ensure_file(&path, cx);
            }
        }
    }

    fn ensure_brought(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(prepared) = self.prepared.clone() else { return };
        if self.brought.contains_key(path) || self.brought_loading.contains(path) {
            return;
        }
        self.brought_loading.insert(path.to_string());
        let (services, tx, path) = (self.services.clone(), self.tx.clone(), path.to_string());
        cx.background_spawn(async move {
            let blob = services.git.head_file(&prepared, &path).map_err(|e| e.to_string());
            let _ = tx.unbounded_send(Msg::Brought { path, blob });
        })
        .detach();
    }

    /// Puts the file on screen in the editor, reading what it needs first.
    pub(crate) fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(place) = self.model.place.clone() else { return };
        let (file, text, rows_hunks) = if place.brought_in {
            self.ensure_brought(&place.path, cx);
            let Some(blob) = self.brought.get(&place.path) else { return };
            (None, blob.text_or_empty().trim_end_matches('\n').to_string(), None)
        } else {
            self.ensure_file(&place.path, cx);
            self.prefetch(cx);
            let Some(key) = self.key_of(&place.path) else { return };
            let Some(view) = self.view_of(&key) else { return };
            match view.shown() {
                // The last row's line end is not a row of its own in the editor.
                Some(shown) => (Some(key), shown.text().strip_suffix('\n').unwrap_or(shown.text()).to_string(), Some((shown.rows.clone(), shown.hunks().to_vec()))),
                None => (Some(key), String::new(), None),
            }
        };
        let wanted = ShownKey { path: place.path.clone(), brought_in: place.brought_in, file, checkout: self.checkout.is_some() };
        if self.shown.as_ref() == Some(&wanted) {
            return;
        }
        self.shown = Some(wanted);
        self.draft = None;
        let caret = self.pending_caret.take();
        self.editor.update(cx, |state, cx| {
            // The last file's server must not answer for this one while the next one starts.
            let lsp = state.lsp_mut();
            lsp.definition_provider = None;
            lsp.hover_provider = None;
            lsp.show_document = None;
            state.clear_hover_state(cx);
            state.set_value(text, window, cx);
            if let Some(position) = caret {
                state.set_cursor_position(position, window, cx);
            }
        });
        beui::code_editor::set_diagnostics(&self.editor, Vec::new(), cx);
        self.start_session(&place.path, rows_hunks.map(|(rows, _)| rows).unwrap_or_default(), cx);
        cx.notify();
    }

    /// A language server on the file in the head's checkout, when there is a checkout and servers.
    fn start_session(&mut self, path: &str, rows: beui::RowMap, cx: &mut Context<Self>) {
        self.session = None;
        self._session = None;
        let (Some(workers), Some(dir)) = (self.services.config.workers.clone(), self.checkout.clone()) else { return };
        let on_disk = PathBuf::from(&dir).join(path);
        if !on_disk.is_file() {
            return;
        }
        let this = cx.entity().downgrade();
        let elsewhere: Elsewhere = Rc::new(move |jump: Jump, window: &mut Window, cx: &mut gpui_kit::App| {
            let this = this.clone();
            // gpui-base asks while it updates the editor, and a jump replaces the editor's text.
            window.defer(cx, move |window, cx| {
                this.update(cx, |view, cx| view.jump(jump, window, cx)).ok();
            });
        });
        let editor = self.editor.clone();
        let session = cx.new(|cx| EditorSession::for_review(workers, editor, on_disk, rows, Some(elsewhere), cx));
        self._session = Some(cx.observe(&session, |_, _, cx| cx.notify()));
        self.session = Some(session);
    }

    /// Follows a jump out of the file on screen: to another changed file, or Brought In.
    fn jump(&mut self, jump: Jump, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dir) = self.checkout.clone() else { return };
        let root = canonical(&PathBuf::from(&dir));
        let Ok(relative) = canonical(&jump.path).strip_prefix(&root).map(|p| p.to_string_lossy().into_owned()) else {
            // The toolchain's own sources, say: named, not opened.
            let name = jump.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let line = jump.position.line + 1;
            if let Some(session) = &self.session {
                session.update(cx, |s, cx| s.say(format!("defined in {name} on line {line}, outside this repository"), cx));
            }
            return;
        };
        let caret = self.editor.read(cx).cursor_position();
        self.model.jump_to(&relative, Some((caret.line, caret.character)));
        // A position in the file's own rows becomes one in the shown rows once the diff is read.
        self.pending_caret = Some(jump.position);
        if let Some(view) = self.key_of(&relative).and_then(|k| self.view_of(&k)).and_then(|v| v.shown().map(|s| s.rows.clone())) {
            self.pending_caret = Some(Position { line: view.to_view(jump.position.line as usize) as u32, ..jump.position });
        }
        self.show(window, cx);
    }

    pub(crate) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(caret) = self.model.go_back() {
            self.pending_caret = caret.map(|(line, character)| Position { line, character });
            self.show(window, cx);
        }
    }

    /// Opens a changed file from the tree or the keys.
    pub fn open_file(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.open(path) {
            self.show(window, cx);
        }
        cx.notify();
    }

    /// Opens any file of the head: the changed one in its diff, another Brought In beside them.
    pub(crate) fn bring_in(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.model.jump_to(path, None);
        self.show(window, cx);
        cx.notify();
    }

    pub(crate) fn step(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.step(by) {
            self.show(window, cx);
        }
        cx.notify();
    }

    /// `x`: the file on screen is seen, or not. Written down in the background.
    pub(crate) fn toggle_seen(&mut self, cx: &mut Context<Self>) {
        let Some(SeenChange { path, version, seen }) = self.model.toggle_seen() else { return };
        let (services, reference) = (self.services.clone(), self.model.reference.clone());
        cx.background_spawn(async move {
            let _ = if seen { services.reviewed.mark(&reference, &path, &version, now()) } else { services.reviewed.unmark(&reference, &path) };
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn put_back(&mut self, cx: &mut Context<Self>) {
        if self.model.put_back() {
            let (services, reference) = (self.services.clone(), self.model.reference.clone());
            cx.background_spawn(async move {
                let _ = services.reviewed.clear(&reference);
            })
            .detach();
        }
        cx.notify();
    }

    // ---- the base picker ----

    /// Opens the list of bases: the whole pull request, since the last review, and each commit.
    pub fn pick_base(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut choices = vec![BaseChoice::Whole];
        let mut items = vec![FinderItem::new("The whole pull request", "from where the branch starts")];
        if let Some(point) = self.model.data.review_point.as_deref().filter(|p| self.model.commits.iter().any(|c| c.sha == *p) || crate::git::is_sha(p)) {
            choices.push(BaseChoice::LastReview);
            items.push(FinderItem::new("Since your last review", short(point).to_string()));
        }
        for commit in &self.model.commits {
            choices.push(BaseChoice::Commit(commit.sha.clone()));
            items.push(FinderItem::new(commit.title.clone(), format!("since {} by {}", short(&commit.sha), commit.author)));
        }
        let finder = cx.new(|cx| {
            let mut finder = Finder::new("Show changes", "Find a commit", Filter::Here, window, cx);
            finder.set_items(items, cx);
            finder
        });
        self.open_lookup(finder, LookupKind::Base(choices), window, cx);
    }

    /// Go to file: any file of the head. Opens one the pull request changed, or Brought In one it did not.
    pub fn go_to_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prepared) = self.prepared.clone() else { return };
        let finder = cx.new(|cx| {
            let mut finder = Finder::new("Go to file", "Part of a path", Filter::Here, window, cx).command(beui::keys::Command::GoToFile);
            finder.set_note("Loading files", cx);
            finder
        });
        self.open_lookup(finder, LookupKind::Files(Vec::new()), window, cx);
        let (services, tx, epoch) = (self.services.clone(), self.tx.clone(), self.epoch);
        cx.background_spawn(async move {
            let files = services.git.head_files(&prepared).unwrap_or_default();
            let _ = tx.unbounded_send(Msg::Files { epoch, files });
        })
        .detach();
    }

    fn open_lookup(&mut self, finder: Entity<Finder>, kind: LookupKind, window: &mut Window, cx: &mut Context<Self>) {
        let events = cx.subscribe_in(&finder, window, |view, _, event: &FinderEvent, window, cx| match event {
            FinderEvent::Pick(at) => {
                let picked = view.picker.take();
                view.focus.focus(window, cx);
                match picked.map(|p| p.kind) {
                    Some(LookupKind::Base(choices)) => {
                        if let Some(choice) = choices.get(*at).cloned() {
                            view.choose_base(choice, cx);
                        }
                    }
                    Some(LookupKind::Files(paths)) => {
                        if let Some(path) = paths.get(*at).cloned() {
                            view.bring_in(&path, window, cx);
                        }
                    }
                    None => {}
                }
                cx.notify();
            }
            FinderEvent::Dismiss => {
                view.picker = None;
                view.focus.focus(window, cx);
                cx.notify();
            }
            FinderEvent::Query(_) => {}
        });
        gpui_kit::Focusable::focus_handle(&finder, cx).focus(window, cx);
        self.picker = Some(Lookup { finder, kind, _events: events });
        cx.notify();
    }

    pub(crate) fn picker_element(&self) -> Option<AnyElement> {
        self.picker.as_ref().map(|p| p.finder.clone().into_any_element())
    }

    // ---- writes ----

    /// Sends a change to the forge in the background. `words` is what the notice says while it is on its
    /// way. A read-only view sends nothing and says so.
    fn write(&mut self, words: &str, cx: &mut Context<Self>, send: impl FnOnce(&dyn lathe_forge::Forge, &PullRef) -> Result<String, ForgeError> + Send + 'static) -> Option<Task<Result<String, ForgeError>>> {
        if self.model.read_only {
            self.notice = Some("Read-only: nothing was sent.".into());
            cx.notify();
            return None;
        }
        self.notice = Some(words.to_string());
        cx.notify();
        let (services, reference) = (self.services.clone(), self.model.reference.clone());
        Some(cx.background_spawn(async move { send(services.forge.as_ref(), &reference) }))
    }

    /// Runs a write to its end and takes the answer in as a message.
    pub(crate) fn write_and_report(&mut self, words: &str, cx: &mut Context<Self>, send: impl FnOnce(&dyn lathe_forge::Forge, &PullRef) -> Result<String, ForgeError> + Send + 'static) {
        let Some(task) = self.write(words, cx, send) else { return };
        let tx = self.tx.clone();
        cx.spawn(async move |_, _| {
            let _ = tx.unbounded_send(Msg::Wrote(task.await));
        })
        .detach();
    }

    pub(crate) fn post_remark(&mut self, text: String, _window: &mut Window, cx: &mut Context<Self>) {
        self.write_and_report("Posting…", cx, move |forge, reference| forge.comment(reference, &text).map(|_| "Commented.".to_string()));
    }

    pub(crate) fn send_verdict(&mut self, verb: Verb, note: String, window: &mut Window, cx: &mut Context<Self>) {
        let verdict = match verb {
            Verb::Approve => Verdict::Approve,
            Verb::RequestChanges => Verdict::RequestChanges,
            Verb::Comment => Verdict::Comment,
        };
        let Some(task) = self.write("Sending…", cx, move |forge, reference| forge.submit_review(reference, verdict, &note).map(|()| "Sent.".to_string())) else {
            self.verdict.update(cx, |b, cx| b.refused("Read-only: nothing was sent.", cx));
            return;
        };
        let tx = self.tx.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |view, window, cx| match &result {
                Ok(_) => view.verdict.update(cx, |b, cx| b.sent(window, cx)),
                Err(error) => view.verdict.update(cx, |b, cx| b.refused(error.to_string(), cx)),
            })
            .ok();
            let _ = tx.unbounded_send(Msg::Wrote(result));
        })
        .detach();
    }

    /// Sends the comments held in the reader's review, with no verdict (a plain comment review).
    pub(crate) fn send_unsent(&mut self, cx: &mut Context<Self>) {
        self.write_and_report("Sending your comments…", cx, |forge, reference| forge.submit_review(reference, Verdict::Comment, "").map(|()| "Sent.".to_string()));
    }

    pub(crate) fn merge_pull(&mut self, action: beui::merge::Action, choice: beui::merge::Choice, title: String, message: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(head) = self.model.pull().map(|p| p.head_sha.clone()) else { return };
        let request = match crate::actions::merge_request(action, choice, &title, &message, &head) {
            Ok(request) => request,
            Err(words) => {
                self.notice = Some(words);
                cx.notify();
                return;
            }
        };
        let delete = request.delete_branch;
        let Some(task) = self.write("Merging…", cx, move |forge, reference| {
            forge.merge(reference, &request).map(|outcome| match outcome {
                MergeOutcome::Merged => "Merged.".to_string(),
                MergeOutcome::WillMergeWhenReady => "It will merge when it is ready.".to_string(),
                MergeOutcome::Queued => "Added to the merge queue.".to_string(),
            })
        }) else {
            return;
        };
        let tx = self.tx.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            if let Ok(words) = &result {
                let merged = words == "Merged.";
                this.update(cx, |view, cx| {
                    if merged {
                        view.merge.update(cx, |b, cx| b.merged(delete, cx));
                    }
                })
                .ok();
            }
            let _ = tx.unbounded_send(Msg::Wrote(result));
        })
        .detach();
    }

    pub(crate) fn reply(&mut self, thread: ThreadId, text: String, cx: &mut Context<Self>) {
        self.write_and_report("Replying…", cx, move |forge, _| forge.reply(&thread, &text).map(|_| "Replied.".to_string()));
    }

    pub(crate) fn resolve(&mut self, thread: ThreadId, resolved: bool, cx: &mut Context<Self>) {
        self.write_and_report(if resolved { "Resolving…" } else { "Opening again…" }, cx, move |forge, _| forge.resolve(&thread, resolved).map(|()| if resolved { "Resolved." } else { "Open again." }.to_string()));
    }

    /// A new comment on a line: held when a review is open or the reader chose to start one, else sent.
    pub(crate) fn line_comment(&mut self, line: NewLine, hold: bool, cx: &mut Context<Self>) {
        let in_review = self.model.in_review();
        self.write_and_report(if hold || in_review { "Adding to your review…" } else { "Commenting…" }, cx, move |forge, reference| {
            forge.hold_comment(reference, &line)?;
            if hold {
                Ok("Added to your review. Nothing is sent until you send the review.".to_string())
            } else {
                forge.submit_review(reference, Verdict::Comment, "").map(|()| "Commented.".to_string())
            }
        });
    }

    // ---- the comment being written ----

    /// Opens a composer for a new comment on the shown row.
    pub(crate) fn open_composer(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.current_view() else { return };
        let Some(shown) = view.shown() else { return };
        let (line, side) = match (shown.lines.head_line(row), shown.lines.base_line(row)) {
            (Some(line), _) => (line, Side::Right),
            (None, Some(line)) => (line, Side::Left),
            _ => return,
        };
        self.start_draft(row, view.path.clone(), DraftKind::Line { line, side }, window, cx);
    }

    /// Opens the reply composer under a thread, in the diff where it hangs.
    pub(crate) fn open_reply(&mut self, thread: &ThreadId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.model.data.threads.iter().find(|t| t.id == *thread).cloned() else { return };
        // Go to the file first; the row is where the thread hangs.
        if self.model.place.as_ref().is_none_or(|p| p.path != t.path || p.brought_in) && self.model.entry(&t.path).is_some() {
            self.open_file(&t.path, window, cx);
        }
        let row = self.current_view().and_then(|v| {
            let placement = place(&self.model.data.threads, &v);
            let index = self.model.data.threads.iter().position(|x| x.id == *thread)?;
            placement.rows.iter().find(|(_, i)| *i == index).map(|(row, _)| *row)
        });
        self.start_draft(row.unwrap_or(0), t.path, DraftKind::Reply(thread.clone()), window, cx);
    }

    fn start_draft(&mut self, row: usize, path: String, kind: DraftKind, window: &mut Window, cx: &mut Context<Self>) {
        let in_review = self.model.in_review();
        let is_line = matches!(kind, DraftKind::Line { .. });
        let composer = cx.new(|cx| {
            let composer = LineComposer::new(row, window, cx);
            if is_line { composer.review_pass(in_review) } else { composer }
        });
        let events = cx.subscribe_in(&composer, window, |view, _, event: &LineComposerEvent, _, cx| {
            let Some(draft) = view.draft.take() else { return };
            match event {
                LineComposerEvent::Cancel { .. } => {}
                LineComposerEvent::Submit { text, .. } | LineComposerEvent::Hold { text, .. } => {
                    let hold = matches!(event, LineComposerEvent::Hold { .. });
                    match draft.kind {
                        DraftKind::Line { line, side } => {
                            view.line_comment(NewLine { path: draft.path.clone(), line, start_line: None, side, body: text.to_string() }, hold, cx);
                        }
                        DraftKind::Reply(thread) => view.reply(thread, text.to_string(), cx),
                    }
                }
            }
            cx.notify();
        });
        gpui_kit::Focusable::focus_handle(&composer, cx).focus(window, cx);
        self.draft = Some(Draft { row, path, kind, composer, _events: events });
        cx.notify();
    }

    /// The blocks that sit under rows of the diff: the threads and the composer being written.
    pub(crate) fn row_blocks(&self, view: &FileView, cx: &mut Context<Self>) -> Vec<RowBlock> {
        let placement = place(&self.model.data.threads, view);
        let mut by_row: Vec<(usize, Vec<Block>)> = Vec::new();
        let now = now();
        let this = cx.entity().downgrade();
        for (row, index) in &placement.rows {
            let thread = self.model.data.threads[*index].clone();
            let (weak_reply, weak_resolve) = (this.clone(), this.clone());
            let (id_reply, id_resolve) = (thread.id.clone(), thread.id.clone());
            let comments: Vec<beui::Comment> = thread.comments.iter().map(|c| crate::present::comment(c, now)).collect();
            let resolved = thread.resolved;
            let element_id = SharedString::from(format!("pr-thread-{}", thread.id.0));
            let build: Rc<dyn Fn() -> AnyElement> = Rc::new(move || {
                let (weak_reply, weak_resolve, id_reply, id_resolve) = (weak_reply.clone(), weak_resolve.clone(), id_reply.clone(), id_resolve.clone());
                LineComment::new(element_id.clone(), comments.clone())
                    .resolved(resolved)
                    .on_reply(move |_, window, cx| {
                        weak_reply.update(cx, |view, cx| view.open_reply(&id_reply, window, cx)).ok();
                    })
                    .on_resolve(move |_, _, cx| {
                        weak_resolve.update(cx, |view, cx| view.resolve(id_resolve.clone(), true, cx)).ok();
                    })
                    .into_any_element()
            });
            match by_row.iter_mut().find(|(r, _)| r == row) {
                Some((_, list)) => list.push(build),
                None => by_row.push((*row, vec![build])),
            }
        }
        if let Some(draft) = &self.draft
            && self.model.place.as_ref().is_some_and(|p| p.path == draft.path || view.old_path.as_deref() == Some(draft.path.as_str()))
        {
            let composer = draft.composer.clone();
            let build: Rc<dyn Fn() -> AnyElement> = Rc::new(move || composer.clone().into_any_element());
            match by_row.iter_mut().find(|(r, _)| *r == draft.row) {
                Some((_, list)) => list.push(build),
                None => by_row.push((draft.row, vec![build])),
            }
        }
        by_row.sort_by_key(|(row, _)| *row);
        by_row
            .into_iter()
            .map(|(row, builds)| RowBlock {
                row,
                render: Rc::new(move |_, _| div().flex().flex_col().children(builds.iter().map(|b| b())).into_any_element()),
            })
            .collect()
    }

    // ---- the pane's keys ----

    pub(crate) fn handlers(&self, cx: &mut Context<Self>) -> ReviewHandlers {
        let this = cx.entity().downgrade();
        let with = move |f: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let this = this.clone();
            move |window: &mut Window, cx: &mut gpui_kit::App| {
                this.update(cx, |s, cx| f(s, window, cx)).ok();
            }
        };
        ReviewHandlers::default()
            .on_next(with(|s, w, cx| s.step(1, w, cx)))
            .on_previous(with(|s, w, cx| {
                if s.model.back.is_empty() {
                    s.step(-1, w, cx)
                } else {
                    s.go_back(w, cx)
                }
            }))
            .on_mark(with(|s, _, cx| s.toggle_seen(cx)))
            .on_review_mode(with(|s, _, cx| {
                s.review_mode = !s.review_mode;
                cx.notify();
            }))
            .on_dismiss(with(|s, w, cx| {
                if s.picker.is_some() {
                    s.picker = None;
                    s.focus.focus(w, cx);
                    cx.notify();
                } else if !s.model.back.is_empty() {
                    s.go_back(w, cx);
                } else {
                    s.review_mode = false;
                    cx.notify();
                }
            }))
            .on_toggle_details(with(|s, _, cx| {
                s.details = !s.details;
                cx.notify();
            }))
            .on_toggle_files(with(|s, _, cx| {
                let shown = s.layout().tree;
                s.tree = Some(!shown);
                cx.notify();
            }))
            .on_put_back(with(|s, _, cx| s.put_back(cx)))
            .on_go_to_file(with(|s, w, cx| s.go_to_file(w, cx)))
    }

    pub(crate) fn layout(&self) -> Fit {
        fit(self.width, self.details && !self.review_mode, self.tree)
    }

    /// Asks the owner to open the file on screen in its editor.
    pub fn open_in_editor(&mut self, cx: &mut Context<Self>) {
        if let Some(place) = &self.model.place {
            let line = self.editor.read(cx).cursor_position().line + 1;
            cx.emit(PullEvent::OpenFile { pull: self.model.reference.clone(), path: place.path.clone(), line: Some(line) });
        }
    }

    /// The words under the rail's header: what changed, what is going on, why the diff is what it is.
    pub(crate) fn header_note(&self) -> Option<String> {
        crate::notes::compose(self.notice.as_deref(), self.model.git_error.as_deref(), self.model.base.as_ref().and_then(|b| b.note.as_deref()), &self.model.errors)
    }
}

/// A job that has been asked for and not read, so the same job is not asked for twice.
fn placeholder_job(job: &lathe_forge::JobRef) -> lathe_forge::Job {
    lathe_forge::Job { reference: job.clone(), name: String::new(), status: lathe_forge::CheckStatus::Queued, conclusion: None, run_id: 0, attempt: 1, steps: Vec::new() }
}
