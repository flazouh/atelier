use std::{collections::HashSet, path::PathBuf, rc::Rc};

use atelier_ui::{
    ActiveTheme, ChangedFile, ChangedFileTree, ChecksPanel, Comment, CommentComposer,
    CommentComposerEvent, CommitsSummary, ConversationList, Filter, Finder, FinderEvent,
    FinderItem, InlineReview, LineComment, ReviewBar, ReviewFileHeader, ReviewHandlers,
    ReviewProgress, RowMap, UnsentComments, VerdictBox, VerdictEvent, file_tree::FileTree,
    keys::Command, review::step, theme::radius, typography::TextSize,
};
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
    Window, base::input::RowBlock, component::input::EditorState, div, prelude::FluentBuilder,
    px,
};
use atelier_editor::{EditorSession, Elsewhere, Jump};
use atelier_lsp::{LspError, canonical, client::uri_to_path};
use lsp_types::Position;

use crate::{pr_fixture::Fixture, workers::workers};
use super::types::{COMMITS, PADDING, PR_BODY, RAIL_GAP, RAIL_MAX, STATUS_HEIGHT, THREAD_ROW, TREE};
use super::helpers::{checks, commits, conversation, fit};

/// What the file card shows: a changed file, or one brought in to read beside them.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Place {
    /// Relative to the repository.
    pub(super) path: String,
    brought_in: bool,
}

/// Where a row of a lookup leads: a file and a place in it, in that file's own rows, or in the shown
/// rows when it is the file on screen.
#[derive(Clone, Debug)]
struct Lead {
    pub(super) path: PathBuf,
    position: Position,
    shown_rows: bool,
}

/// A lookup open over the diff, and where each of its rows leads.
struct Lookup {
    command: Command,
    pub(super) finder: Entity<Finder>,
    leads: Vec<Lead>,
    asking: Task<()>,
    _events: Subscription,
}

pub struct PrStory {
    fixture: Fixture,
    editor: Entity<EditorState>,
    pub(super) place: Place,
    /// Where Escape and Previous go back to, newest last, with the caret there.
    pub(super) back: Vec<(Place, Position)>,
    session: Option<Entity<EditorSession>>,
    lookup: Option<Lookup>,
    /// The repository's files, listed once on a background thread; `None` until then.
    pub(super) files: Option<Rc<Vec<String>>>,
    _listing: Task<()>,
    pub(super) seen: HashSet<SharedString>,
    pub(super) focus: FocusHandle,
    details: bool,
    /// The tree as ⌘⇧B last left it; `None` lets the width decide.
    pub(super) tree: Option<bool>,
    /// The pane's width in the last frame.
    pub(super) width: f32,
    review_mode: bool,
    pub(super) unsent: usize,
    composer: Entity<CommentComposer>,
    pub(super) verdict: Entity<VerdictBox>,
    pub(super) merge: Entity<atelier_ui::MergeBox>,
    _subscriptions: Vec<Subscription>,
    _session: Option<Subscription>,
}

