//! The "Pull request" and "Pull requests" stories, laid out like GitQuiet's pull request and working set
//! screens (`site/public/store/pull-request.png` and `working-set.png`), in beui's look.
//!
//! The pull request: a left rail (unsent comments, checks, the conversation, the box for the whole pull
//! request, the verdict, the commits) and a right pane (the seen bar, the changed file tree, and the
//! diff on its card with a thread in place). GitQuiet's keys work while no box has focus: `s` and `w`
//! move between files, `x` marks one seen, `r` is review mode, ⌘B folds the rail and ⌘⇧B the tree.
//!
//! It fits any pane from an 1100px window up ([`fit`]): the rail and the right pane share the width,
//! the tree folds away first when the diff would get too narrow, then the rail narrows. ⌘⇧B brings the
//! tree back, and the choice holds until the next ⌘⇧B.
//!
//! The diff is read, not typed in, and its new side is the file at the pull request's head, so the
//! language server answers on it as in the Editor story (`editor_lsp`), with rust-analyzer on a small
//! crate written to disk (`pr_fixture`). The removed rows get nothing.
//!
//! - Hover a name for its card; ⌘-click it or press F12 to go to its definition. In this file the caret
//!   moves; in another changed file, that file opens and the tree selects it; in a file the pull request
//!   did not change, the file opens Brought In, out of the seen count. Escape or Previous goes back to
//!   the file before, along a stack.
//! - On a declaration, ⌘-click lists its uses.
//! - `u` Uses, `o` Names in this file, `T` Go to name, `t` Go to file each open a finder over the diff;
//!   their caps are on the file's header.
//! - The server's problems underline the new side, and its state is the line under the diff.

use std::{collections::HashSet, path::PathBuf, rc::Rc};

use beui::{
    ActiveTheme, ChangedFile, ChangedFileTree, CheckRun, CheckState, ChecksPanel, CommentComposer, CommentComposerEvent,
    CommitData, CommitsSummary, Comment, ConversationList, Court, CourtItem, CourtList, Filter, Finder, FinderEvent, FinderItem,
    InlineReview, JobStep, LineComment, PrChipData, PrState, RemarkSummary, ReviewBar, ReviewFileHeader, ReviewHandlers,
    ReviewProgress, RowMap, ThreadSummary, UnsentComments, VerdictBox, VerdictEvent,
    file_tree::FileTree,
    keys::Command,
    pr::{Checks, ReviewState},
    review::step,
    theme::radius,
    typography::TextSize,
};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window,
    base::input::RowBlock,
    component::input::EditorState,
    deferred, div, prelude::FluentBuilder, px,
};
use lathe_lsp::{LspError, canonical, client::uri_to_path};
use lsp_types::Position;

use crate::{
    editor_lsp::{EditorSession, Elsewhere, Jump},
    pr_fixture::Fixture,
};

/// The head row of `src/request.rs` a bot's thread hangs under.
const THREAD_ROW: usize = 16;
/// The line under the diff that says what the server is doing.
const STATUS_HEIGHT: f32 = 22.;

fn checks() -> Vec<CheckRun> {
    let step = |name: &str, state, log: &[&str]| JobStep {
        name: name.to_string().into(),
        state,
        seconds: None,
        log: log.iter().map(|l| SharedString::from(l.to_string())).collect(),
    };
    vec![
        CheckRun {
            name: "linux-x64".into(),
            summary: "cargo test".into(),
            state: CheckState::Failed,
            steps: vec![
                step("Set up job", CheckState::Passed, &[]),
                step("Run cargo test", CheckState::Failed, &[
                    "error[E0308]: mismatched types",
                    "  --> src/request.rs:22:37: expected `bool`, found integer",
                    "##[error]Process completed with exit code 101.",
                ]),
            ],
        },
        CheckRun { name: "windows-x64".into(), summary: "flaky timing on the runner".into(), state: CheckState::Tolerated, steps: vec![
            step("Run tests", CheckState::Tolerated, &["Timed out after 50ms waiting for the socket", "##[error]Process completed with exit code 1."]),
        ] },
        CheckRun { name: "lint".into(), summary: "cargo fmt --check".into(), state: CheckState::Passed, steps: vec![] },
        CheckRun { name: "clippy".into(), summary: "cargo clippy".into(), state: CheckState::Passed, steps: vec![] },
        CheckRun { name: "darwin-aarch64".into(), summary: "cargo test".into(), state: CheckState::Running, steps: vec![] },
    ]
}

