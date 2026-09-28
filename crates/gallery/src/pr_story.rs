//! The "Pull request" and "Pull requests" stories, laid out like GitQuiet's pull request and working set
//! screens (`site/public/store/pull-request.png` and `working-set.png`), in beui's look. Plain fixture
//! data: nothing here asks GitHub anything.
//!
//! The pull request: a left rail (unsent comments, checks, the conversation, the box for the whole pull
//! request, the verdict, the commits) and a right pane (the seen bar, the changed file tree, and the
//! diff on its card with a thread in place). GitQuiet's keys work while no box has focus: `s` and `w`
//! move between files, `x` marks one seen, `r` is review mode, ⌘B folds the rail and ⌘⇧B the files.

use std::collections::HashSet;

use beui::{
    ActiveTheme, ChangedFile, ChangedFileTree, CheckRun, CheckState, ChecksPanel, CommentComposer, CommentComposerEvent,
    CommitData, CommitsSummary, Comment, ConversationList, Court, CourtItem, CourtList, InlineHunk, InlineReview,
    JobStep, LineComment, PrChipData, PrState, RemarkSummary, ReviewBar, ReviewFileHeader, ReviewHandlers, ReviewProgress,
    ThreadSummary, UnsentComments, VerdictBox, VerdictEvent,
    file_tree::FileTree,
    pr::{Checks, ReviewState},
    review::step,
    theme::radius,
};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled,
    Subscription, Window,
    base::input::RowBlock,
    component::input::EditorState,
    div, prelude::FluentBuilder, px,
};

struct PrFile {
    path: &'static str,
    text: &'static str,
    hunks: fn() -> Vec<InlineHunk>,
}

const SERVER: &str = "fn onAborted(this: *RequestContext) void {
    if (this.flags.aborted) {
        this.finalizeForAbort();
        return;
    }
    if (this.response_ptr) |response| {
        this.renderResponse(response);
    }
    // A stream aborted between chunks still owns the sink, and the top of it
    // wrote a second set of headers onto a socket uWS had already taken back.
    if (this.response_ptr) |response| {
        if (this.flags.has_written_status and this.byte_stream != null) {
            this.detachByteStream();
        }
        this.renderResponse(response);
    }
    if (this.flags.aborted_mid_chunk) {
        this.flushPending();
    }
}
";

const RESPONSE: &str = "pub fn writeStatus(this: *Response, status: u16) void {
    this.status = status;
    this.flags.has_written_status = true;
}
";

const STREAMS: &str = "pub fn detach(this: *ByteStream) void {
    this.sink = null;
    this.pending.deinit();
    this.pending = .{};
}
";

const HTTP: &str = "pub const PATIENCE_MS = 20;
pub const PATIENCE_MS = 50;
";

const PR_FILES: [PrFile; 4] = [
    PrFile { path: "src/bun.js/api/server.zig", text: SERVER, hunks: || vec![InlineHunk::new("s-1", 5..8, 8..16)] },
    PrFile { path: "src/bun.js/webcore/response.zig", text: RESPONSE, hunks: || vec![InlineHunk::new("r-1", 2..2, 2..3)] },
    PrFile { path: "src/bun.js/webcore/streams.zig", text: STREAMS, hunks: || vec![InlineHunk::new("t-1", 2..2, 2..4)] },
    PrFile { path: "src/http.zig", text: HTTP, hunks: || vec![InlineHunk::new("h-1", 0..1, 1..2)] },
];

