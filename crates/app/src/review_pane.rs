//! The review of what a session's agent changed: the review bar, the changed file tree, and the file's
//! card with the real editor and the inline review's hunks. One turn, or the whole session, by the
//! bar's switch.
//!
//! Each file is a `lathe_review::Merged`, the source of truth: the editor holds its text, a decision
//! calls `decide`, the reader's typing calls `edited`. The file on disk is `Merged::current()`: the pane
//! writes it through the Project at once after a decision, and a moment after the reader stops typing.
//! When the agent writes a file under review again (a later turn), the watch reports it; the pane reads
//! it, rebases the file's hunks on it, and puts the difference in the editor as one small edit, so the
//! caret and the scroll stay where they were.
//!
//! What the reader decided and marked is kept with the session, per scope and file, so the review opens
//! again as it was left. Comments go to the session, which sends them with the next message.

use std::{collections::{HashMap, HashSet}, sync::Arc, time::{Duration, Instant}};

use beui::{
    ActiveTheme, Breadcrumb, ChangedFile, Crumb, ChangedFileTree, Comment, Decision, InlineHunk, InlineReview, LineComment, LineComposer, LineComposerEvent, RowMap,
    ReviewBar, ReviewFileHeader, ReviewHandlers, ReviewProgress,
    file_tree::FileTree,
    inline_review,
    review::{step, whole_file},
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Task, Window, anchored, canvas, deferred, point,
    base::input::{InputEvent, RowBlock},
    component::input::EditorState,
    div, prelude::FluentBuilder, px,
};
use lathe_editor::EditorSession;
use lathe_project::Project;
use lathe_review::{Content, FileReview, Merged};

use crate::{
    agent_session::{AgentSession, SessionEvent},
    ship::{
        kept::{Kept, kept},
        strip::{ShipStrip, StripEvent},
    },
    review_state::Decided,
    review_text::{moved_caret, row_of, splice},
};

/// Makes the language server session for a file of the project, shown with the given rows over it.
pub type SessionFor = std::rc::Rc<dyn Fn(&str, Entity<EditorState>, RowMap, &mut gpui_kit::App) -> Entity<EditorSession>>;

/// How long after the reader's last key the file is written.
const WRITE_AFTER: Duration = Duration::from_millis(300);
/// Below this width the tree hides, and `s`, `w` and the bar walk the files; review mode shows it.
const TREE_FROM: f32 = 680.;

/// Which changes the review holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// One turn, by its index in the session.
    Turn(usize),
    /// The session as one change.
    Whole,
}

impl Scope {
    /// The key `Reviewed` keeps marks under: the turn, or a key no turn has for the whole session.
    pub fn key(self) -> usize {
        match self {
            Self::Turn(turn) => turn,
            Self::Whole => usize::MAX,
        }
    }
}

/// A file as the review holds it: what the turn said about it, its hunks now, and what the pane
/// believes is on disk, so its own writes are no news when the watch reports them.
#[derive(Clone, Debug)]
pub struct PaneFile {
    pub review: FileReview,
    /// `None` for a file with no text to review: binary, or its text before the turn unknown.
    pub merged: Option<Merged>,
    pub on_disk: Option<String>,
    /// The file before each decision, newest last: an undo in the editor brings one back.
    undo: Vec<Merged>,
    /// The text each decision left, beside `undo`: Undo decision acts only while the file holds it.
    decided_text: Vec<String>,
    /// The short id of the commit that took this file's decisions.
    pub committed: Option<String>,
}

impl PaneFile {
    pub fn new(review: FileReview) -> Self {
        let merged = match &review.content {
            Content::Text(merged) => Some(merged.clone()),
            Content::Binary | Content::Unknown => None,
        };
        let on_disk = review.after.clone();
        Self { review, merged, on_disk, undo: Vec::new(), decided_text: Vec::new(), committed: None }
    }

    fn hunks(&self) -> &[InlineHunk] {
        self.merged.as_ref().map_or(&[], |m| m.hunks())
    }

    /// What the disk needs to hold what the reader decided, when it does not yet: the file's text, or no
    /// file at all for one the agent made and the reader rejected whole.
    fn to_disk(&self) -> Option<DiskChange> {
        let merged = self.merged.as_ref()?;
        let text = merged.current();
        let rejected_whole = self.review.before.is_none() && merged.hunks().is_empty() && text.is_empty();
        match (rejected_whole, &self.on_disk) {
            (true, None) => None,
            (true, Some(_)) => Some(DiskChange::Remove),
            (false, on_disk) => (text != on_disk.as_deref().unwrap_or("")).then_some(DiskChange::Write(text)),
        }
    }
}

/// A change the pane makes on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DiskChange {
    Write(String),
    Remove,
}

impl DiskChange {
    /// The file on disk once this is done.
    fn on_disk(&self) -> Option<String> {
        match self {
            Self::Write(text) => Some(text.clone()),
            Self::Remove => None,
        }
    }

    fn apply(&self, project: &dyn Project, path: &str) -> std::io::Result<()> {
        match self {
            Self::Write(text) => project.write(path, text.as_bytes()),
            Self::Remove => project.remove(path),
        }
    }
}

pub enum PaneEvent {
    /// Escape or the bar's close: the pane goes, and the editor comes back.
    Close,
    /// A line for the status line, such as a write that failed.
    Said(SharedString),
    /// The strip made a commit or a branch, so the branch and its changes are to be read again.
    GitChanged,
    /// The reader asked to see this pull request.
    ShowPull(lathe_forge::PullRef),
}

impl EventEmitter<PaneEvent> for ReviewPane {}

