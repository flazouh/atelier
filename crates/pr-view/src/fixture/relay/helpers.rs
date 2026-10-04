use atelier_forge::{
    CheckCounts, CheckStatus, Conclusion, Involved, Job, JobRef, PullBrief, PullState,
    PullSummary, ReviewDecision, Shelf, Step,
};

use super::super::sample;
use super::types::{COMMITS, NUMBER};

pub(super) fn job(id: u64, name: &str, steps: &[(&str, Option<Conclusion>)]) -> Job {
    Job {
        reference: JobRef { repo: sample::reference(NUMBER).repo, id },
        name: name.into(),
        status: CheckStatus::Done,
        conclusion: Some(Conclusion::Failure),
        run_id: 1,
        attempt: 1,
        steps: steps
            .iter()
            .enumerate()
            .map(|(i, (name, conclusion))| Step { number: i as u32 + 1, name: (*name).into(), status: CheckStatus::Done, conclusion: *conclusion, started_at: Some(100), completed_at: Some(104 + i as u64) })
            .collect(),
    }
}

/// The working set the list shows: the relay pull request and others, in each of the Courts.
pub fn involved() -> Vec<Involved> {
    let repo = |slug: &str| {
        let (owner, name) = slug.split_once('/').unwrap();
        atelier_forge::RepoRef::new("github.com", owner, name)
    };
    let item = |n: u64, slug: &str, title: &str, state: PullState, shelf: Option<Shelf>, review: ReviewDecision, checks: (u32, u32, u32), comments: u32, size: (u32, u32), ago: u64, author: &str| {
        let reference = atelier_forge::PullRef { repo: repo(slug), number: n };
        Involved {
            summary: PullSummary {
                brief: PullBrief { reference, title: title.into(), state, url: format!("https://github.com/{slug}/pull/{n}") },
                author: author.into(),
                created_at: sample::NOW - ago - 3600,
                updated_at: sample::NOW - ago,
                additions: size.0,
                deletions: size.1,
                comments,
                review,
                checks: Some(CheckCounts { passed: checks.0, failed: checks.1, running: checks.2 }),
                standing: Default::default(),
            },
            shelf,
        }
    };
    use PullState::*;
    use ReviewDecision::*;
    vec![
        item(NUMBER, "flazouh/relay", COMMITS[0], Open, Some(Shelf::NeedsAction), Required, (3, 1, 1), 8, (164, 10), 1800, "Rui"),
        item(327442, "microsoft/vscode", "Debounce the explorer's file watcher on very large workspaces", Open, Some(Shelf::NeedsAction), Required, (11, 0, 7), 4, (74, 29), 3600, "Kai"),
        item(22841, "oven-sh/bun", "Fix Bun.serve() dropping the body on a 304 from an upstream fetch", Open, Some(Shelf::ReadyToMerge), Approved, (12, 0, 0), 6, (214, 38), 7200, "Jarred"),
        item(95412, "vercel/next.js", "Turbopack: keep chunk order stable across server renders", Open, Some(Shelf::NeedsAction), ChangesRequested, (37, 4, 0), 14, (486, 121), 18_000, "Tess"),
        item(19187, "tailwindlabs/tailwindcss", "Keep arbitrary values with a slash out of the modifier parser", Open, Some(Shelf::WaitingForReview), Required, (6, 0, 0), 3, (88, 24), 43_200, "Ada"),
        item(412, "flazouh/gitquiet", "Serve the Working Set from the store before GitHub answers", Open, Some(Shelf::WaitingForReview), Required, (4, 0, 0), 2, (331, 94), 75_600, "Rui"),
        item(327004, "microsoft/vscode", "Bump electron to 34.5.1", Open, Some(Shelf::WaitingForReview), Approved, (9, 0, 9), 0, (6, 6), 7200, "Sam"),
        item(22790, "oven-sh/bun", "Bump zlib-ng to 2.2.5", Open, Some(Shelf::MergeQueue), Approved, (24, 0, 8), 0, (4, 4), 10_800, "Jo"),
        item(409, "flazouh/gitquiet", "Publish releases through the Chrome Web Store", Merged, None, Approved, (4, 0, 0), 0, (114, 0), 86_400, "Kai"),
    ]
}