impl PrStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let fixture = Fixture::write();
        let first = &fixture.changed[0];
        let editor = atelier_ui::CodeEditor::state(first.path, first.text.as_str(), window, cx);
        let composer = cx.new(|cx| {
            let mut c = CommentComposer::new("On this pull request", "You", window, cx);
            c.set_text("The abort path reads right to me. Before I approve: is the HEAD case dperrault asked about covered anywhere, or does that want its own test?", window, cx);
            c.open(window, cx);
            c
        });
        let verdict = cx.new(|cx| VerdictBox::new("f4a97b1c9e2d4f0a", false, window, cx));
        let merge = cx.new(|cx| {
            // As the checks panel shows: linux-x64 is a required check, and it failed.
            let facts = atelier_ui::merge::MergeFacts { checks_failing: 1, ..crate::merge_story::states().remove(0).1 };
            let choice = atelier_ui::merge::first_choice(&facts, None);
            atelier_ui::MergeBox::new(facts, choice, COMMITS[0], PR_BODY, window, cx)
        });
        let subs = vec![
            cx.subscribe_in(&merge, window, |_, merge, event: &atelier_ui::MergeBoxEvent, _, cx| {
                println!("merge: {event:?}");
                use atelier_ui::merge::Action;
                let atelier_ui::MergeBoxEvent::Act { action, choice, .. } = event else { return };
                let deleted = match action {
                    Action::Merge(_) | Action::BypassAndMerge(_) => choice.delete_branch,
                    Action::DeleteBranch => true,
                    _ => return,
                };
                merge.update(cx, |b, cx| b.merged(deleted, cx));
            }),
            cx.subscribe(&composer, |_, _, event: &CommentComposerEvent, _| println!("comment: {event:?}")),
            cx.subscribe_in(&verdict, window, |_, verdict, event: &VerdictEvent, window, cx| {
                println!("verdict: {event:?}");
                verdict.update(cx, |v, cx| v.sent(window, cx));
            }),
        ];
        let mut seen = HashSet::new();
        seen.insert(SharedString::from("tests/abort.rs"));
        let place = Place { path: first.path.to_string(), brought_in: false };
        let listing = cx.background_spawn({
            let root = fixture.root.clone();
            async move { crate::pr_fixture::list_files(&root) }
        });
        let _listing = cx.spawn(async move |story, cx| {
            let files = listing.await;
            story
                .update(cx, |story, cx| {
                    story.files = Some(Rc::new(files));
                    if story.lookup.as_ref().is_some_and(|l| l.command == Command::GoToFile) {
                        story.fill_files(cx);
                    }
                })
                .ok();
        });
        let mut story = Self {
            fixture,
            editor,
            place: place.clone(),
            back: Vec::new(),
            session: None,
            lookup: None,
            files: None,
            _listing,
            seen,
            focus: cx.focus_handle(),
            details: true,
            tree: None,
            width: f32::MAX,
            review_mode: false,
            unsent: 2,
            composer,
            verdict,
            merge,
            _subscriptions: subs,
            _session: None,
        };
        story.show(place, None, window, cx);
        story
    }

    pub(super) fn changed(&self) -> Vec<ChangedFile> {
        self.fixture.changed.iter().map(|f| ChangedFile::new(f.path, f.added(), f.removed())).collect()
    }

    /// The rows the card shows over the file on screen.
    pub(super) fn rows(&self) -> RowMap {
        match self.fixture.changed_at(&self.place.path) {
            Some(i) if !self.place.brought_in => self.fixture.changed[i].rows.clone(),
            _ => RowMap::default(),
        }
    }

    fn path_on_disk(&self) -> PathBuf {
        self.fixture.root.join(&self.place.path)
    }

    /// Puts `place` in the card, the caret at `position` in shown rows, and a language server on it.
    fn show(&mut self, place: Place, position: Option<Position>, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.fixture.changed_at(&place.path).filter(|_| !place.brought_in);
        let (text, rows) = match changed {
            Some(i) => (self.fixture.changed[i].text.clone(), self.fixture.changed[i].rows.clone()),
            None => {
                let text = std::fs::read_to_string(self.fixture.root.join(&place.path)).unwrap_or_default();
                (text.trim_end_matches('\n').to_string(), RowMap::default())
            }
        };
        self.editor.update(cx, |state, cx| {
            // The last file's server must not answer for this one while the next one starts.
            let lsp = state.lsp_mut();
            lsp.definition_provider = None;
            lsp.hover_provider = None;
            lsp.show_document = None;
            // A hover card belongs to the text it was asked about.
            state.clear_hover_state(cx);
            state.set_value(text, window, cx);
            if let Some(position) = position {
                state.set_cursor_position(position, window, cx);
            }
        });
        atelier_ui::code_editor::set_diagnostics(&self.editor, Vec::new(), cx);
        self.place = place;
        let this = cx.entity().downgrade();
        // gpui-base asks while it updates the editor, and a jump replaces the editor's text, so it waits
        // until that update is over.
        let elsewhere: Elsewhere = Rc::new(move |jump: Jump, window: &mut Window, cx: &mut gpui_kit::App| {
            let this = this.clone();
            window.defer(cx, move |window, cx| {
                this.update(cx, |story, cx| story.jump(jump, window, cx)).ok();
            });
        });
        let (editor, path) = (self.editor.clone(), self.path_on_disk());
        let session = cx.new(|cx| EditorSession::for_review(workers(), editor, path, rows, Some(elsewhere), cx));
        self._session = Some(cx.observe_in(&session, window, |story, session, window, cx| {
            // A ⌘-click on a declaration lists its uses: in this view, as the Uses lookup.
            let uses = session.read(cx).references().to_vec();
            if !uses.is_empty() {
                session.update(cx, |s, cx| s.close_references(cx));
                story.open_finder(Command::Uses, window, cx);
                story.fill_uses(uses, cx);
            }
            cx.notify();
        }));
        self.session = Some(session);
        cx.notify();
    }

    /// Opens a changed file from the tree or the keys: a fresh start, so the way back is cleared.
    pub(super) fn open(&mut self, path: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if self.place.path == path.as_ref() && !self.place.brought_in {
            return;
        }
        self.back.clear();
        self.show(Place { path: path.to_string(), brought_in: false }, None, window, cx);
    }

    /// Follows a jump out of the file on screen: to another changed file, or Brought In.
    fn jump(&mut self, jump: Jump, window: &mut Window, cx: &mut Context<Self>) {
        let Some(relative) = self.fixture.relative(&canonical(&jump.path)) else {
            // The toolchain's own sources, say: named, not opened.
            let name = jump.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let line = jump.position.line + 1;
            if let Some(session) = &self.session {
                session.update(cx, |s, cx| s.say(format!("defined in {name} on line {line}, outside this repository"), cx));
            }
            return;
        };
        let caret = self.editor.read(cx).cursor_position();
        self.back.push((self.place.clone(), caret));
        let (place, position) = match self.fixture.changed_at(&relative) {
            Some(i) => {
                let line = self.fixture.changed[i].rows.to_view(jump.position.line as usize) as u32;
                (Place { path: relative, brought_in: false }, Position { line, ..jump.position })
            }
            None => (Place { path: relative, brought_in: true }, jump.position),
        };
        self.show(place, Some(position), window, cx);
    }

    fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((place, caret)) = self.back.pop() {
            self.show(place, Some(caret), window, cx);
        }
    }

    pub(super) fn step(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let order = FileTree::new(&self.changed()).file_order();
        // From a file brought in, the walk goes on from the changed file it was reached from.
        let from = std::iter::once(&self.place)
            .chain(self.back.iter().rev().map(|(p, _)| p))
            .find(|p| !p.brought_in)
            .map(|p| SharedString::from(p.path.clone()));
        if let Some(path) = step(&order, from.as_ref(), by) {
            self.open(&path, window, cx);
        }
    }

    /// Opens the lookup for `command` and asks for its rows. Uses asks about the name under the
    /// pointer when the pointer is over the text, else the one at the caret.
    fn open_lookup(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let pointer = self.editor.read(cx).offset_at_pointer(window);
        self.open_finder(command, window, cx);
        self.ask(command, pointer, cx);
        cx.notify();
    }

    /// Opens the lookup for `command`, empty.
    fn open_finder(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let (title, placeholder, filter) = match command {
            Command::Uses => ("Uses", "Filter the uses", Filter::Here),
            Command::FileNames => ("Names in this file", "Filter the names", Filter::Here),
            Command::GoToName => ("Go to name", "Part of a name", Filter::Owner),
            _ => ("Go to file", "Part of a path", Filter::Here),
        };
        let finder = cx.new(|cx| Finder::new(title, placeholder, filter, window, cx).command(command));
        let events = cx.subscribe_in(&finder, window, |story, _, event: &FinderEvent, window, cx| match event {
            FinderEvent::Query(query) => {
                if story.lookup.as_ref().is_some_and(|l| l.command == Command::GoToName) {
                    story.ask_names(query.clone(), cx);
                }
            }
            FinderEvent::Pick(at) => story.pick(*at, window, cx),
            FinderEvent::Dismiss => story.close_lookup(window, cx),
        });
        finder.read(cx).focus_handle(cx).focus(window, cx);
        self.lookup = Some(Lookup { command, finder, leads: Vec::new(), asking: Task::ready(()), _events: events });
        cx.notify();
    }

    /// Fills the open lookup for `command`: from the server, or from the repository's files.
    fn ask(&mut self, command: Command, pointer: Option<usize>, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else { return self.note("The language server is not ready", cx) };
        match command {
            Command::Uses => match session.read(cx).uses(pointer, cx) {
                Some(task) => self.wait(task, "Asking for the uses", |story, uses, cx| story.fill_uses(uses, cx), cx),
                None => self.note("Point at a name in the new side, or put the caret on one", cx),
            },
            Command::FileNames => match session.read(cx).names(cx) {
                Some(task) => self.wait(task, "Asking for the names", |story, names, cx| story.fill_names(names, false, cx), cx),
                None => self.note("The language server is not ready", cx),
            },
            Command::GoToName => self.note("Type part of a name", cx),
            _ => self.fill_files(cx),
        }
    }

    /// Go to file's rows: the repository's files, once the background listing has them.
    fn fill_files(&mut self, cx: &mut Context<Self>) {
        let Some(files) = self.files.clone() else { return self.note("Loading files", cx) };
        let leads = files
            .iter()
            .map(|f| Lead { path: self.fixture.root.join(f), position: Position::default(), shown_rows: false })
            .collect();
        let items = files.iter().map(|f| FinderItem::new(f.clone(), self.file_note(f)).icon(f.clone())).collect();
        self.set_rows(items, leads, cx);
    }

    /// "changed" for a file the pull request changed, nothing for the rest.
    fn file_note(&self, relative: &str) -> &'static str {
        if self.fixture.changed_at(relative).is_some() { "changed" } else { "" }
    }

    fn note(&mut self, note: &'static str, cx: &mut Context<Self>) {
        if let Some(lookup) = &self.lookup {
            lookup.finder.update(cx, |f, cx| f.set_note(note, cx));
        }
    }

    fn set_rows(&mut self, items: Vec<FinderItem>, leads: Vec<Lead>, cx: &mut Context<Self>) {
        if let Some(lookup) = &mut self.lookup {
            lookup.leads = leads;
            lookup.finder.update(cx, |f, cx| f.set_items(items, cx));
        }
    }

    /// Waits for a server's answer off the UI thread; the lookup says `asking` meanwhile.
    fn wait<T: 'static>(
        &mut self,
        task: Task<Result<T, LspError>>,
        asking: &'static str,
        then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.note(asking, cx);
        let waiting = cx.spawn(async move |story, cx| {
            let answer = task.await;
            story
                .update(cx, |story, cx| match answer {
                    Ok(found) => then(story, found, cx),
                    Err(LspError::Superseded) => {}
                    Err(error) => {
                        let text = format!("{error}");
                        if let Some(lookup) = &story.lookup {
                            lookup.finder.update(cx, |f, cx| f.set_note(text, cx));
                        }
                    }
                })
                .ok();
        });
        if let Some(lookup) = &mut self.lookup {
            lookup.asking = waiting;
        }
    }

    /// A place's line for a row: the file's own line, counted from 1.
    fn line_of(&self, lead: &Lead) -> usize {
        let line = lead.position.line as usize;
        let line = if lead.shown_rows { self.rows().to_head(line).unwrap_or(line) } else { line };
        line + 1
    }

    fn lead(&self, path: Option<PathBuf>, position: Position) -> Option<Lead> {
        let path = path?;
        let shown_rows = path == self.path_on_disk();
        Some(Lead { path, position, shown_rows })
    }

    fn fill_uses(&mut self, uses: Vec<atelier_lsp::Target>, cx: &mut Context<Self>) {
        let mut items = Vec::new();
        let mut leads = Vec::new();
        for target in uses {
            let Some(lead) = self.lead(target.path.clone(), target.range.start) else { continue };
            let relative = self.fixture.relative(&lead.path).unwrap_or_default();
            items.push(FinderItem::new(target.line_text.clone(), format!("{relative}:{}", self.line_of(&lead))).icon(relative));
            leads.push(lead);
        }
        if items.is_empty() {
            self.note("No uses", cx);
        }
        self.set_rows(items, leads, cx);
    }

    fn fill_names(&mut self, names: Vec<atelier_lsp::Symbol>, with_file: bool, cx: &mut Context<Self>) {
        let mut items = Vec::new();
        let mut leads = Vec::new();
        for symbol in names {
            let path = uri_to_path(&symbol.uri).map(|p| canonical(&p));
            let Some(lead) = self.lead(path, symbol.range.start) else { continue };
            let relative = self.fixture.relative(&lead.path).unwrap_or_default();
            let line = self.line_of(&lead);
            let detail = match (with_file, symbol.container.is_empty()) {
                (true, _) => format!("{relative}:{line}"),
                (false, true) => format!("line {line}"),
                (false, false) => format!("{}, line {line}", symbol.container),
            };
            let item = FinderItem::new(symbol.name.clone(), detail);
            items.push(if with_file { item.icon(relative) } else { item });
            leads.push(lead);
        }
        if items.is_empty() {
            self.note("No names", cx);
        }
        self.set_rows(items, leads, cx);
    }

    fn ask_names(&mut self, query: SharedString, cx: &mut Context<Self>) {
        if query.trim().is_empty() {
            self.set_rows(Vec::new(), Vec::new(), cx);
            return self.note("Type part of a name", cx);
        }
        let Some(task) = self.session.as_ref().and_then(|s| s.read(cx).project_names(&query, cx)) else { return };
        self.wait(task, "Asking for the names", |story, names, cx| story.fill_names(names, true, cx), cx);
    }

    fn pick(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lead) = self.lookup.as_ref().and_then(|l| l.leads.get(at).cloned()) else { return };
        self.close_lookup(window, cx);
        if lead.path == self.path_on_disk() {
            let position = if lead.shown_rows {
                lead.position
            } else {
                Position { line: self.rows().to_view(lead.position.line as usize) as u32, ..lead.position }
            };
            self.editor.update(cx, |state, cx| state.set_cursor_position(position, window, cx));
        } else {
            self.jump(Jump { path: lead.path, position: lead.position }, window, cx);
        }
    }

    fn close_lookup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.lookup = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn handlers(&self, cx: &mut Context<Self>) -> ReviewHandlers {
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
                if s.back.is_empty() {
                    s.step(-1, w, cx)
                } else {
                    s.go_back(w, cx)
                }
            }))
            .on_mark(with(|s, _, cx| {
                if s.place.brought_in {
                    return;
                }
                let path = SharedString::from(s.place.path.clone());
                if !s.seen.remove(&path) {
                    s.seen.insert(path);
                }
                cx.notify();
            }))
            .on_review_mode(with(|s, _, cx| {
                s.review_mode = !s.review_mode;
                cx.notify();
            }))
            .on_dismiss(with(|s, w, cx| {
                if s.lookup.is_some() {
                    s.close_lookup(w, cx);
                } else if !s.back.is_empty() {
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
                let shown = s.fit().tree;
                s.tree = Some(!shown);
                cx.notify();
            }))
            .on_put_back(with(|s, _, cx| {
                s.seen.clear();
                cx.notify();
            }))
            .on_uses(with(|s, w, cx| s.open_lookup(Command::Uses, w, cx)))
            .on_file_names(with(|s, w, cx| s.open_lookup(Command::FileNames, w, cx)))
            .on_go_to_name(with(|s, w, cx| s.open_lookup(Command::GoToName, w, cx)))
            .on_go_to_file(with(|s, w, cx| s.open_lookup(Command::GoToFile, w, cx)))
    }
}

