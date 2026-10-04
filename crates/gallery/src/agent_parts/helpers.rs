use atelier_ui::{
    AgentText, AgentTextStatus, Button, ButtonVariant, ChangedFile, ChangedFiles, Checks,
    FileChange, ModelBadge, PrCard, PrChip, PrChipData, PrFacts, PrReviewer, PrStanding, PrState, PrVerdict, ReviewState,
    StandingTone, SubagentCard, SubagentRow, SubagentStrip, ToolCall, ToolStatus,
    pr_glance::{PrDoing, PrFailing, PrFile, PrGlanceCard, PrSession, key_of, pr_cards},
};
use gpui_kit::{Context, IntoElement, ParentElement, RenderOnce, Styled, div, px};
use atelier_agents::{claude, coding_agents::CodingAgent, labs::Lab};

use super::super::{Gallery, narrow, row, section};
use super::types::{LOOP, PR_TEXT, TOOLS};

/// The live tool call at `tick`.
pub fn live_tool(tick: usize, offset: usize) -> &'static str {
    TOOLS[(tick + offset) % TOOLS.len()]
}

/// The files a turn changes, in the order it touches them.
pub fn changed_files() -> Vec<ChangedFile> {
    vec![
        ChangedFile::new("crates/ui/src/file_diff.rs", 18, 6),
        ChangedFile::new("crates/ui/src/file_diff/tests.rs", 42, 0),
        ChangedFile::new("crates/ui/src/inline_review.rs", 3, 3),
        ChangedFile::new("crates/ui/src/hunk.rs", 64, 0).change(FileChange::Added),
        ChangedFile::new("crates/ui/src/theme.rs", 2, 1),
        ChangedFile::new("crates/gallery/src/main.rs", 12, 4),
        ChangedFile::new("docs/diff-view.md", 0, 31).change(FileChange::Deleted),
        ChangedFile::new("crates/ui/src/line_numbers.rs", 5, 5).change(FileChange::Renamed { from: "crates/ui/src/gutter.rs".into() }),
        ChangedFile::new("crates/ui/src/code_editor.rs", 7, 2),
        ChangedFile::new("crates/ui/src/lib.rs", 1, 0),
        ChangedFile::new("Cargo.lock", 4, 4),
        ChangedFile::new("README.md", 2, 0),
    ]
}

/// The session's pull request.
pub fn pr_3344() -> PrChipData {
    PrChipData {
        number: 3344,
        repo: "flazouh/atelier".into(),
        title: "Fix the off-by-one in the diff view's line numbers".into(),
        state: PrState::Open,
        url: "https://github.com/flazouh/atelier/pull/3344".into(),
        facts: Some(PrFacts {
            author: "flazouh".into(),
            added: 128,
            removed: 41,
            comments: 3,
            review: ReviewState::Approved,
            checks: Some(Checks { passed: 12, failed: 0, running: 0 }),
            updated_at: hours_ago(3),
            head: "diff-line-numbers".into(),
            base: "main".into(),
            conflicting: false,
            reviewers: vec![PrReviewer { who: "ana".into(), verdict: PrVerdict::Approved }],
            standing: Some(PrStanding { tone: StandingTone::Ready, word: "Ready to merge".into(), detail: "".into() }),
        }),
    }
}

fn hours_ago(hours: u64) -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) - hours * 3600
}

/// A pull request whose test fails, waiting on a team's review.
fn pr_3311() -> PrChipData {
    PrChipData {
        number: 3311,
        repo: "flazouh/atelier".into(),
        title: "fix(relay): detach the byte stream before a second write".into(),
        state: PrState::Open,
        url: "https://github.com/flazouh/atelier/pull/3311".into(),
        facts: Some(PrFacts {
            author: "flazouh".into(),
            added: 79,
            removed: 10,
            comments: 4,
            review: ReviewState::Requested,
            checks: Some(Checks { passed: 9, failed: 1, running: 0 }),
            updated_at: hours_ago(1),
            head: "relay-abort".into(),
            base: "main".into(),
            conflicting: false,
            reviewers: vec![
                PrReviewer { who: "ana".into(), verdict: PrVerdict::Commented },
                PrReviewer { who: "core".into(), verdict: PrVerdict::Waiting },
            ],
            standing: Some(PrStanding { tone: StandingTone::Held, word: "Blocked".into(), detail: "a required check fails".into() }),
        }),
    }
}

