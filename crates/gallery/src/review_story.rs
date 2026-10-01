//! The Review story: the review bar, the changed file tree, and the real editor with the inline review's
//! hunks, one open comment thread and one comment being written, over four fixture files.
//!
//! Pressing a file in the tree, or the file keys, loads that file's text and hunks; each file keeps its
//! own edits while another is open. Accept file accepts every hunk of the open file and moves on. A file
//! with no hunks left counts as reviewed. Put all back restores every file.

use std::{collections::HashSet, time::Instant};

use beui::{
    ActiveTheme, ChangedFile, ChangedFileTree, Comment, Decision, InlineHunk, InlineReview, LineComment, LineComposer,
    LineComposerEvent, ReviewBar, ReviewFileHeader, ReviewHandlers, ReviewProgress,
    file_tree::FileTree,
    inline_review::{self, DecisionHistory},
    review::{step, whole_file},
    theme::radius,
};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, anchored, deferred, point,
    base::input::{InputEvent, RowBlock},
    component::input::EditorState,
    div, prelude::FluentBuilder, px,
};

/// One changed file: its path, its buffer with both sides of every hunk, and the hunks in it.
struct Fixture {
    path: &'static str,
    text: &'static str,
    hunks: fn() -> Vec<InlineHunk>,
}

const FILE_DIFF: &str = "pub fn hunk_starts(header: &str) -> Option<usize> {
    let plus = header.split('+').nth(1)?;
    let start = plus.split(',').next()?;
    start.parse().ok()
    start.trim().parse().ok()
}

pub fn line_numbers(lines: &[DiffLine]) -> Vec<usize> {
    let mut n = 0;
    let mut n = 1;
    lines.iter().map(|_| { n += 1; n - 1 }).collect()
}
";

const FILE_DIFF_TESTS: &str = "use super::*;