/// How the pane splits its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// The rail's width, or `None` when it is folded.
    pub rail: Option<f32>,
    pub tree: bool,
}

impl PrStory {
    pub(super) fn fit(&self) -> Fit {
        fit(self.width, self.details && !self.review_mode, self.tree)
    }
}

impl Render for PrStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let handlers = self.handlers(cx);
        let changed = self.changed();
        let shown = self.fixture.changed_at(&self.place.path).filter(|_| !self.place.brought_in);
        let progress = ReviewProgress {
            files: changed.len(),
            reviewed: self.seen.len(),
            added: changed.iter().map(|f| f.added).sum(),
            removed: changed.iter().map(|f| f.removed).sum(),
        };
        let height = f32::from(window.viewport_size().height);
        let body = height - 16. - 52.;
        let (threads, remarks) = conversation();
        let unsent = self.unsent;
        let layout = self.fit();

        let rail = div()
            .id("pr-rail")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(layout.rail.unwrap_or(RAIL_MAX)))
            .h(px(height - 16.))
            .gap(px(8.))
            .overflow_y_scroll()
            .child(UnsentComments::new("pr-unsent", unsent).on_send({
                let this = cx.entity().downgrade();
                move |_, cx| {
                    this.update(cx, |s, cx| {
                        s.unsent = 0;
                        cx.notify();
                    })
                    .ok();
                }
            }))
            .child(ChecksPanel::new("pr-checks", checks()))
            .child(div().flex().flex_col().flex_none().rounded(radius::lg()).bg(theme.card).child(ConversationList::new("pr-conversation", threads, remarks)).child(self.composer.clone()))
            .child(self.verdict.clone())
            .child(self.merge.clone())
            .child(CommitsSummary::new("pr-commits", commits()));

        let open = cx.listener(|this, path: &SharedString, window, cx| this.open(path, window, cx));
        let (hunks, added, removed, blocks) = match shown {
            Some(i) => {
                let file = &self.fixture.changed[i];
                let blocks = if file.path == "src/request.rs" {
                    vec![RowBlock {
                        row: file.rows.to_view(THREAD_ROW),
                        render: std::rc::Rc::new(move |_, _| {
                            LineComment::new("pr-thread", vec![Comment::new(
                                "bot",
                                "1h ago",
                                "`has_written_status` is read before `detach` clears the sink, so a response that was already partly written takes this branch twice. Consider capturing it above the `if`.",
                            )])
                            .on_reply(|_, _, _| {})
                            .on_resolve(|_, _, _| {})
                            .into_any_element()
                        }),
                    }]
                } else {
                    Vec::new()
                };
                (file.hunks.clone(), file.added(), file.removed(), blocks)
            }
            None => (Vec::new(), 0, 0, Vec::new()),
        };
        let status = self.session.as_ref().map(|s| s.read(cx).status().join("   ")).unwrap_or_default();
        let current = shown.map(|i| SharedString::from(self.fixture.changed[i].path)).unwrap_or_default();
        let card = || div().h(px(body)).bg(theme.card).rounded(radius::lg()).p(px(6.));
        let files = div()
            .flex()
            .flex_1()
            .min_w_0()
            .gap(px(8.))
            .when(layout.tree, |d| {
                d.child(
                    card().flex_none().w(px(TREE)).child(
                        ChangedFileTree::new("pr-tree", changed.clone())
                            .reviewed(self.seen.clone())
                            .current(current)
                            .on_open(move |path, window, cx| open(path, window, cx)),
                    ),
                )
            })
            .child(
                card()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        ReviewFileHeader::new("pr-file", self.place.path.clone(), added, removed, handlers.clone())
                            .brought_in(self.place.brought_in),
                    )
                    .child(
                        InlineReview::new("pr-diff", &self.editor, hunks)
                            .decisions(false)
                            .read_only(true)
                            .on_card(true)
                            .row_blocks(blocks)
                            .height(px(body - 12. - 40. - STATUS_HEIGHT)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .h(px(STATUS_HEIGHT))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .truncate()
                            .text_size(TextSize::Xs.font_size())
                            .text_color(theme.muted_foreground)
                            .child(status),
                    ),
            );
        let lookup = self.lookup.as_ref().map(|l| {
            let this = cx.entity().downgrade();
            let finder_focus = gpui_kit::Focusable::focus_handle(&l.finder, cx);
            atelier_ui::popover::Popover::new("pr-story-lookup")
                .open(true)
                .hang(atelier_ui::popover::Hang::Centre(52.))
                .height(360.)
                .panel_focus(&finder_focus)
                .return_focus(&self.focus)
                .on_close(move |window, cx| {
                    this.update(cx, |story, cx| story.close_lookup(window, cx)).ok();
                })
                .child(l.finder.clone())
        });
        let right = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(8.))
            .child(
                ReviewBar::new("pr-bar", progress, handlers.clone())
                    .reviewed_word("seen")
                    .mark_label("Seen")
                    .next_primary(true)
                    .review_mode(self.review_mode),
            )
            .child(files)
            .children(lookup);

        // The pane's width after layout; a change that moves the split draws again.
        let this = cx.entity().downgrade();
        let measure = gpui_kit::canvas(
            move |bounds, _, cx| {
                let width = f32::from(bounds.size.width);
                this.update(cx, |s, cx| {
                    if (s.width - width).abs() > 0.5 {
                        let before = s.fit();
                        s.width = width;
                        if s.fit() != before {
                            cx.notify();
                        }
                    }
                })
                .ok();
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let pane = div()
            .relative()
            .flex()
            .size_full()
            .min_w_0()
            .gap(px(RAIL_GAP))
            .p(px(PADDING / 2.))
            .bg(theme.background)
            .child(measure)
            .when(layout.rail.is_some(), |d| d.child(rail))
            .child(right);
        handlers.keys(pane, &self.focus)
    }
}