/// The open cards, each in one state, and the chips that open them on a hover.
#[derive(IntoElement)]
pub struct PrCardsStory;

impl RenderOnce for PrCardsStory {
    fn render(self, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) -> impl IntoElement {
        let failing = pr_3311();
        let reading = PrChipData { number: 3312, title: "Retry the socket once before it times out".into(), ..pr_3311() };
        let merging = PrChipData { number: 3344, ..pr_3344() };
        let merged = PrChipData { number: 3298, state: PrState::Merged, title: "Keep the tab's scroll on a reload".into(), facts: pr_3344().facts.map(|f| PrFacts { standing: None, ..f }), ..pr_3344() };
        let store = pr_cards(cx);
        store.update(cx, |s, cx| {
            s.update_glance(key_of(&failing), |g| {
                g.failing = Some(PrFailing { name: "test (linux-x64)".into(), line: Some("error[E0308]: mismatched types".into()), url: Some("https://github.com".into()) });
                g.files = Some(vec![
                    PrFile { path: "crates/relay/src/stream.rs".into(), added: 42, removed: 8 },
                    PrFile { path: "crates/relay/tests/abort.rs".into(), added: 31, removed: 0 },
                    PrFile { path: "crates/relay/src/request.rs".into(), added: 6, removed: 2 },
                ]);
                g.session = Some(PrSession { title: "Detach the stream on abort".into(), status: "Idle".into(), running: false });
            }, cx);
            s.update_glance(key_of(&reading), |g| {
                g.failing = Some(PrFailing { name: "lint".into(), line: None, url: None });
                g.session = Some(PrSession { title: "Retry the socket".into(), status: "Working".into(), running: true });
            }, cx);
            s.update_glance(key_of(&merging), |g| g.doing = Some(PrDoing::Working("Merging…".into())), cx);
        });
        let card = |id: &'static str, pr: PrChipData, menu: bool, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
            let card = window.use_keyed_state(id, cx, |_, cx| {
                let mut card = PrGlanceCard::new(id, pr, Some(std::sync::Arc::new(|pr: &PrChipData, _: &mut gpui_kit::Window, _: &mut gpui_kit::App| println!("open #{}", pr.number))), cx);
                card.set_menu(menu, cx);
                card
            });
            div().flex_none().child(card)
        };
        let chip = |id: &'static str, pr: PrChipData| PrChip::new(id, pr).on_open(|pr, _, _| println!("open #{}", pr.number));
        div()
            .child(section("Hover a chip", row().child(chip("cards-chip-3311", pr_3311())).child(chip("cards-chip-3344", pr_3344()))))
            .child(section(
                "Open: a failing check, a review asked, its session; reading a log; merging; merged; the parts menu",
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .child(card("glance-failing", failing, false, window, cx))
                    .child(card("glance-reading", reading, false, window, cx))
                    .child(card("glance-merging", merging, false, window, cx))
                    .child(card("glance-merged", merged, false, window, cx))
                    .child(card("glance-menu", pr_3311(), true, window, cx)),
            ))
    }
}

/// What the app knows: only #3344 is a real pull request here.
pub fn resolve_pr(number: u64) -> Option<PrChipData> {
    (number == 3344).then(pr_3344)
}

/// The subagents at `tick`: Explore runs the whole time, Test runner joins at 2 and finishes at 8, and
/// Review joins at 4. The script starts at 5, so the still frame shows all three running.
pub fn strip_rows(tick: usize) -> Vec<SubagentRow> {
    let t = (tick + 5) % LOOP;
    let look = claude::look();
    let mut rows = vec![
        SubagentRow::new("sa-explore", look.clone(), "Explore", "Find every caller of hunk_starts")
            .tool(live_tool(t, 0))
            .tool_calls(3 + t as u64)
            .elapsed(format!("{}s", 12 + t)),
    ];
    if (2..11).contains(&t) {
        let test = SubagentRow::new("sa-test", look.clone(), "Test runner", "Run the diff parser tests").tool_calls(t as u64);
        rows.push(if t >= 8 { test.finished(Some(9)) } else { test.tool(live_tool(t, 4)).elapsed(format!("{}s", t - 1)) });
    }
    if t >= 4 {
        rows.push(
            SubagentRow::new("sa-review", look, "Review", "Check the off-by-one fix")
                .tool(live_tool(t, 2))
                .tool_calls(t as u64 - 3)
                .elapsed(format!("{}s", t - 3)),
        );
    }
    rows
}