fn conversation() -> (Vec<ThreadSummary>, Vec<RemarkSummary>) {
    let thread = |people: &[&str], first: &str, n: usize, resolved: bool| ThreadSummary {
        people: people.iter().map(|p| SharedString::from(p.to_string())).collect(),
        comments: (0..n).map(|i| Comment::new(people[i % people.len()].to_string(), "1h ago", if i == 0 { first.to_string() } else { "Agreed, changed it.".into() })).collect(),
        first: first.to_string().into(),
        resolved,
    };
    let remark = |author: &str, text: &str| RemarkSummary {
        author: author.to_string().into(),
        comment: Comment::new(author.to_string(), "3h ago", text.to_string()),
        first: text.to_string().into(),
    };
    (
        vec![
            thread(&["bot"], "has_written_status is read before detach clears the sink", 1, false),
            thread(&["Ada", "Rui"], "The early return leaves pending full while the sink is gone", 2, false),
            thread(&["Dario"], "Does this need to run before write_status? On a HEAD request it would not", 1, false),
            thread(&["Mia", "Rui"], "Fifty milliseconds is going to be flaky on the Windows runners", 2, false),
            thread(&["Kai", "Rui"], "aborted_mid_chunk is the field on the flags rather than on the response", 2, true),
        ],
        vec![
            remark("canary", "linux-x64 built at 5b2c1a9. Download the canary build to try it."),
            remark("Jarred", "Pushed the decoder fix. The abort path is the interesting one."),
            remark("bench", "http server throughput: 118,402 req/s on main, 119,180 req/s here."),
        ],
    )
}

const COMMITS: [&str; 6] =
    ["Detach the byte stream before a second write", "Test an abort between chunks", "Hold the sink until flush", "Name the fields", "Drop the extra render", "Keep the status flag"];

/// The pull request's description, which fills the squash commit's message.
const PR_BODY: &str = "A client that aborted between two chunks left the relay writing into a closed sink. The stream now detaches on abort, and a second write does nothing.";

fn commits() -> Vec<CommitData> {
    COMMITS
        .iter()
        .enumerate()
        .map(|(i, title)| CommitData {
            sha: format!("f4a97b{i}c9e2d").into(),
            title: title.to_string().into(),
            author: "Rui".into(),
            age: if i == 0 { "2h ago".into() } else { format!("{}d ago", i).into() },
            at: 100 - i as u64,
        })
        .collect()
}

/// What the file card shows: a changed file, or one brought in to read beside them.
#[derive(Clone, Debug, PartialEq)]
struct Place {
    /// Relative to the repository.
    path: String,
    brought_in: bool,
}

/// Where a row of a lookup leads: a file and a place in it, in that file's own rows, or in the shown
/// rows when it is the file on screen.
#[derive(Clone, Debug)]
struct Lead {
    path: PathBuf,
    position: Position,
    shown_rows: bool,
}

/// A lookup open over the diff, and where each of its rows leads.
struct Lookup {
    command: Command,
    finder: Entity<Finder>,
    leads: Vec<Lead>,
    asking: Task<()>,
    _events: Subscription,
}

pub struct PrStory {
    fixture: Fixture,
    editor: Entity<EditorState>,
    place: Place,
    /// Where Escape and Previous go back to, newest last, with the caret there.
    back: Vec<(Place, Position)>,
    session: Option<Entity<EditorSession>>,
    lookup: Option<Lookup>,
    /// The repository's files, listed once on a background thread; `None` until then.
    files: Option<Rc<Vec<String>>>,
    _listing: Task<()>,
    seen: HashSet<SharedString>,
    focus: FocusHandle,
    details: bool,
    /// The tree as ⌘⇧B last left it; `None` lets the width decide.
    tree: Option<bool>,
    /// The pane's width in the last frame.
    width: f32,
    review_mode: bool,
    unsent: usize,
    composer: Entity<CommentComposer>,
    verdict: Entity<VerdictBox>,
    merge: Entity<beui::MergeBox>,
    _subscriptions: Vec<Subscription>,
    _session: Option<Subscription>,
}