pub struct ReviewPane {
    pub session: Entity<AgentSession>,
    project: Arc<dyn Project>,
    pub scope: Scope,
    /// The turn the switch goes back to from the whole session.
    turn: usize,
    /// How many turns the session had when the files were read.
    turns: usize,
    pub files: Vec<PaneFile>,
    /// The files as the tree and the walk list them, made once when the files are read.
    changed: Vec<ChangedFile>,
    pub current: usize,
    editor: Entity<EditorState>,
    /// The project's language servers, when the review has them: the open file's session on its
    /// merged text, which the server reads without the removed rows.
    language: Option<SessionFor>,
    lsp: Option<Entity<EditorSession>>,
    /// Ships what the review kept: the commit, then the push and the pull request.
    pub ship: Entity<ShipStrip>,
    focus: FocusHandle,
    pub review_mode: bool,
    resolving: Vec<beui::Resolve>,
    /// The comment being written: its file, the composer, and what the pane listens to on it.
    composer: Option<(usize, Entity<LineComposer>, [Subscription; 2])>,
    width: f32,
    /// The folder a press on the breadcrumb asked the tree to open, and how many times it has asked.
    reveal: (u64, SharedString),
    writing: Option<Task<()>>,
    /// The whole session's files, read off the UI thread; dropping it drops the read.
    loading: Task<()>,
    /// No files yet: the whole session is being read.
    reading: bool,
    /// The file to open once the files are read.
    opening_at: Option<String>,
    _edits: Subscription,
    _session: Subscription,
    _ship: Subscription,
}

impl ReviewPane {
    pub fn new(
        session: Entity<AgentSession>,
        project: Arc<dyn Project>,
        scope: Scope,
        path: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pressed = Instant::now();
        // A turn's files are at hand; the whole session's come from a background read.
        let files = match scope {
            Scope::Turn(turn) => {
                let s = session.read(cx);
                files_of(s.reviews.turns.turns().get(turn).map(|t| t.files().to_vec()).unwrap_or_default(), scope, &s.reviews.decided, &s.reviews.committed)
            }
            Scope::Whole => Vec::new(),
        };
        let current = path.and_then(|p| files.iter().position(|f| f.review.path == p)).unwrap_or(0);
        let (editor, _edits) = Self::editor_for(files.get(current), window, cx);
        let _session = cx.subscribe_in(&session, window, |this, _, event: &SessionEvent, window, cx| {
            // A turn ended: the whole session holds it now.
            if matches!(event, SessionEvent::Changed) && this.scope == Scope::Whole && this.turns != this.session.read(cx).reviews.turns.turns().len() {
                this.set_scope(Scope::Whole, window, cx);
            }
        });
        let turns = session.read(cx).reviews.turns.turns().len();
        let turn = match scope {
            Scope::Turn(turn) => turn,
            Scope::Whole => turns.saturating_sub(1),
        };
        let (backend, model) = (session.read(cx).agent.backend.clone(), session.read(cx).model.clone());
        let ship = cx.new(|cx| ShipStrip::new(project.clone(), backend, model, window, cx));
        let _ship = cx.subscribe_in(&ship, window, |this, _, event: &StripEvent, window, cx| match event {
            StripEvent::WantsOpen => this.open_ship(window, cx),
            StripEvent::AcceptAllAndCommit => {
                this.decide_every_file(Decision::Accept, window, cx);
                this.open_ship(window, cx);
            }
            StripEvent::Committed { sha, paths, subject } => {
                this.committed(sha, paths, cx);
                let event = crate::tasks::signal::TaskEvent::Committed { sha: sha.clone(), subject: subject.clone() };
                this.session.update(cx, |s, cx| s.tell_task(event, cx));
                // The strip's fields close, so the review takes the keys back: the push key reaches it.
                this.focus.focus(window, cx);
            }
            StripEvent::Pushed => cx.emit(PaneEvent::GitChanged),
            StripEvent::Rewrote(moved) => this.rewrote(moved, cx),
            StripEvent::ShowPull(reference) => cx.emit(PaneEvent::ShowPull(reference.clone())),
            StripEvent::PullOpened(reference) => {
                let reference = reference.clone();
                let event = crate::tasks::signal::TaskEvent::PrOpened { number: reference.number, repo: reference.repo.slug() };
                this.session.update(cx, |s, cx| {
                    s.set_pull(reference, cx);
                    s.tell_task(event, cx);
                });
                cx.emit(PaneEvent::GitChanged);
            }
        });
        // Typing not yet written when the pane goes is written as it goes.
        cx.on_release(|pane: &mut Self, cx| pane.flush(cx)).detach();
        let mut pane = Self {
            session,
            project,
            scope,
            turn,
            turns,
            changed: changed_of(&files),
            files,
            current,
            editor,
            language: None,
            lsp: None,
            ship,
            _ship,
            focus: cx.focus_handle(),
            review_mode: false,
            resolving: Vec::new(),
            composer: None,
            width: f32::MAX,
            reveal: (0, SharedString::default()),
            writing: None,
            loading: Task::ready(()),
            reading: scope == Scope::Whole,
            opening_at: path.map(str::to_string),
            _edits,
            _session,
        };
        if scope == Scope::Whole {
            pane.set_scope(Scope::Whole, window, cx);
        }
        pane.check_disk(pane.files.iter().map(|f| f.review.path.clone()).collect(), window, cx);
        timing(window, format!("review of {} files opened", pane.files.len()), pressed, pressed.elapsed());
        pane
    }