/// A running subagent card at `tick`.
pub fn running_card(id: &'static str, tick: usize) -> SubagentCard {
    SubagentCard::new(id, claude::look(), "Explore", "Find every caller of hunk_starts")
        .model("Opus 5.5")
        .model_mark(anthropic())
        .elapsed(format!("{}s", 12 + tick % LOOP))
        .tool_calls(12 + (tick % LOOP) as u64)
        .live_tool(live_tool(tick, 0))
        .calls(sample_calls())
}

fn sample_calls() -> Vec<ToolCall> {
    vec![
        ToolCall::new("sc-read", "Read file").file("crates/ui/src/file_diff.rs").meta("214 lines").status(ToolStatus::Done),
        ToolCall::new("sc-grep", "Searched code").tool("fn hunk_starts").status(ToolStatus::Done),
        ToolCall::new("sc-read2", "Read file").file("crates/ui/src/theme.rs").status(ToolStatus::Running),
    ]
}

/// Play live, or Pause while it plays.
fn live_button(playing: bool, cx: &mut Context<Gallery>) -> impl IntoElement {
    Button::new("live")
        .label(if playing { "Pause" } else { "Play live" })
        .variant(ButtonVariant::Secondary)
        .on_click(cx.listener(|this, _, _, cx| this.toggle_live(cx)))
}

pub fn changed_files_story(tick: usize, playing: bool, cx: &mut Context<Gallery>) -> impl IntoElement {
    let files = changed_files();
    // While it runs, a file arrives on each tick.
    let arrived = (3 + tick % LOOP).min(files.len());
    let running = arrived < files.len();
    div()
        .child(section("Live", row().child(live_button(playing, cx))))
        .child(section(
            "Running turn, open: files past the fold enter too",
            narrow(ChangedFiles::new("cf-running-open", files.iter().take(arrived).cloned().collect()).running(running).default_open(true)),
        ))
        .child(section(
            "Finished turn, folded",
            narrow(ChangedFiles::new("cf-done", files.clone()).on_open_file(|path, _, _| println!("open {path}")).on_review(|path, _, _| println!("review from {path}"))),
        ))
        .child(section(
            if running { "Running turn" } else { "Running turn (all files in)" },
            narrow(ChangedFiles::new("cf-running", files.iter().take(arrived).cloned().collect()).running(running)),
        ))
        .child(section("Short list", narrow(ChangedFiles::new("cf-short", files.into_iter().take(2).collect()))))
}

/// The Git view's sidebar with a worktree in each state it can be in.
pub fn worktrees_story() -> impl IntoElement {
    use atelier_ui::{
        git_panel::GitPanel,
        worktree_list::{NoteTone::*, WorktreeNote as Note, WorktreeRow},
    };
    let row = |path: &str, branch: Option<&str>, notes: Vec<Note>| WorktreeRow {
        path: path.to_string().into(),
        folder: format!("~/code/{path}").into(),
        branch: branch.map(|b| b.to_string().into()),
        main: false,
        notes,
        sessions: 0,
    };
    let rows = vec![
        WorktreeRow { main: true, sessions: 2, ..row("atelier", Some("main"), vec![]) },
        row("atelier-sidebar", Some("sidebar-rework"), vec![Note::new("3 uncommitted", Warning), Note::new("2 only here", Warning)]),
        row("atelier-slider", Some("compact-slider"), vec![Note::new("merged", Good)]),
        row("atelier-spike", None, vec![Note::new("1 only here", Warning)]),
        row("atelier-old", Some("old-layout"), vec![Note::new("folder missing", Warning)]),
        row("atelier-usb", Some("release"), vec![Note::new("locked", Quiet), Note::new("4 behind", Quiet)]),
        row("atelier-pr", Some("pr-302"), vec![Note::new("1 only here", Warning), Note::new("remote branch gone", Quiet)]),
    ];
    div().child(section(
        "Git view sidebar: main checkout, work in progress, merged, detached, missing, locked",
        div().w(px(320.)).h(px(640.)).child(GitPanel::new("wt-git", "atelier").branch(Some("main".into())).worktrees(rows)),
    ))
}

