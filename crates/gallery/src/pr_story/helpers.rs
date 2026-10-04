use atelier_ui::{
    CheckRun,
    CheckState,
    Comment,
    CommitData,
    Court,
    CourtItem,
    CourtList,
    JobStep,
    PrChipData,
    PrState,
    RemarkSummary,
    ThreadSummary,
    pr::{Checks, ReviewState},
};
use gpui_kit::{AnyElement, Entity, IntoElement, ParentElement, SharedString, Styled, div, px};

use super::structs::{Fit, PrStory};
use super::types::{COMMITS, DIFF_MIN, PADDING, RAIL_GAP, RAIL_MAX, RAIL_MIN, TREE, TREE_GAP};

pub(super) fn checks() -> Vec<CheckRun> {
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

pub(super) fn conversation() -> (Vec<ThreadSummary>, Vec<RemarkSummary>) {
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

pub(super) fn commits() -> Vec<CommitData> {
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

/// The working set, as GitQuiet's screen lists it.
pub fn pull_requests() -> impl IntoElement {
    let item = |n: u64, repo: &str, title: &str, court: Court, why: &str, review: ReviewState, checks: Checks, comments: usize, size: (usize, usize), age: &str, at: u64, unread: bool| CourtItem {
        pr: PrChipData { number: n, repo: repo.to_string().into(), title: title.to_string().into(), state: PrState::Open, url: format!("https://github.com/{repo}/pull/{n}").into(), facts: None },
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