    /// An editor on `file`'s merged text, and the subscription that follows the reader's typing.
    fn editor_for(file: Option<&PaneFile>, window: &mut Window, cx: &mut Context<Self>) -> (Entity<EditorState>, Subscription) {
        let (path, text) = file.map_or(("", String::new()), |f| (f.review.path.as_str(), f.merged.as_ref().map_or_else(String::new, |m| m.text().to_string())));
        let editor = beui::CodeEditor::state(path, text, window, cx);
        let edits = cx.subscribe(&editor, |this, state, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let text = state.read(cx).value().to_string();
                this.typed(text, cx);
            }
        });
        (editor, edits)
    }

    /// The review with the project's language servers on each file it shows.
    pub fn with_language(mut self, language: SessionFor, window: &mut Window, cx: &mut Context<Self>) -> Self {
        self.attach_language(language, window, cx);
        self
    }

    /// Starts the language server's session on the editor the pane already made.
    fn attach_language(&mut self, language: SessionFor, _: &mut Window, cx: &mut Context<Self>) {
        self.language = Some(language);
        self.start_lsp(cx);
    }

    /// The open file's server session, on its editor, when the review has servers and the file has text.
    fn start_lsp(&mut self, cx: &mut Context<Self>) {
        self.lsp = match (&self.language, self.files.get(self.current)) {
            (Some(language), Some(file)) if file.merged.is_some() => Some(language(&file.review.path, self.editor.clone(), RowMap::new(file.hunks()), cx)),
            _ => None,
        };
    }

    /// What the review kept, file by file: the accepted hunks and the reader's edits in them.
    pub fn kept(&self) -> Vec<Kept> {
        self.files.iter().filter_map(|f| kept(&f.review, f.merged.as_ref())).collect()
    }

    /// The strip committed `paths`: each shows as committed, with no undo left that would change it
    /// under the commit, and the project reads git again.
    fn committed(&mut self, sha: &str, paths: &[String], cx: &mut Context<Self>) {
        let short: String = sha.chars().take(7).collect();
        let scope = self.scope;
        let mut keys = Vec::new();
        for file in self.files.iter_mut().filter(|f| paths.contains(&f.review.path)) {
            file.committed = Some(short.clone());
            file.undo.clear();
            file.decided_text.clear();
            keys.push((scope, file.review.path.clone()));
        }
        self.session.update(cx, |s, cx| {
            s.reviews.committed.extend(keys.into_iter().map(|key| (key, short.clone())));
            s.save_review(cx);
        });
        cx.emit(PaneEvent::GitChanged);
        cx.notify();
    }

    /// A rebase gave commits new ids: each mark and the session's record name the new one.
    fn rewrote(&mut self, moved: &[(String, String)], cx: &mut Context<Self>) {
        let short = |sha: &str| sha.chars().take(7).collect::<String>();
        let map: Vec<(String, String)> = moved.iter().map(|(old, new)| (short(old), short(new))).collect();
        let renamed = |sha: &mut String| {
            if let Some((_, new)) = map.iter().find(|(old, _)| old == sha) {
                *sha = new.clone();
            }
        };
        for file in &mut self.files {
            if let Some(sha) = file.committed.as_mut() {
                renamed(sha);
            }
        }
        self.session.update(cx, |s, cx| {
            s.reviews.committed.values_mut().for_each(renamed);
            s.save_review(cx);
        });
        cx.emit(PaneEvent::GitChanged);
        cx.notify();
    }

    /// The card of the session's pull request.
    #[cfg(test)]
    pub fn session_card(&self, cx: &gpui_kit::App) -> Option<Entity<crate::pull_card::PullCard>> {
        self.session.read(cx).pull_card.clone()
    }

    /// The pull request the session's record keeps.
    #[cfg(test)]
    pub fn session_pull(&self, cx: &gpui_kit::App) -> Option<lathe_forge::PullRef> {
        self.session.read(cx).reviews.pull.clone()
    }

    /// The commits the session's record names, by scope and path.
    #[cfg(test)]
    pub fn session_committed(&self, cx: &gpui_kit::App) -> HashMap<(Scope, String), String> {
        self.session.read(cx).reviews.committed.clone()
    }

    /// "Committed in <id>" for file `at` once its decisions are committed and none is left to make.
    pub fn committed_words(&self, at: usize) -> Option<SharedString> {
        let file = self.files.get(at)?;
        let short = file.committed.as_ref().filter(|_| file.hunks().is_empty())?;
        Some(format!("Committed in {short}").into())
    }

    /// How file `at` was decided, once no hunk of it is left to decide: "Accepted" when it holds the
    /// turn's text, "Rejected" when it holds the text before the turn, "Decided" for a mix. `None` while
    /// a hunk waits, and for a file whose decisions are committed.
    pub fn decided_words(&self, at: usize) -> Option<SharedString> {
        let file = self.files.get(at)?;
        let merged = file.merged.as_ref()?;
        if !file.hunks().is_empty() || file.committed.is_some() {
            return None;
        }
        let now = merged.current();
        let words = if file.review.after.as_deref() == Some(now.as_str()) {
            "Accepted"
        } else if file.review.before.as_deref().unwrap_or("") == now {
            "Rejected"
        } else {
            "Decided"
        };
        Some(words.into())
    }

    /// Brings back the open file as it was before its last decision, on disk too.
    pub fn undo_decision(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.current;
        let Some(file) = self.files.get_mut(at) else { return };
        let now = file.merged.as_ref().map(|m| m.text().to_string());
        // Typing after the decision would be lost: the editor's own undo takes it back first.
        if file.decided_text.last().is_some_and(|left| Some(left) != now.as_ref()) {
            return cx.emit(PaneEvent::Said("You typed after the last decision: undo the typing first with ⌘Z".into()));
        }
        let Some(before) = file.undo.pop() else { return };
        file.decided_text.pop();
        file.merged = Some(before);
        // The hunk comes back, so the editor is made again from the file's state, with its marks.
        self.load_editor(window, cx);
        self.write(&[at], cx);
        cx.notify();
    }

    /// Whether an undo in the editor can bring back file `at` before a decision.
    #[cfg(test)]
    pub fn can_undo(&self, at: usize) -> bool {
        self.files.get(at).is_some_and(|f| !f.undo.is_empty())
    }

    /// Opens the Ship strip on what the review kept. Typing not yet written is written first.
    pub(crate) fn open_ship(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.writing.take().is_some() {
            self.write(&[self.current], cx);
        }
        let kept = self.kept();
        // Asked again with nothing accepted, the key accepts all and commits, as the strip offers.
        if kept.is_empty() && self.ship.read(cx).stage == crate::ship::strip::Stage::NothingKept {
            return self.ship.update(cx, |strip, cx| strip.accept_all_and_commit(cx));
        }
        let refs = self.session.read(cx).task.as_ref().map(|t| t.key.clone());
        self.ship.update(cx, |strip, cx| {
            strip.set_refs(refs);
            strip.open(kept, window, cx)
        });
    }

    /// The server's words for the status line, when the open file has one.
    pub fn status(&self, cx: &gpui_kit::App) -> Vec<SharedString> {
        self.lsp.as_ref().map(|lsp| lsp.read(cx).status()).unwrap_or_default()
    }

    /// Tells the open file's server which shown rows the file does not have, after its hunks moved.
    fn sync_rows(&self, cx: &mut Context<Self>) {
        if let (Some(lsp), Some(file)) = (&self.lsp, self.files.get(self.current)) {
            let rows = RowMap::new(file.hunks());
            lsp.update(cx, |s, cx| s.set_rows(rows, cx));
        }
    }

    /// Puts the open file in a new editor. When the reader's keys were in the old one, they go to the
    /// new one, or the next key would reach nothing.
    fn load_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let had_keys = self.editor.focus_handle(cx).is_focused(window);
        (self.editor, self._edits) = Self::editor_for(self.files.get(self.current), window, cx);
        self.start_lsp(cx);
        if had_keys {
            focus_once_painted(self.editor.focus_handle(cx), 3, window, cx);
        }
    }

    /// The editor's text changed. The pane's own edits leave it equal to the file's merged text; an undo
    /// of a decision brings back the file as it was before it; anything else is the reader's typing, and
    /// the hunks move with it.
    fn typed(&mut self, text: String, cx: &mut Context<Self>) {
        let at = self.current;
        let Some(file) = self.files.get_mut(at) else { return };
        let Some(merged) = &file.merged else { return };
        if merged.text() == text {
            return;
        }
        file.merged = Some(match file.undo.iter().rposition(|m| m.text() == text) {
            Some(back) => {
                let before = file.undo[back].clone();
                file.undo.truncate(back);
                file.decided_text.truncate(back);
                before
            }
            None => merged.edited(&text),
        });
        self.sync_rows(cx);
        let this = cx.entity().downgrade();
        let timer = cx.background_executor().timer(WRITE_AFTER);
        self.writing = Some(cx.spawn(async move |_, cx| {
            timer.await;
            _ = this.update(cx, |pane, cx| pane.write(&[at], cx));
        }));
        cx.notify();
    }

    /// Keeps file `at` as it stands with the session, so the review opens again as it was left.
    fn keep(&self, at: usize, cx: &mut Context<Self>) {
        let file = &self.files[at];
        let (key, kept) = ((self.scope, file.review.path.clone()), (file.merged.clone(), file.on_disk.clone()));
        self.session.update(cx, |s, cx| {
            s.reviews.decided.insert(key, kept);
            s.save_review(cx);
        });
    }

    /// Keeps files `at` with the session and writes the ones whose text is not on disk yet, off the UI
    /// thread. An accept leaves the disk as it is, but its decision is kept all the same.
    fn write(&mut self, at: &[usize], cx: &mut Context<Self>) {
        let mut writes = Vec::new();
        let count = self.files.len();
        for &i in at.iter().filter(|i| **i < count) {
            if let Some(change) = self.files[i].to_disk() {
                self.files[i].on_disk = change.on_disk();
                writes.push((self.files[i].review.path.clone(), change));
            }
            self.keep(i, cx);
        }
        if writes.is_empty() {
            return;
        }
        let project = self.project.clone();
        let written = cx.background_spawn(async move {
            writes.into_iter().filter_map(|(path, change)| change.apply(project.as_ref(), &path).err().map(|e| format!("Could not write {path}: {e}"))).collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            for error in written.await {
                _ = this.update(cx, |_, cx| cx.emit(PaneEvent::Said(error.into())));
            }
        })
        .detach();
    }

    /// Writes the open file when typing waits to be written, as the pane goes: the writes run on their
    /// own, past the pane.
    fn flush(&mut self, cx: &mut gpui_kit::App) {
        if self.writing.take().is_none() {
            return;
        }
        let Some(change) = self.files.get(self.current).and_then(PaneFile::to_disk) else { return };
        let (project, path) = (self.project.clone(), self.files[self.current].review.path.clone());
        cx.background_spawn(async move { drop(change.apply(project.as_ref(), &path)) }).detach();
    }

    /// Files of the project changed on disk: those under review are read again, and the ones the agent
    /// wrote (not the pane) take the new text, with the reader's decisions kept.
    pub fn check_disk(&mut self, paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let ours: Vec<String> = paths.into_iter().filter(|p| self.files.iter().any(|f| &f.review.path == p && f.merged.is_some())).collect();
        if ours.is_empty() {
            return;
        }
        let project = self.project.clone();
        let read = cx.background_spawn(async move {
            ours.into_iter().map(|path| {
                let text = project.read(&path).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
                (path, text)
            }).collect::<Vec<_>>()
        });
        cx.spawn_in(window, async move |this, cx| {
            let texts = read.await;
            _ = this.update_in(cx, |pane, window, cx| {
                for (path, text) in texts {
                    pane.agent_wrote(&path, text, window, cx);
                }
            });
        })
        .detach();
    }

    fn agent_wrote(&mut self, path: &str, text: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.files.iter().position(|f| f.review.path == path) else { return };
        let file = &mut self.files[at];
        let Some(merged) = &file.merged else { return };
        if file.on_disk == text {
            return;
        }
        let rebased = merged.rebased_on(text.as_deref().unwrap_or(""));
        file.on_disk = text;
        file.undo.clear();
        file.decided_text.clear();
        let new_text = rebased.text().to_string();
        file.merged = Some(rebased);
        self.keep(at, cx);
        if at == self.current {
            self.resolving.clear();
            put_text(&self.editor, &new_text, window, cx);
            self.sync_rows(cx);
        }
        cx.notify();
    }

    fn open(&mut self, path: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.files.iter().position(|f| f.review.path == path.as_ref()) else { return };
        if at == self.current {
            return;
        }
        // A file left with typing not yet written is written now.
        if self.writing.take().is_some() {
            self.write(&[self.current], cx);
        }
        self.current = at;
        self.resolving.clear();
        self.composer = None;
        self.load_editor(window, cx);
        cx.notify();
    }

    /// A file is reviewed once no hunk is left in it, or once the reader marked it.
    fn reviewed(&self, cx: &gpui_kit::App) -> HashSet<SharedString> {
        let s = self.session.read(cx);
        self.files
            .iter()
            .filter(|f| (f.merged.is_some() && f.hunks().is_empty()) || s.reviews.is_reviewed(self.scope.key(), &f.review))
            .map(|f| f.review.path.clone().into())
            .collect()
    }

    fn progress(&self, reviewed: &HashSet<SharedString>) -> ReviewProgress {
        let (added, removed) = self.files.iter().fold((0, 0), |(a, r), f| {
            let (fa, fr) = f.review.counts();
            (a + fa, r + fr)
        });
        ReviewProgress { files: self.files.len(), reviewed: reviewed.len(), added, removed }
    }

    fn toggle_mark(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.files.get(self.current) else { return };
        let (key, review) = (self.scope.key(), file.review.clone());
        self.session.update(cx, |s, cx| {
            let on = !s.reviews.is_reviewed(key, &review);
            s.reviews.set_reviewed(key, &review, on);
            s.save_review(cx);
        });
        cx.notify();
    }

    fn step(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let order = FileTree::new(&self.changed).file_order();
        let current = self.files.get(self.current).map(|f| SharedString::from(f.review.path.clone()));
        if let Some(path) = step(&order, current.as_ref(), by) {
            self.open(&path, window, cx);
        }
    }

    /// Applies `decisions` to file `at`: its `Merged`, its undo, and the editor when it is the open
    /// one. The caller writes. False when there was nothing to decide.
    fn decide(&mut self, at: usize, decisions: &[(InlineHunk, Decision)], window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(merged) = self.files.get(at).and_then(|f| f.merged.clone()) else { return false };
        if decisions.is_empty() {
            return false;
        }
        let decided = decisions.iter().fold(merged.clone(), |m, (h, d)| m.decide(&h.id, *d).unwrap_or(m));
        let file = &mut self.files[at];
        file.undo.push(merged);
        file.decided_text.push(decided.text().to_string());
        file.merged = Some(decided);
        if at == self.current {
            inline_review::apply(&self.editor, decisions, window, cx);
            self.sync_rows(cx);
        }
        true
    }

    /// Decides hunk `id` of the open file, writes the file, and gives the rows that closed; `None` when
    /// the file has no such hunk.
    pub(crate) fn decide_hunk(&mut self, id: &str, decision: Decision, window: &mut Window, cx: &mut Context<Self>) -> Option<std::ops::Range<usize>> {
        let at = self.current;
        let hunk = self.files.get(at)?.hunks().iter().find(|h| h.id == id).cloned()?;
        let closed = hunk.closing(decision);
        self.decide(at, &[(hunk, decision)], window, cx);
        self.write(&[at], cx);
        cx.notify();
        Some(closed)
    }

    /// Decides every hunk of the open file at once, writes it, and moves to the next file.
    fn decide_file(&mut self, decision: Decision, window: &mut Window, cx: &mut Context<Self>) {
        let started = Instant::now();
        self.decide_file_now(decision, window, cx);
        timing(window, "review: a whole file decided".into(), started, started.elapsed());
    }

    fn decide_file_now(&mut self, decision: Decision, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.current;
        self.resolving.clear();
        let decisions = self.files.get(at).map(|f| whole_file(f.hunks(), decision)).unwrap_or_default();
        if self.decide(at, &decisions, window, cx) {
            self.write(&[at], cx);
        }
        self.step(1, window, cx);
        cx.notify();
    }

    /// Decides every hunk of every file, and writes them.
    fn decide_every_file(&mut self, decision: Decision, window: &mut Window, cx: &mut Context<Self>) {
        self.resolving.clear();
        let decided: Vec<usize> = (0..self.files.len())
            .filter(|&at| {
                let decisions = whole_file(self.files[at].hunks(), decision);
                self.decide(at, &decisions, window, cx)
            })
            .collect();
        self.write(&decided, cx);
        cx.notify();
    }

    /// Reads the files of `scope` from the session, and opens the one at the same path if it has it.
    /// A turn's files are at hand; the whole session is diffed off the UI thread, and the scope drawn
    /// now stays until its files land. A later switch drops a read still running.
    fn set_scope(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        let turns = session.reviews.turns.turns().len();
        match scope {
            Scope::Turn(turn) => {
                let reviews = session.reviews.turns.turns().get(turn).map(|t| t.files().to_vec()).unwrap_or_default();
                self.loading = Task::ready(());
                self.show_scope(scope, reviews, turns, window, cx);
            }
            Scope::Whole => {
                let all = session.reviews.turns.clone();
                let whole = cx.background_spawn(async move { all.whole() });
                self.loading = cx.spawn_in(window, async move |this, cx| {
                    let reviews = whole.await;
                    _ = this.update_in(cx, |pane, window, cx| pane.show_scope(scope, reviews, turns, window, cx));
                });
            }
        }
    }

    /// Shows `reviews`, the files of `scope`, with what the reader decided before, at the same file.
    fn show_scope(&mut self, scope: Scope, reviews: Vec<FileReview>, turns: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.writing.take().is_some() {
            self.write(&[self.current], cx);
        }
        let path = self.files.get(self.current).map(|f| f.review.path.clone()).or_else(|| self.opening_at.take());
        if let Scope::Turn(turn) = scope {
            self.turn = turn;
        }
        self.scope = scope;
        self.reading = false;
        self.files = files_of(reviews, scope, &self.session.read(cx).reviews.decided, &self.session.read(cx).reviews.committed);
        self.changed = changed_of(&self.files);
        self.turns = turns;
        self.current = path.and_then(|p| self.files.iter().position(|f| f.review.path == p)).unwrap_or(0);
        self.resolving.clear();
        self.composer = None;
        self.load_editor(window, cx);
        self.check_disk(self.files.iter().map(|f| f.review.path.clone()).collect(), window, cx);
        cx.notify();
    }

    fn switch_scope(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scope = match self.scope {
            Scope::Turn(_) => Scope::Whole,
            Scope::Whole => Scope::Turn(self.turn),
        };
        self.set_scope(scope, window, cx);
    }

    fn open_composer(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let composer = cx.new(|cx| LineComposer::new(row, window, cx));
        let sub = cx.subscribe(&composer, |this, _, event: &LineComposerEvent, cx| {
            if let LineComposerEvent::Submit { row, text } = event {
                this.comment(*row, text.to_string(), cx);
            }
            this.composer = None;
            cx.notify();
        });
        // The composer is drawn in a row block, which the editor draws only when it draws itself: each
        // change of the composer draws the editor again, or what is typed would not show.
        let redraw = cx.observe(&composer, |this, _, cx| this.editor.update(cx, |_, cx| cx.notify()));
        focus_once_painted(composer.focus_handle(cx), 3, window, cx);
        self.composer = Some((self.current, composer, [sub, redraw]));
        cx.notify();
    }

    /// Adds the reader's comment on `row` of the open file to the session, for the next message.
    fn comment(&mut self, row: usize, body: impl Into<String>, cx: &mut Context<Self>) {
        let Some(file) = self.files.get(self.current) else { return };
        let Some(anchor) = file.merged.as_ref().and_then(|m| m.anchor(row..row + 1)) else { return };
        let (turn, path) = (self.turn, file.review.path.clone());
        self.session.update(cx, |s, cx| {
            s.reviews.comments.add(turn, path, anchor, body);
            s.save_review(cx);
            cx.notify();
        });
    }

    fn handlers(&self, cx: &mut Context<Self>) -> ReviewHandlers {
        let this = cx.entity().downgrade();
        let with = move |f: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let this = this.clone();
            move |window: &mut Window, cx: &mut gpui_kit::App| {
                this.update(cx, |pane, cx| f(pane, window, cx)).ok();
            }
        };
        ReviewHandlers::default()
            .on_next(with(|s, w, cx| s.step(1, w, cx)))
            .on_previous(with(|s, w, cx| s.step(-1, w, cx)))
            .on_accept_file(with(|s, w, cx| s.decide_file(Decision::Accept, w, cx)))
            .on_reject_file(with(|s, w, cx| s.decide_file(Decision::Reject, w, cx)))
            .on_accept_all(with(|s, w, cx| s.decide_every_file(Decision::Accept, w, cx)))
            .on_reject_all(with(|s, w, cx| s.decide_every_file(Decision::Reject, w, cx)))
            .on_switch_scope(with(|s, w, cx| s.switch_scope(w, cx)))
            .on_comment(with(|s, w, cx| {
                let row = s.editor.read(cx).cursor_position().line as usize;
                s.open_composer(row, w, cx)
            }))
            .on_commit(with(|s, w, cx| s.open_ship(w, cx)))
            .on_undo_decision(with(|s, w, cx| s.undo_decision(w, cx)))
            .on_push(with(|s, w, cx| s.ship.update(cx, |strip, cx| strip.push(w, cx))))
            .on_open_pull(with(|s, w, cx| s.ship.update(cx, |strip, cx| strip.open_pull(w, cx))))
            .on_mark(with(|s, _, cx| s.toggle_mark(cx)))
            .on_review_mode(with(|s, _, cx| {
                s.review_mode = !s.review_mode;
                cx.notify();
            }))
            .on_dismiss(with(|s, _, cx| {
                if s.review_mode {
                    s.review_mode = false;
                    cx.notify();
                } else {
                    cx.emit(PaneEvent::Close);
                }
            }))
    }

    /// The comments on the open file, as blocks under their rows: those waiting for the next message,
    /// those sent (resolved once the agent's turn after them ended), and the one being written.
    fn blocks(&self, cx: &mut Context<Self>) -> Vec<RowBlock> {
        let Some(file) = self.files.get(self.current) else { return Vec::new() };
        let Some(merged) = &file.merged else { return Vec::new() };
        let s = self.session.read(cx);
        let path = file.review.path.as_str();
        let mut blocks: Vec<RowBlock> = Vec::new();
        let waiting = s.reviews.comments.for_file(path).map(|c| (c.clone(), None));
        let sent = s.reviews.sent.iter().filter(|(c, _)| c.path == path).map(|(c, answered)| (c.clone(), Some(*answered)));
        for (comment, sent) in waiting.chain(sent).collect::<Vec<_>>() {
            let Some(row) = row_of(merged, comment.side, comment.last_line) else { continue };
            let time = match sent {
                None => "not sent yet",
                Some(false) => "sent",
                Some(true) => "answered",
            };
            let entry = Comment::new("You", time, comment.body.clone());
            let (session, id) = (self.session.clone(), comment.id);
            let element = gpui_kit::ElementId::NamedInteger("comment".into(), id);
            let this = cx.entity().downgrade();
            blocks.push(RowBlock {
                row,
                render: std::rc::Rc::new(move |_, _| {
                    let thread = LineComment::new(element.clone(), vec![entry.clone()]).resolved(sent == Some(true));
                    match sent {
                        // Waiting: Resolve takes it back before it goes; Reply adds another on its row.
                        None => {
                            let (session, reply) = (session.clone(), this.clone());
                            thread
                                .on_resolve(move |_, _, cx| session.update(cx, |s, cx| {
                                    s.reviews.comments.remove(id);
                                    s.save_review(cx);
                                    cx.notify();
                                }))
                                .on_reply(move |_, window, cx| drop(reply.update(cx, |p, cx| p.open_composer(row, window, cx))))
                                .into_any_element()
                        }
                        Some(_) => thread.into_any_element(),
                    }
                }),
            });
        }
        if let Some((at, composer, _)) = &self.composer
            && *at == self.current
        {
            let (composer, row) = (composer.clone(), composer.read(cx).row());
            blocks.push(RowBlock { row, render: std::rc::Rc::new(move |_, _| composer.clone().into_any_element()) });
        }
        blocks
    }
}