#[test]
fn a_hunk_at_line_one_starts_at_one() {
    assert_eq!(hunk_starts(\"@@ -1,1 +1,1 @@\"), Some(1));
}

#[test]
fn numbers_count_from_the_hunk_start() {
    let lines = DiffLine::parse(\"@@ -3,2 +3,2 @@\\n a\\n b\");
    assert_eq!(line_numbers(&lines), vec![3, 4]);
}
";

const THEME: &str = "pub fn diff_color(&self, added: bool) -> Hsla {
    if added { self.success } else { self.danger }
}

pub fn diff_line(&self, added: bool) -> Hsla {
    self.diff_color(added).opacity(0.07)
    self.diff_color(added).opacity(0.18)
}
";

const MAIN: &str = "fn diffs() -> impl IntoElement {
    let lines = DiffLine::parse(DIFF);
    let (added, removed) = diff_stats(&lines);
    narrow(FileDiff::new(\"diff\", \"file_diff.rs\", lines))
}
";

const FIXTURES: [Fixture; 4] = [
    Fixture {
        path: "crates/beui/src/file_diff.rs",
        text: FILE_DIFF,
        hunks: || vec![InlineHunk::new("fd-1", 3..4, 4..5), InlineHunk::new("fd-2", 8..9, 9..10)],
    },
    Fixture { path: "crates/beui/src/file_diff/tests.rs", text: FILE_DIFF_TESTS, hunks: || vec![InlineHunk::new("fdt-1", 7..7, 7..12)] },
    Fixture { path: "crates/beui/src/theme.rs", text: THEME, hunks: || vec![InlineHunk::new("th-1", 5..6, 6..7)] },
    Fixture { path: "crates/gallery/src/main.rs", text: MAIN, hunks: || vec![InlineHunk::new("mn-1", 2..3, 3..3)] },
];

/// A file as the review holds it now.
struct FileState {
    path: SharedString,
    text: String,
    hunks: Vec<InlineHunk>,
    /// The file's size of change, which stays as it was when the review opened.
    added: usize,
    removed: usize,
}

impl FileState {
    fn open(f: &Fixture) -> Self {
        let hunks = (f.hunks)();
        let added = hunks.iter().map(|h| h.added.len()).sum();
        let removed = hunks.iter().map(|h| h.removed.len()).sum();
        Self { path: f.path.into(), text: f.text.to_string(), hunks, added, removed }
    }
}

/// A thread on one row of one file.
struct Thread {
    file: usize,
    row: usize,
    comments: Vec<Comment>,
}

pub struct ReviewStory {
    /// The scope the bar's switch shows: 0 this turn, 1 the whole session.
    scope: usize,
    editor: Entity<EditorState>,
    /// The pane's own focus: Escape in the editor moves here, so the letters work.
    focus: FocusHandle,
    /// Files the reader marked seen with `x`, whatever their hunks.
    marked: HashSet<SharedString>,
    /// `r`: the review on the whole window.
    review_mode: bool,
    files: Vec<FileState>,
    current: usize,
    resolving: Vec<beui::Resolve>,
    history: DecisionHistory,
    threads: Vec<Thread>,
    composer: Option<(usize, Entity<LineComposer>, Subscription)>,
    _edits: Subscription,
}

impl ReviewStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let files: Vec<FileState> = FIXTURES.iter().map(FileState::open).collect();
        let editor = beui::CodeEditor::state(files[0].path.as_ref(), files[0].text.clone(), window, cx);
        // The user can type anywhere, so the hunks follow the rows they describe.
        let _edits = cx.subscribe(&editor, |this, state, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let text = state.read(cx).value().to_string();
            let file = &mut this.files[this.current];
            if text != file.text {
                file.hunks = match this.history.hunks_for(&text) {
                    Some(hunks) => hunks,
                    None => inline_review::track_edit(&file.hunks, &file.text, &text),
                };
                file.text = text;
                cx.notify();
            }
        });
        let mut story = Self {
            editor,
            focus: cx.focus_handle(),
            marked: HashSet::new(),
            review_mode: false,
            scope: 0,
            files,
            current: 0,
            resolving: Vec::new(),
            history: DecisionHistory::default(),
            threads: vec![Thread {
                file: 0,
                row: 5,
                comments: vec![
                    Comment::new("Maya", "12m ago", "Does this still hold when the header has **no count**, as in `@@ -3 +3 @@`?"),
                    Comment::new("Claude", "10m ago", "Yes: `split(',')` returns the whole start when there is no comma."),
                ],
            }],
            composer: None,
            _edits,
        };
        story.open_composer(10, window, cx);
        story
    }

    fn open_composer(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let composer = cx.new(|cx| LineComposer::new(row, window, cx));
        let sub = cx.subscribe(&composer, |this, _, event: &LineComposerEvent, cx| {
            if let LineComposerEvent::Submit { row, text } = event {
                this.threads.push(Thread { file: this.current, row: *row, comments: vec![Comment::new("You", "now", text.clone())] });
            }
            this.composer = None;
            cx.notify();
        });
        composer.focus_handle(cx).focus(window, cx);
        self.composer = Some((self.current, composer, sub));
        cx.notify();
    }

    fn changed(&self) -> Vec<ChangedFile> {
        self.files.iter().map(|f| ChangedFile::new(f.path.clone(), f.added, f.removed)).collect()
    }

    /// A file is reviewed when no hunk is left in it, or when the reader marked it.
    fn reviewed(&self) -> HashSet<SharedString> {
        self.files.iter().filter(|f| f.hunks.is_empty() || self.marked.contains(&f.path)).map(|f| f.path.clone()).collect()
    }

    fn toggle_mark(&mut self, cx: &mut Context<Self>) {
        let path = self.files[self.current].path.clone();
        if !self.marked.remove(&path) {
            self.marked.insert(path);
        }
        cx.notify();
    }

    fn progress(&self) -> ReviewProgress {
        ReviewProgress {
            files: self.files.len(),
            reviewed: self.reviewed().len(),
            added: self.files.iter().map(|f| f.added).sum(),
            removed: self.files.iter().map(|f| f.removed).sum(),
        }
    }

    /// Loads another file into the editor. Its text is set first, so the edit it makes is not taken
    /// for the user's.
    fn open(&mut self, path: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = self.files.iter().position(|f| &f.path == path) else { return };
        if at == self.current {
            return;
        }
        self.current = at;
        self.resolving.clear();
        let text = self.files[at].text.clone();
        self.editor.update(cx, |state, cx| state.set_value(text, window, cx));
        cx.notify();
    }

    fn step(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let order = FileTree::new(&self.changed()).file_order();
        if let Some(path) = step(&order, Some(&self.files[self.current].path), by) {
            self.open(&path, window, cx);
        }
    }

    /// Decides every hunk of the open file at once, then moves to the next file.
    fn decide_file(&mut self, decision: Decision, window: &mut Window, cx: &mut Context<Self>) {
        let hunks = std::mem::take(&mut self.files[self.current].hunks);
        if !hunks.is_empty() {
            let before = (self.files[self.current].text.clone(), hunks.clone());
            inline_review::apply(&self.editor, &whole_file(&hunks, decision), window, cx);
            let file = &mut self.files[self.current];
            file.text = self.editor.read(cx).value().to_string();
            self.history.record(before, (file.text.clone(), Vec::new()));
        }
        self.step(1, window, cx);
        cx.notify();
    }

    fn put_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.files = FIXTURES.iter().map(FileState::open).collect();
        self.marked.clear();
        self.resolving.clear();
        let text = self.files[self.current].text.clone();
        self.editor.update(cx, |state, cx| state.set_value(text, window, cx));
        cx.notify();
    }

    fn handlers(&self, cx: &mut Context<Self>) -> ReviewHandlers {
        let this = cx.entity().downgrade();
        let with = move |f: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let this = this.clone();
            move |window: &mut Window, cx: &mut gpui_kit::App| {
                this.update(cx, |story, cx| f(story, window, cx)).ok();
            }
        };
        ReviewHandlers::default()
            .on_next(with(|s, w, cx| s.step(1, w, cx)))
            .on_previous(with(|s, w, cx| s.step(-1, w, cx)))
            .on_accept_file(with(|s, w, cx| s.decide_file(Decision::Accept, w, cx)))
            .on_reject_file(with(|s, w, cx| s.decide_file(Decision::Reject, w, cx)))
            .on_put_back(with(|s, w, cx| s.put_back(w, cx)))
            .on_mark(with(|s, _, cx| s.toggle_mark(cx)))
            .on_review_mode(with(|s, _, cx| {
                s.review_mode = !s.review_mode;
                cx.notify();
            }))
            .on_dismiss(with(|s, _, cx| {
                s.review_mode = false;
                cx.notify();
            }))
    }

    /// The threads and the composer on the open file, as blocks under their rows.
    fn blocks(&self, cx: &mut Context<Self>) -> Vec<RowBlock> {
        let mut blocks: Vec<RowBlock> = Vec::new();
        for (i, thread) in self.threads.iter().enumerate().filter(|(_, t)| t.file == self.current) {
            let comments = thread.comments.clone();
            let row = thread.row;
            let this = cx.entity().downgrade();
            let reply_to = this.clone();
            blocks.push(RowBlock {
                row,
                render: std::rc::Rc::new(move |_, _| {
                    let (resolve, reply) = (this.clone(), reply_to.clone());
                    LineComment::new(("thread", i), comments.clone())
                        .on_reply(move |_, window, cx| {
                            reply.update(cx, |s, cx| s.open_composer(row, window, cx)).ok();
                        })
                        .on_resolve(move |_, _, cx| {
                            resolve.update(cx, |s, cx| {
                                s.threads.retain(|t| !(t.file == s.current && t.row == row));
                                cx.notify();
                            })
                            .ok();
                        })
                        .into_any_element()
                }),
            });
        }
        if let Some((file, composer, _)) = &self.composer
            && *file == self.current
        {
            let (composer, row) = (composer.clone(), composer.read(cx).row());
            blocks.push(RowBlock { row, render: std::rc::Rc::new(move |_, _| composer.clone().into_any_element()) });
        }
        blocks
    }
}