fn checks() -> Vec<CheckRun> {
    let step = |name: &str, state, log: &[&str]| JobStep {
        name: name.to_string().into(),
        state,
        seconds: None,
        log: log.iter().map(|l| SharedString::from(l.to_string())).collect(),
    };
    vec![
        CheckRun {
            name: "bun-linux-x64".into(),
            summary: "zig build test".into(),
            state: CheckState::Failed,
            steps: vec![
                step("Set up job", CheckState::Passed, &[]),
                step("Run zig build test", CheckState::Failed, &[
                    "Build Summary: 412/414 steps succeeded; 1 failed",
                    "test/js/bun/http/serve-abort.test.ts:",
                    "error: expected 1 set of headers, received 2",
                    "##[error]Process completed with exit code 1.",
                ]),
            ],
        },
        CheckRun { name: "windows-x64".into(), summary: "flaky timing on the runner".into(), state: CheckState::Tolerated, steps: vec![
            step("Run tests", CheckState::Tolerated, &["Timed out after 20ms waiting for the socket", "##[error]Process completed with exit code 1."]),
        ] },
        CheckRun { name: "lint".into(), summary: "zig fmt --check".into(), state: CheckState::Passed, steps: vec![] },
        CheckRun { name: "typecheck".into(), summary: "tsc --noEmit".into(), state: CheckState::Passed, steps: vec![] },
        CheckRun { name: "bun-darwin-aarch64".into(), summary: "zig build test".into(), state: CheckState::Running, steps: vec![] },
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
            thread(&["bot"], "has_written_status is read before detachByteStream clears it", 1, false),
            thread(&["Ada", "Rui"], "The early return leaves pending.state at .pending while the sink is gone", 2, false),
            thread(&["Dario"], "Does this need to run before renderResponse? On a HEAD request it would not", 1, false),
            thread(&["Mia", "Rui"], "Twenty milliseconds is going to be flaky on the Windows runners", 2, false),
            thread(&["Kai", "Rui"], "receivedlastchunk is the field on the state rather than on the response", 2, true),
        ],
        vec![
            remark("canary", "bun-linux-x64 built at 5b2c1a9. Download the canary build to try it."),
            remark("Jarred", "Pushed the decoder fix. The abort path is the interesting one."),
            remark("bench", "http server throughput: 118,402 req/s on main, 119,180 req/s here."),
        ],
    )
}

fn commits() -> Vec<CommitData> {
    ["Detach the byte stream before a second write", "Test an abort between chunks", "Hold the sink until flush", "Name the fields", "Drop the extra render", "Keep the status flag"]
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

pub struct PrStory {
    editor: Entity<EditorState>,
    current: usize,
    seen: HashSet<SharedString>,
    focus: FocusHandle,
    details: bool,
    files_pane: bool,
    review_mode: bool,
    unsent: usize,
    composer: Entity<CommentComposer>,
    verdict: Entity<VerdictBox>,
    _subscriptions: Vec<Subscription>,
}

impl PrStory {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = beui::CodeEditor::state(PR_FILES[0].path, PR_FILES[0].text, window, cx);
        let composer = cx.new(|cx| {
            let mut c = CommentComposer::new("On this pull request", "You", window, cx);
            c.set_text("The abort path reads right to me. Before I approve: is the HEAD case dperrault asked about covered anywhere, or does that want its own test?", window, cx);
            c.open(window, cx);
            c
        });
        let verdict = cx.new(|cx| VerdictBox::new("f4a97b1c9e2d4f0a", false, window, cx));
        let subs = vec![
            cx.subscribe(&composer, |_, _, event: &CommentComposerEvent, _| println!("comment: {event:?}")),
            cx.subscribe_in(&verdict, window, |_, verdict, event: &VerdictEvent, window, cx| {
                println!("verdict: {event:?}");
                verdict.update(cx, |v, cx| v.sent(window, cx));
            }),
        ];
        let mut seen = HashSet::new();
        seen.insert(SharedString::from(PR_FILES[3].path));
        Self {
            editor,
            current: 0,
            seen,
            focus: cx.focus_handle(),
            details: true,
            files_pane: true,
            review_mode: false,
            unsent: 2,
            composer,
            verdict,
            _subscriptions: subs,
        }
    }

    fn changed(&self) -> Vec<ChangedFile> {
        PR_FILES
            .iter()
            .map(|f| {
                let hunks = (f.hunks)();
                ChangedFile::new(f.path, hunks.iter().map(|h| h.added.len()).sum(), hunks.iter().map(|h| h.removed.len()).sum())
            })
            .collect()
    }

    fn open(&mut self, path: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at) = PR_FILES.iter().position(|f| f.path == path.as_ref()) else { return };
        if at != self.current {
            self.current = at;
            self.editor.update(cx, |s, cx| s.set_value(PR_FILES[at].text, window, cx));
            cx.notify();
        }
    }

    fn step(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let order = FileTree::new(&self.changed()).file_order();
        if let Some(path) = step(&order, Some(&SharedString::from(PR_FILES[self.current].path)), by) {
            self.open(&path, window, cx);
        }
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
            .on_previous(with(|s, w, cx| s.step(-1, w, cx)))
            .on_mark(with(|s, _, cx| {
                let path = SharedString::from(PR_FILES[s.current].path);
                if !s.seen.remove(&path) {
                    s.seen.insert(path);
                }
                cx.notify();
            }))
            .on_review_mode(with(|s, _, cx| {
                s.review_mode = !s.review_mode;
                cx.notify();
            }))
            .on_dismiss(with(|s, _, cx| {
                s.review_mode = false;
                cx.notify();
            }))
            .on_toggle_details(with(|s, _, cx| {
                s.details = !s.details;
                cx.notify();
            }))
            .on_toggle_files(with(|s, _, cx| {
                s.files_pane = !s.files_pane;
                cx.notify();
            }))
            .on_put_back(with(|s, _, cx| {
                s.seen.clear();
                cx.notify();
            }))
    }
}