/// Focuses `handle` now and again at each of the next `frames` frames: a composer in a row block is
/// painted only once the editor has laid it out, a frame or two later, and a frame that does not paint
/// the focused handle drops the focus. It counts as focused until then, so the retry cannot ask.
fn focus_once_painted(handle: FocusHandle, frames: usize, window: &mut Window, cx: &mut gpui_kit::App) {
    handle.focus(window, cx);
    if frames > 0 {
        window.on_next_frame(move |window, cx| focus_once_painted(handle, frames - 1, window, cx));
    }
}

/// With `LATHE_TIMINGS=1`, prints what an action cost: its own `work`, how long after `since` the frame
/// that shows it began (the wait for the display), and that frame's layout and paint (with
/// `LATHE_FRAMES=1`). A next-frame callback runs as a frame begins, before it is drawn, so the frame's
/// cost is read at the start of the one after it.
fn timing(window: &Window, what: String, since: Instant, work: Duration) {
    if std::env::var("LATHE_TIMINGS").is_ok_and(|v| v == "1") {
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        window.on_next_frame(move |window, _| {
            let began = since.elapsed();
            window.on_next_frame(move |_, _| {
                let frame = crate::frame_meter::last_frame();
                eprintln!("{what}: work {:.1} ms, its frame began after {:.1} ms and took {:.1} ms", ms(work), ms(began), ms(frame));
            });
        });
    }
}

