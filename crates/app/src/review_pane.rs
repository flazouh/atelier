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

use std::{collections::HashSet, sync::Arc, time::{Duration, Instant}};

use beui::{
    ActiveTheme, ChangedFile, ChangedFileTree, Comment, Decision, InlineReview, LineComment, LineComposer, LineComposerEvent,
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
use lathe_project::Project;
use lathe_review::{Content, FileReview, Merged};

use crate::{
    agent_session::{AgentSession, SessionEvent},
    review_text::{moved_caret, row_of, splice},
};

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
}

impl PaneFile {
    pub fn new(review: FileReview) -> Self {
        let merged = match &review.content {
            Content::Text(merged) => Some(merged.clone()),
            Content::Binary | Content::Unknown => None,
        };
        let on_disk = review.after.clone();
        Self { review, merged, on_disk, undo: Vec::new() }
    }

    fn hunks(&self) -> &[beui::InlineHunk] {
        self.merged.as_ref().map_or(&[], |m| m.hunks())
    }

    /// The text to write, when it differs from the disk: the file with what the reader decided.
    fn to_write(&self) -> Option<String> {
        let text = self.merged.as_ref()?.current();
        (text != self.on_disk.as_deref().unwrap_or("")).then_some(text)
    }
}

pub enum PaneEvent {
    /// Escape or the bar's close: the pane goes, and the editor comes back.
    Close,
    /// A line for the status line, such as a write that failed.
    Said(SharedString),
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
    pub current: usize,
    editor: Entity<EditorState>,
    focus: FocusHandle,
    pub review_mode: bool,
    resolving: Vec<beui::Resolve>,
    composer: Option<(usize, Entity<LineComposer>, Subscription)>,
    width: f32,
    writing: Option<Task<()>>,
    _edits: Subscription,
    _session: Subscription,
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
        let files = read_files(session.read(cx), scope);
        let current = path.and_then(|p| files.iter().position(|f| f.review.path == p)).unwrap_or(0);
        let (editor, _edits) = Self::editor_for(files.get(current), window, cx);
        let _session = cx.subscribe_in(&session, window, |this, _, event: &SessionEvent, window, cx| {
            // A turn ended: the whole session holds it now.
            if matches!(event, SessionEvent::Changed) && this.scope == Scope::Whole && this.turns != this.session.read(cx).review.turns().len() {
                this.set_scope(Scope::Whole, window, cx);
            }
        });
        let turns = session.read(cx).review.turns().len();
        let turn = match scope {
            Scope::Turn(turn) => turn,
            Scope::Whole => turns.saturating_sub(1),
        };
        let mut pane = Self {
            session,
            project,
            scope,
            turn,
            turns,
            files,
            current,
            editor,
            focus: cx.focus_handle(),
            review_mode: false,
            resolving: Vec::new(),
            composer: None,
            width: f32::MAX,
            writing: None,
            _edits,
            _session,
        };
        pane.check_disk(pane.files.iter().map(|f| f.review.path.clone()).collect(), window, cx);
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
                before
            }
            None => merged.edited(&text),
        });
        self.keep(at, cx);
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
        let (scope, file) = (self.scope, self.files[at].clone());
        self.session.update(cx, |s, _| drop(s.decided.insert((scope, file.review.path.clone()), (file.merged, file.on_disk))));
    }

    /// Writes files `at` with what the reader decided, off the UI thread.
    fn write(&mut self, at: &[usize], cx: &mut Context<Self>) {
        let mut writes = Vec::new();
        for &i in at {
            let Some(text) = self.files.get(i).and_then(PaneFile::to_write) else { continue };
            self.files[i].on_disk = Some(text.clone());
            self.keep(i, cx);
            writes.push((self.files[i].review.path.clone(), text));
        }
        if writes.is_empty() {
            return;
        }
        let project = self.project.clone();
        let written = cx.background_spawn(async move {
            writes.into_iter().filter_map(|(path, text)| project.write(&path, text.as_bytes()).err().map(|e| format!("Could not write {path}: {e}"))).collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            for error in written.await {
                _ = this.update(cx, |_, cx| cx.emit(PaneEvent::Said(error.into())));
            }
        })
        .detach();
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
        let new_text = rebased.text().to_string();
        file.merged = Some(rebased);
        self.keep(at, cx);
        if at == self.current {
            self.resolving.clear();
            put_text(&self.editor, &new_text, window, cx);
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
        (self.editor, self._edits) = Self::editor_for(self.files.get(at), window, cx);
        cx.notify();
    }

    fn changed(&self) -> Vec<ChangedFile> {
        let files: Vec<FileReview> = self.files.iter().map(|f| f.review.clone()).collect();
        lathe_review::present::changed_files(&files)
    }

    /// A file is reviewed once no hunk is left in it, or once the reader marked it.
    fn reviewed(&self, cx: &gpui_kit::App) -> HashSet<SharedString> {
        let s = self.session.read(cx);
        self.files
            .iter()
            .filter(|f| (f.merged.is_some() && f.hunks().is_empty()) || s.reviewed.is_reviewed(self.scope.key(), &f.review))
            .map(|f| f.review.path.clone().into())
            .collect()
    }

    fn progress(&self, cx: &gpui_kit::App) -> ReviewProgress {
        let (added, removed) = self.files.iter().fold((0, 0), |(a, r), f| {
            let (fa, fr) = f.review.counts();
            (a + fa, r + fr)
        });
        ReviewProgress { files: self.files.len(), reviewed: self.reviewed(cx).len(), added, removed }
    }

    fn toggle_mark(&mut self, cx: &mut Context<Self>) {
        let Some(file) = self.files.get(self.current) else { return };
        let (key, review) = (self.scope.key(), file.review.clone());
        self.session.update(cx, |s, _| {
            if s.reviewed.is_reviewed(key, &review) {
                s.reviewed.unmark(key, &review.path);
            } else {
                s.reviewed.mark(key, &review);
            }
        });
        cx.notify();
    }

    fn step(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let order = FileTree::new(&self.changed()).file_order();
        let current = self.files.get(self.current).map(|f| SharedString::from(f.review.path.clone()));
        if let Some(path) = step(&order, current.as_ref(), by) {
            self.open(&path, window, cx);
        }
    }

    /// Decides hunk `id` of the open file, writes the file, and gives the rows that closed; `None` when
    /// the file has no such hunk.
    fn decide_hunk(&mut self, id: &str, decision: Decision, window: &mut Window, cx: &mut Context<Self>) -> Option<std::ops::Range<usize>> {
        let at = self.current;
        let merged = self.files.get(at)?.merged.clone()?;
        let hunk = merged.hunks().iter().find(|h| h.id == id).cloned()?;
        let closed = hunk.closing(decision);
        let decided = merged.decide(id, decision)?;
        let file = &mut self.files[at];
        file.undo.push(merged);
        file.merged = Some(decided);
        inline_review::apply(&self.editor, &[(hunk, decision)], window, cx);
        self.write(&[at], cx);
        cx.notify();
        Some(closed)
    }

    /// Decides every hunk of the open file at once, writes it, and moves to the next file.
    fn decide_file(&mut self, decision: Decision, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.current;
        let Some(merged) = self.files.get(at).and_then(|f| f.merged.clone()) else { return self.step(1, window, cx) };
        let hunks = merged.hunks().to_vec();
        if !hunks.is_empty() {
            self.resolving.clear();
            let decided = decide_all(&merged, decision);
            let file = &mut self.files[at];
            file.undo.push(merged);
            file.merged = Some(decided);
            inline_review::apply(&self.editor, &whole_file(&hunks, decision), window, cx);
            self.write(&[at], cx);
        }
        self.step(1, window, cx);
        cx.notify();
    }

    /// Decides every hunk of every file, and writes them.
    fn decide_every_file(&mut self, decision: Decision, window: &mut Window, cx: &mut Context<Self>) {
        self.resolving.clear();
        let mut changed = Vec::new();
        for (i, file) in self.files.iter_mut().enumerate() {
            let Some(merged) = file.merged.clone() else { continue };
            if merged.hunks().is_empty() {
                continue;
            }
            if i == self.current {
                inline_review::apply(&self.editor, &whole_file(merged.hunks(), decision), window, cx);
            }
            file.merged = Some(decide_all(&merged, decision));
            file.undo.push(merged);
            changed.push(i);
        }
        self.write(&changed, cx);
        cx.notify();
    }

    /// Reads the files of `scope` from the session, and opens the one at the same path if it has it.
    fn set_scope(&mut self, scope: Scope, window: &mut Window, cx: &mut Context<Self>) {
        if self.writing.take().is_some() {
            self.write(&[self.current], cx);
        }
        let path = self.files.get(self.current).map(|f| f.review.path.clone());
        if let Scope::Turn(turn) = scope {
            self.turn = turn;
        }
        self.scope = scope;
        self.files = read_files(self.session.read(cx), scope);
        self.turns = self.session.read(cx).review.turns().len();
        self.current = path.and_then(|p| self.files.iter().position(|f| f.review.path == p)).unwrap_or(0);
        self.resolving.clear();
        self.composer = None;
        (self.editor, self._edits) = Self::editor_for(self.files.get(self.current), window, cx);
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
        focus_once_painted(composer.focus_handle(cx), 3, window, cx);
        self.composer = Some((self.current, composer, sub));
        cx.notify();
    }

    /// Adds the reader's comment on `row` of the open file to the session, for the next message.
    fn comment(&mut self, row: usize, body: impl Into<String>, cx: &mut Context<Self>) {
        let Some(file) = self.files.get(self.current) else { return };
        let Some(anchor) = file.merged.as_ref().and_then(|m| m.anchor(row..row + 1)) else { return };
        let (turn, path) = (self.turn, file.review.path.clone());
        self.session.update(cx, |s, cx| {
            s.comments.add(turn, path, anchor, body);
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
        let waiting = s.comments.for_file(path).map(|c| (c.clone(), None));
        let sent = s.sent_comments.iter().filter(|(c, _)| c.path == path).map(|(c, answered)| (c.clone(), Some(*answered)));
        for (comment, sent) in waiting.chain(sent).collect::<Vec<_>>() {
            let Some(row) = row_of(merged, comment.side, comment.last_line) else { continue };
            let time = match sent {
                None => "not sent yet",
                Some(false) => "sent",
                Some(true) => "answered",
            };
            let entry = Comment::new("You", time, comment.body.clone());
            let (session, id) = (self.session.clone(), comment.id);
            let this = cx.entity().downgrade();
            blocks.push(RowBlock {
                row,
                render: std::rc::Rc::new(move |_, _| {
                    let thread = LineComment::new(("comment", id as usize), vec![entry.clone()]).resolved(sent == Some(true));
                    match sent {
                        // Waiting: Resolve takes it back before it goes; Reply adds another on its row.
                        None => {
                            let (session, reply) = (session.clone(), this.clone());
                            thread
                                .on_resolve(move |_, _, cx| session.update(cx, |s, cx| {
                                    s.comments.remove(id);
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

/// Focuses `handle` now and again on each of the next `frames` frames until it holds: a composer in a
/// row block is painted only once the editor has laid it out, and a handle not yet painted loses the
/// focus the press gave it.
fn focus_once_painted(handle: FocusHandle, frames: usize, window: &mut Window, cx: &mut gpui_kit::App) {
    if handle.is_focused(window) {
        return;
    }
    handle.focus(window, cx);
    if frames > 0 {
        window.on_next_frame(move |window, cx| focus_once_painted(handle, frames - 1, window, cx));
    }
}

/// Every hunk of `merged` decided the same way.
fn decide_all(merged: &Merged, decision: Decision) -> Merged {
    merged.hunks().iter().fold(merged.clone(), |m, h| m.decide(&h.id, decision).unwrap_or(m))
}

/// The files of `scope`, with what the reader decided before where the session kept it.
pub fn read_files(session: &AgentSession, scope: Scope) -> Vec<PaneFile> {
    let reviews = match scope {
        Scope::Turn(turn) => session.review.turns().get(turn).map(|t| t.files().to_vec()).unwrap_or_default(),
        Scope::Whole => session.review.whole(),
    };
    reviews
        .into_iter()
        .map(|review| {
            let mut file = PaneFile::new(review);
            if let Some((merged, on_disk)) = session.decided.get(&(scope, file.review.path.clone())) {
                (file.merged, file.on_disk) = (merged.clone(), on_disk.clone());
            }
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
        let progress = self.progress(cx);
        let reviewed = self.reviewed(cx);
        let scope = match self.scope {
            Scope::Turn(_) => 0,
            Scope::Whole => 1,
        };
        let bar = ReviewBar::new("review-bar", progress, handlers.clone())
            .review_mode(self.review_mode)
            .scopes(["This turn".into(), "Whole session".into()], scope);

        let Some(file) = self.files.get(self.current) else {
            let empty = div()
                .flex()
                .flex_col()
                .size_full()
                .gap(px(8.))
                .child(bar)
                .child(div().flex().flex_1().items_center().justify_center().text_size(TextSize::Sm.font_size()).text_color(theme.muted_foreground).child("Nothing changed"));
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
        let header = ReviewFileHeader::new("review-file", path.clone(), added, removed, handlers.clone());
        let file_card = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme.card)
            .rounded(radius::LG)
            .p(px(6.))
            .child(header)
            .children(what.map(|w| div().px(px(10.)).pb(px(6.)).text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(w)))
            .child(div().flex_1().min_h_0().child(review));
        let show_tree = self.review_mode || self.width >= TREE_FROM;
        let tree = show_tree.then(|| {
            div().flex_none().w(px(220.)).h_full().bg(theme.card).rounded(radius::LG).p(px(6.)).child(
                ChangedFileTree::new("review-tree", self.changed()).reviewed(reviewed).current(path).on_open(move |path, window, cx| open(path, window, cx)),
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