pub fn subagent_card_story(tick: usize, playing: bool, cx: &mut Context<Gallery>) -> impl IntoElement {
    let done = SubagentCard::new("sc-done", claude::look(), "Review", "Check the off-by-one fix")
        .model("Opus 5.5")
        .model_mark(anthropic())
        .tool_calls(12)
        .finished(Some(38))
        .calls(sample_calls());
    div()
        .child(section("Live", row().child(live_button(playing, cx))))
        .child(section("Running, press to open its tool calls", narrow(running_card("sc-running", tick))))
        .child(section("Done", narrow(done)))
        .child(section(
            "Before its first tool call",
            narrow(SubagentCard::new("sc-new", claude::look(), "Plan", "Split the review into steps").model("Sonnet 5").model_mark(anthropic()).elapsed("1s")),
        ))
}

pub fn subagent_strip_story(tick: usize, playing: bool, cx: &mut Context<Gallery>) -> impl IntoElement {
    div()
        .child(section("Live: rows join, finish, hold, and leave", row().child(live_button(playing, cx))))
        .child(section("Above the composer", narrow(SubagentStrip::new("strip", strip_rows(tick)))))
}

pub fn pr_card_story() -> impl IntoElement {
    let card = |id: &'static str, state: PrState, checks: Checks, review: ReviewState| {
        PrCard::new(id, PrChipData { state, ..pr_3344() }).checks(checks).review(review).on_open(|pr, _, _| println!("open #{}", pr.number))
    };
    div()
        .child(section("Open, checks passed, approved", narrow(card("pc-open", PrState::Open, Checks { passed: 3, ..Default::default() }, ReviewState::Approved))))
        .child(section("Draft, checks running", narrow(card("pc-draft", PrState::Draft, Checks { passed: 1, running: 2, ..Default::default() }, ReviewState::None))))
        .child(section("Open, one failing, changes asked", narrow(card("pc-fail", PrState::Open, Checks { passed: 2, failed: 1, running: 0 }, ReviewState::ChangesRequested))))
        .child(section("Merged", narrow(card("pc-merged", PrState::Merged, Checks { passed: 3, ..Default::default() }, ReviewState::Approved))))
        .child(section("Closed, review asked", narrow(card("pc-closed", PrState::Closed, Checks::default(), ReviewState::Requested))))
}

pub fn pr_chip_story() -> impl IntoElement {
    let chip = |id: &'static str, state: PrState| PrChip::new(id, PrChipData { state, ..pr_3344() }).on_open(|pr, _, _| println!("open #{}", pr.number));
    div()
        .child(section(
            "Each state",
            row()
                .child(chip("chip-open", PrState::Open))
                .child(chip("chip-draft", PrState::Draft))
                .child(chip("chip-merged", PrState::Merged))
                .child(chip("chip-closed", PrState::Closed)),
        ))
        .child(section(
            "In agent text: #3344 is known, #9999 is not",
            narrow(
                AgentText::new("chip-text", PR_TEXT)
                    .status(AgentTextStatus::Complete)
                    .pr_resolver(resolve_pr)
                    .on_open_pr(|pr, _, _| println!("open #{}", pr.number)),
            ),
        ))
}

/// The Anthropic mark, for Claude's models.
pub fn anthropic() -> atelier_ui::BrandMark {
    Lab::Anthropic.mark().expect("Anthropic has a mark")
}

/// A badge for `label`, with the mark when there is one and a monogram when there is not.
fn badge(id: &'static str, label: &'static str, mark: Option<atelier_ui::BrandMark>) -> ModelBadge {
    match mark {
        Some(mark) => ModelBadge::new(label).mark(id, mark),
        None => ModelBadge::new(label).monogram(id),
    }
}

pub fn model_badge_story() -> impl IntoElement {
    let models = ["Opus 5.5", "GPT-5.2", "Grok 4.5", "Auto", "Copilot", "Local 7B"];
    let labs = Lab::ALL.iter().zip(models).map(|(lab, model)| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .w(px(110.))
            .child(badge(lab.name(), model, lab.mark()))
            .child(div().text_size(px(11.)).opacity(0.6).child(lab.name()))
    });
    let agents = CodingAgent::ALL.iter().map(|agent| badge(agent.name(), agent.name(), agent.mark()));
    div()
        .child(section("Labs: grey at rest, in colour under the pointer", row().children(labs)))
        .child(section("Agents", row().children(agents)))
        .child(section("No mark: a monogram", row().child(ModelBadge::new("Local 7B").monogram("mono"))))
        .child(section(
            "In a turn header",
            div().flex().items_center().gap(px(8.)).text_size(px(13.)).child("Claude").child(badge("turn", "Opus 5.5", Some(anthropic()))),
        ))
}