/// The list the tree and the walk take: each file's path, `+a -r` and change.
fn changed_of(files: &[PaneFile]) -> Vec<ChangedFile> {
    let reviews: Vec<FileReview> = files.iter().map(|f| f.review.clone()).collect();
    lathe_review::present::changed_files(&reviews)
}

/// The files of `scope`, with what the reader decided before where the session kept it.
fn files_of(reviews: Vec<FileReview>, scope: Scope, decided: &Decided, committed: &HashMap<(Scope, String), String>) -> Vec<PaneFile> {
    reviews
        .into_iter()
        .map(|review| {
            let mut file = PaneFile::new(review);
            if let Some((merged, on_disk)) = decided.get(&(scope, file.review.path.clone())) {
                (file.merged, file.on_disk) = (merged.clone(), on_disk.clone());
            }
            file.committed = committed.get(&(scope, file.review.path.clone())).cloned();
            file
        })
        .collect()
}

/// Puts `text` in the editor as the one edit between its text and `text`, keeping the caret with the
/// text around it and the scroll where it was.
fn put_text(editor: &Entity<EditorState>, text: &str, window: &mut Window, cx: &mut gpui_kit::App) {
    editor.update(cx, |state, cx| {
        let old = state.value().to_string();
        let (range, with) = splice(&old, text);
        if range.is_empty() && with.is_empty() {
            return;
        }
        let (caret, scroll) = (state.cursor(), state.scroll_offset());
        state.set_selected_range(range.clone(), cx);
        state.replace(with.to_string(), window, cx);
        let caret = moved_caret(caret, &range, with.len());
        state.set_selected_range(caret..caret, cx);
        state.set_scroll_offset(scroll, cx);
    });
}