impl Render for PrStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let handlers = self.handlers(cx);
        let changed = self.changed();
        let file = &changed[self.current];
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

        let rail = div()
            .id("pr-rail")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(380.))
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
            .child(CommitsSummary::new("pr-commits", commits()));

        let open = cx.listener(|this, path: &SharedString, window, cx| this.open(path, window, cx));
        let blocks = if self.current == 0 {
            vec![RowBlock {
                row: 12,
                render: std::rc::Rc::new(move |_, _| {
                    LineComment::new("pr-thread", vec![Comment::new(
                        "bot",
                        "1h ago",
                        "`has_written_status` is read before `detachByteStream` clears it, so a response that was already partly written takes this branch twice. Consider capturing it above the `if`.",
                    )])
                    .on_reply(|_, _, _| {})
                    .on_resolve(|_, _, _| {})
                    .into_any_element()
                }),
            }]
        } else {
            Vec::new()
        };
        let card = || div().h(px(body)).bg(theme.card).rounded(radius::LG).p(px(6.));
        let files = div()
            .flex()
            .flex_1()
            .min_w_0()
            .gap(px(8.))
            .child(
                card().flex_none().w(px(240.)).child(
                    ChangedFileTree::new("pr-tree", changed.clone())
                        .reviewed(self.seen.clone())
                        .current(file.path.clone())
                        .on_open(move |path, window, cx| open(path, window, cx)),
                ),
            )
            .child(
                card().flex_1().min_w_0().flex().flex_col().child(ReviewFileHeader::new("pr-file", file.path.clone(), file.added, file.removed, ReviewHandlers::default())).child(
                    InlineReview::new("pr-diff", &self.editor, (PR_FILES[self.current].hunks)())
                        .decisions(false)
                        .on_card(true)
                        .row_blocks(blocks)
                        .height(px(body - 12. - 40.)),
                ),
            );
        let right = div()
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
            .when(self.files_pane, |d| d.child(files));

        let pane = div()
            .flex()
            .size_full()
            .gap(px(12.))
            .p(px(8.))
            .bg(theme.background)
            .when(self.details && !self.review_mode, |d| d.child(rail))
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