impl Render for ReviewStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let handlers = self.handlers(cx);
        let file = &self.files[self.current];
        let (path, hunks) = (file.path.clone(), file.hunks.clone());

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
            let current = this.current;
            let Some(hunk) = this.files[current].hunks.iter().find(|h| &h.id == id).cloned() else {
                this.resolving.remove(at);
                return;
            };
            let closed = hunk.closing(*decision);
            this.resolving[at].edited(closed.clone());
            let before = (this.files[current].text.clone(), this.files[current].hunks.clone());
            inline_review::apply(&this.editor, &[(hunk, *decision)], window, cx);
            let file = &mut this.files[current];
            file.hunks = inline_review::shift_after(&file.hunks, id, &closed);
            file.text = this.editor.read(cx).value().to_string();
            this.history.record(before, (file.text.clone(), file.hunks.clone()));
            cx.notify();
        });
        // Pressing a file opens it and hands focus back to the editor.
        let open = cx.listener(|this, path: &SharedString, window, cx| {
            this.open(path, window, cx);
            this.editor.update(cx, |state, cx| state.focus(window, cx));
        });
        let add = cx.listener(|this, row: &usize, window, cx| this.open_composer(*row, window, cx));
        let blocks = self.blocks(cx);

        let tree = ChangedFileTree::new("review-tree", self.changed())
            .reviewed(self.reviewed())
            .current(path)
            .on_open(move |path, window, cx| open(path, window, cx));
        let body = if self.review_mode { f32::from(window.viewport_size().height) - 32. - 52. } else { 560. };
        let review = InlineReview::new("review-editor", &self.editor, hunks.clone())
            .on_card(true)
            .height(px(body - 12. - 40.))
            .when_some(hunks.first(), |r, h| r.current(h.id.clone()))
            .resolving(self.resolving.clone())
            .row_blocks(blocks)
            .on_add_comment(move |row, window, cx| add(&row, window, cx))
            .on_decide(move |id, decision, window, cx| decide(&(id.clone(), decision), window, cx))
            .on_resolved(move |id, decision, window, cx| resolved(&(id.clone(), decision), window, cx));

        // The tree and the diff each sit in the same card, under the bar.
        let card = || div().h(px(body)).bg(theme.card).rounded(radius::lg()).p(px(6.));
        let file = &self.files[self.current];
        let header = ReviewFileHeader::new("review-file", file.path.clone(), file.added, file.removed, handlers.clone());
        // The story's width, less the gallery's sidebar (220) and its padding (2 x 40), so the bar fits
        // to the pane rather than the pane growing to the bar. Review mode takes the window, less 32.
        let viewport = window.viewport_size().width;
        let width = if self.review_mode { viewport - px(32.) } else { viewport - px(300.) };
        let pane = div()
            .id("review-pane")
            .flex()
            .flex_col()
            .gap(px(8.))
            .w(width)
            .min_w_0()
            .overflow_hidden()
            .child({
                let this = cx.entity().downgrade();
                let mut with_scopes = handlers.clone();
                with_scopes.on_switch_scope = Some(std::rc::Rc::new(move |_, cx| {
                    this.update(cx, |s, cx| {
                        s.scope = 1 - s.scope;
                        cx.notify();
                    })
                    .ok();
                }));
                ReviewBar::new("review-bar", self.progress(), with_scopes)
                    .scopes([("This turn".into(), "Turn".into()), ("Whole session".into(), "Session".into())], self.scope)
                    .review_mode(self.review_mode)
            })
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(card().flex_none().w(px(220.)).child(tree))
                    // The file's own card: its head stays put while the text under it scrolls.
                    .child(card().flex_1().min_w_0().flex().flex_col().child(header).child(review)),
            );
        let pane = handlers.keys(pane, &self.focus);
        if !self.review_mode {
            return pane.into_any_element();
        }
        // Review mode: the files on the whole window, over everything else, until `r` or Escape.
        let size = window.viewport_size();
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div().w(size.width).h(size.height).p(px(16.)).bg(theme.background).child(pane),
            ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

/// So the gallery can hold the story as any other element.
pub fn element(story: &Entity<ReviewStory>) -> AnyElement {
    story.clone().into_any_element()
}