impl PrStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let fixture = Fixture::write();
        let first = &fixture.changed[0];
        let editor = beui::CodeEditor::state(first.path, first.text.as_str(), window, cx);
        let composer = cx.new(|cx| {
            let mut c = CommentComposer::new("On this pull request", "You", window, cx);
            c.set_text("The abort path reads right to me. Before I approve: is the HEAD case dperrault asked about covered anywhere, or does that want its own test?", window, cx);
            c.open(window, cx);
            c
        });
        let verdict = cx.new(|cx| VerdictBox::new("f4a97b1c9e2d4f0a", false, window, cx));
        let merge = cx.new(|cx| {
            let facts = crate::merge_story::states().remove(0).1;
            let choice = beui::merge::first_choice(&facts, None);
            beui::MergeBox::new(facts, choice, COMMITS[0], PR_BODY, window, cx)
        });
        let subs = vec![
            cx.subscribe_in(&merge, window, |_, merge, event: &beui::MergeBoxEvent, _, cx| {
                println!("merge: {event:?}");
                if let beui::MergeBoxEvent::Act { action: beui::merge::Action::Merge(_), choice, .. } = event {
                    merge.update(cx, |b, cx| b.merged(choice.delete_branch, cx));
                }
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

    fn changed(&self) -> Vec<ChangedFile> {
        self.fixture.changed.iter().map(|f| ChangedFile::new(f.path, f.added(), f.removed())).collect()
    }

    /// The rows the card shows over the file on screen.
    fn rows(&self) -> RowMap {
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
        beui::code_editor::set_diagnostics(&self.editor, Vec::new(), cx);
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
        let session = cx.new(|cx| EditorSession::for_review(editor, path, rows, Some(elsewhere), cx));
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
    fn open(&mut self, path: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
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

    fn step(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
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

    fn fill_uses(&mut self, uses: Vec<lathe_lsp::Target>, cx: &mut Context<Self>) {
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

    fn fill_names(&mut self, names: Vec<lathe_lsp::Symbol>, with_file: bool, cx: &mut Context<Self>) {
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

/// The rail's widest and narrowest, the tree's width, and the least the diff card may have.
const RAIL_MAX: f32 = 380.;
const RAIL_MIN: f32 = 300.;
const TREE: f32 = 240.;
const DIFF_MIN: f32 = 460.;
/// The pane's padding on both sides, the gap after the rail, and the gap after the tree.
const PADDING: f32 = 16.;
const RAIL_GAP: f32 = 12.;
const TREE_GAP: f32 = 8.;

/// How the pane splits its width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// The rail's width, or `None` when it is folded.
    pub rail: Option<f32>,
    pub tree: bool,
}

/// The split for a pane `width` wide: the tree shows while the diff keeps [`DIFF_MIN`] beside a full
/// rail, unless ⌘⇧B chose (`tree`); then the rail takes what the diff leaves, from [`RAIL_MAX`] down to
/// [`RAIL_MIN`].
pub fn fit(width: f32, rail: bool, tree: Option<bool>) -> Fit {
    let rail_space = if rail { RAIL_MAX + RAIL_GAP } else { 0. };
    let tree = tree.unwrap_or(width >= PADDING + rail_space + TREE + TREE_GAP + DIFF_MIN);
    let tree_space = if tree { TREE + TREE_GAP } else { 0. };
    let rail = rail.then(|| (width - PADDING - RAIL_GAP - tree_space - DIFF_MIN).clamp(RAIL_MIN, RAIL_MAX));
    Fit { rail, tree }
}

impl PrStory {
    fn fit(&self) -> Fit {
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
            .child(div().flex().flex_col().flex_none().rounded(radius::LG).bg(theme.card).child(ConversationList::new("pr-conversation", threads, remarks)).child(self.composer.clone()))
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
        let card = || div().h(px(body)).bg(theme.card).rounded(radius::LG).p(px(6.));
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
            deferred(div().absolute().top(px(52.)).left_0().right_0().flex().justify_center().child(l.finder.clone())).with_priority(1)
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

/// The working set, as GitQuiet's screen lists it.
pub fn pull_requests() -> impl IntoElement {
    let item = |n: u64, repo: &str, title: &str, court: Court, why: &str, review: ReviewState, checks: Checks, comments: usize, size: (usize, usize), age: &str, at: u64, unread: bool| CourtItem {
        pr: PrChipData { number: n, repo: repo.to_string().into(), title: title.to_string().into(), state: PrState::Open, url: format!("https://github.com/{repo}/pull/{n}").into() },
        author: ["Kai", "Rui", "Tess", "Mia", "Sam", "Jo"][n as usize % 6].into(),
        court,
        why: why.to_string().into(),
        checks,
        review,
        comments,
        added: size.0,
        removed: size.1,
        age: age.to_string().into(),
        changed_at: at,
        unread,
    };
    let c = |passed, failed, running| Checks { passed, failed, running };
    let items = vec![
        item(327442, "microsoft/vscode", "Debounce the explorer's file watcher on very large workspaces", Court::NeedsYou, "Review asked of you", ReviewState::None, c(11, 0, 7), 4, (74, 29), "1h ago", 99, false),
        item(22841, "oven-sh/bun", "Fix Bun.serve() dropping the body on a 304 from an upstream fetch", Court::NeedsYou, "Ready to merge", ReviewState::Approved, c(12, 0, 0), 6, (214, 38), "2h ago", 98, false),
        item(327108, "microsoft/vscode", "Restore the terminal's scrollback after a window reload", Court::NeedsYou, "Review asked of you", ReviewState::Requested, c(9, 0, 0), 7, (158, 42), "4h ago", 97, true),
        item(95412, "vercel/next.js", "Turbopack: keep chunk order stable across server renders", Court::NeedsYou, "Changes asked of you", ReviewState::ChangesRequested, c(37, 4, 0), 14, (486, 121), "5h ago", 96, true),
        item(37096, "facebook/react", "Warn once per component when a ref is read during render", Court::NeedsYou, "Review asked of you", ReviewState::None, c(8, 0, 0), 11, (46, 8), "6h ago", 95, false),
        item(19204, "tailwindlabs/tailwindcss", "Resolve @source against the stylesheet rather than the project root", Court::NeedsYou, "Failing on your branch", ReviewState::None, c(12, 2, 0), 2, (63, 18), "9h ago", 92, false),
        item(19187, "tailwindlabs/tailwindcss", "Keep arbitrary values with a slash out of the modifier parser", Court::Waiting, "Review asked of Ada", ReviewState::Requested, c(6, 0, 0), 3, (88, 24), "12h ago", 90, false),
        item(412, "flazouh/gitquiet", "Serve the Working Set from the store before GitHub answers", Court::Waiting, "Review asked of Rui", ReviewState::Requested, c(4, 0, 0), 2, (331, 94), "21h ago", 89, false),
        item(414, "flazouh/gitquiet", "Draw the Verdict box where the review is submitted", Court::Waiting, "Review asked of Kai", ReviewState::None, c(4, 0, 0), 1, (128, 16), "1d ago", 88, false),
        item(327004, "microsoft/vscode", "Bump electron to 34.5.1", Court::Running, "Checks running", ReviewState::Approved, c(9, 0, 9), 0, (6, 6), "2h ago", 87, false),
        item(22790, "oven-sh/bun", "Bump zlib-ng to 2.2.5", Court::Running, "In the merge queue", ReviewState::Approved, c(24, 0, 8), 0, (4, 4), "3h ago", 86, false),
        item(95190, "vercel/next.js", "Update the App Router caching docs for use cache", Court::Running, "Checks running", ReviewState::Approved, c(33, 0, 8), 4, (218, 96), "4h ago", 85, false),
        item(409, "flazouh/gitquiet", "Publish releases through the Chrome Web Store", Court::Settled, "Merged", ReviewState::Approved, c(4, 0, 0), 0, (114, 0), "1d ago", 80, false),
    ];
    let mut items = items;
    items[12].pr.state = PrState::Merged;
    div().size_full().p(px(8.)).child(CourtList::new("courts", items).on_open(|pr, _, _| println!("open #{}", pr.number)))
}

/// So the gallery can hold the story as any other element.
pub fn element(story: &Entity<PrStory>) -> AnyElement {
    story.clone().into_any_element()
}

#[cfg(test)]
mod tests;