impl Focusable for ReviewPane {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ReviewPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let handlers = self.handlers(cx);
        let reviewed = self.reviewed(cx);
        let progress = self.progress(&reviewed);
        let scope = match self.scope {
            Scope::Turn(_) => 0,
            Scope::Whole => 1,
        };
        let bar = ReviewBar::new("review-bar", progress, handlers.clone())
            .review_mode(self.review_mode)
            .scopes([("This turn".into(), "Turn".into()), ("Whole session".into(), "Session".into())], scope);

        let Some(file) = self.files.get(self.current) else {
            let empty = div()
                .flex()
                .flex_col()
                .size_full()
                .gap(px(8.))
                .child(bar)
                .child(div().flex().flex_1().items_center().justify_center().text_size(TextSize::Sm.font_size()).text_color(theme.muted_foreground).child(if self.reading { "Reading the session's changes…" } else { "Nothing changed" }));
            return handlers.keys(empty, &self.focus).into_any_element();
        };
        let path: SharedString = file.review.path.clone().into();
        let hunks = file.hunks().to_vec();
        let (added, removed) = file.review.counts();
        let decide = cx.listener(|this, (id, decision): &(SharedString, Decision), _, cx| {
            let (now, reduce) = (Instant::now(), cx.reduce_motion());
            this.resolving.retain(|r| !r.is_over(now, reduce));
            if !this.resolving.iter().any(|r| &r.id == id) {
                this.resolving.push(beui::Resolve::new(id.clone(), *decision));
                cx.notify();
            }
        });
        let resolved = cx.listener(|this, (id, decision): &(SharedString, Decision), window: &mut Window, cx| {
            // A frame can report the same finished fade twice; only the first one edits.
            let Some(at) = this.resolving.iter().position(|r| &r.id == id && !r.is_edited()) else { return };
            match this.decide_hunk(id, *decision, window, cx) {
                Some(closed) => this.resolving[at].edited(closed),
                None => drop(this.resolving.remove(at)),
            }
        });
        let open = cx.listener(|this, path: &SharedString, window, cx| {
            this.open(path, window, cx);
            this.editor.update(cx, |state, cx| state.focus(window, cx));
        });
        let add = cx.listener(|this, row: &usize, window, cx| this.open_composer(*row, window, cx));
        let blocks = self.blocks(cx);
        let what = match (&file.review.content, file.review.exact) {
            (Content::Binary, _) => Some("Not text: listed, with no hunks."),
            (Content::Unknown, _) => Some("Its text before the turn is unknown, so it has no hunks."),
            (Content::Text(_), false) => Some("The text before the turn is the last commit's: edits you had not committed show as the agent's."),
            (Content::Text(_), true) => None,
        };
        let review = InlineReview::new("review-editor", &self.editor, hunks.clone())
            .on_card(true)
            .fill(true)
            .when_some(hunks.first(), |r, h| r.current(h.id.clone()))
            .resolving(self.resolving.clone())
            .row_blocks(blocks)
            .on_add_comment(move |row, window, cx| add(&row, window, cx))
            .on_decide(move |id, decision, window, cx| decide(&(id.clone(), decision), window, cx))
            .on_resolved(move |id, decision, window, cx| resolved(&(id.clone(), decision), window, cx));
        let header = ReviewFileHeader::new("review-file", path.clone(), added, removed, handlers.clone()).path_shown(false).committed(self.committed_words(self.current)).decided(self.decided_words(self.current));
        let crumbs = {
            let parts: Vec<String> = path.split('/').map(str::to_string).collect();
            let this = cx.entity().downgrade();
            let folders = parts.clone();
            div().flex().flex_none().px(px(4.)).child(
                Breadcrumb::new("review-crumbs", parts.iter().map(|p| Crumb::new(p.clone()))).debug_name("review-crumb").on_press(move |i, _, cx| {
                    let folder = folders[..=i].join("/");
                    this.update(cx, |pane, cx| {
                        pane.reveal = (pane.reveal.0 + 1, folder.into());
                        cx.notify();
                    })
                    .ok();
                }),
            )
        };
        let file_card = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme.card)
            .rounded(radius::LG)
            .p(px(8.))
            .child(crumbs)
            .child(header)
            .children(what.map(|w| div().px(px(12.)).pb(px(8.)).text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(w)))
            .child(div().flex_1().min_h_0().child(review));
        let show_tree = self.review_mode || self.width >= TREE_FROM;
        let tree = show_tree.then(|| {
            div().flex_none().w(px(220.)).h_full().bg(theme.card).rounded(radius::LG).p(px(8.)).child(
                ChangedFileTree::new("review-tree", self.changed.clone()).reveal(self.reveal.0, self.reveal.1.clone()).reviewed(reviewed).current(path).on_open(move |path, window, cx| open(path, window, cx)),
            )
        });
        let this = cx.entity().downgrade();
        let measure = canvas(
            move |bounds, _, cx| {
                let width = f32::from(bounds.size.width);
                _ = this.update(cx, |pane, cx| {
                    if (pane.width - width).abs() > 0.5 {
                        pane.width = width;
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let pane = div()
            .id("review-pane")
            .relative()
            .flex()
            .flex_col()
            .gap(px(8.))
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .child(measure)
            .child(bar)
            .child(self.ship.clone())
            .child(div().flex().flex_1().min_h_0().gap(px(8.)).children(tree).child(file_card));
        let pane = handlers.keys(pane, &self.focus);
        if !self.review_mode {
            return pane.into_any_element();
        }
        // Review mode: the files on the whole window, over everything else, until `r` or Escape.
        let size = window.viewport_size();
        deferred(anchored().position(point(px(0.), px(0.))).child(div().w(size.width).h(size.height).p(px(16.)).bg(theme.background).child(pane)))
            .with_priority(1)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
