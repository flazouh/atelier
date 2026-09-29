//! The pull request the gallery story shows: a small Rust crate, "relay", on main; and pull request 3344,
//! which detaches a byte stream when a client aborts. The repository is real git, made on disk. The forge is
//! the in-memory one, holding the pull request's threads, remarks, checks and a failing job with its log.
use std::{path::Path, sync::Arc};

use lathe_forge::{
    Change, ChangedFile, CheckCounts, CheckStatus, Conclusion, Involved, Job, JobRef, MergeSettings, MergeState, Opinion, PullBrief, PullState, PullSummary,
    ReviewDecision, Shelf, Step, Verdict,
};

use super::{FixtureForge, repo::Repo, sample};
use crate::{
    data::Part,
    services::PrConfig,
};

const CONFIG: &str = "use std::time::Duration;\n\n/// How long a client may keep an aborted stream before the relay drops it.\npub const PATIENCE_MS: u64 = 50;\n\n/// The patience as a duration.\npub fn patience() -> Duration {\n    Duration::from_millis(PATIENCE_MS)\n}\n";
const LIB: &str = "//! A small HTTP relay: it forwards a request upstream and streams the answer back.\n\npub mod config;\npub mod request;\npub mod response;\npub mod stream;\n";

const REQUEST: &str = "use crate::config;
use crate::response::Response;
use crate::stream::ByteStream;

/// One request the relay is answering.
pub struct RequestContext {
    pub response: Option<Response>,
    pub byte_stream: Option<ByteStream>,
}

impl RequestContext {
    /// The client went away while the answer was on its way.
    pub fn on_aborted(&mut self) {
        if let Some(response) = self.response.as_mut() {
            // A stream aborted between chunks still owns the sink, and the top of it
            // wrote a second set of headers onto a socket the server had already taken back.
            if response.flags.has_written_status && self.byte_stream.is_some() {
                if let Some(stream) = self.byte_stream.as_mut() {
                    stream.detach();
                }
            }
            response.flags.aborted = 1;
        }
    }

    /// How long the relay waits on a client that went quiet.
    pub fn patience(&self) -> std::time::Duration {
        config::patience()
    }
}
";

const RESPONSE: &str = "/// What the relay has told the client so far.
#[derive(Default)]
pub struct Flags {
    pub aborted: bool,
    pub aborted_mid_chunk: bool,
    pub has_written_status: bool,
}

/// The answer going back to the client.
#[derive(Default)]
pub struct Response {
    pub status: u16,
    pub flags: Flags,
}

impl Response {
    /// Writes the status line once.
    pub fn write_status(&mut self, status: u16) {
        self.status = status;
        self.flags.has_written_status = true;
    }
}
";

const STREAM: &str = "/// A body the upstream sends in chunks.
#[derive(Default)]
pub struct ByteStream {
    pub sink: Option<Vec<u8>>,
    pub pending: Vec<u8>,
}

impl ByteStream {
    /// Lets go of the sink and anything not yet sent.
    pub fn detach(&mut self) {
        self.sink = None;
        self.pending.clear();
    }
}
";

const ABORT_TEST: &str = "use relay::request::RequestContext;
use relay::response::Response;
use relay::stream::ByteStream;

#[test]
fn an_abort_between_chunks_detaches_the_stream() {
    let mut response = Response::default();
    response.write_status(200);
    let mut request = RequestContext { response: Some(response), byte_stream: Some(ByteStream::default()) };
    request.on_aborted();
    assert!(request.byte_stream.unwrap().sink.is_none());
}
";

/// A changed file: its text at the head, and each hunk as the head row its added rows start at, the rows it
/// removed there, and how many rows it added.
struct Changed {
    path: &'static str,
    head: &'static str,
    hunks: &'static [(usize, &'static [&'static str], usize)],
}

const CHANGED: &[Changed] = &[
    Changed {
        path: "src/request.rs",
        head: REQUEST,
        hunks: &[(13, &["        if let Some(response) = self.response.as_mut() {", "            response.write_status(200);", "        }"], 10)],
    },
    Changed { path: "src/response.rs", head: RESPONSE, hunks: &[(19, &[], 1)] },
    Changed { path: "src/stream.rs", head: STREAM, hunks: &[(10, &["        self.sink = None;"], 2)] },
    Changed { path: "tests/abort.rs", head: ABORT_TEST, hunks: &[(0, &[], 12)] },
];

impl Changed {
    /// The file before the pull request: the head with each hunk's added rows swapped for its removed ones.
    /// `None` for a file the pull request made.
    fn before(&self) -> Option<String> {
        let head: Vec<&str> = self.head.trim_end_matches('\n').split('\n').collect();
        let (mut rows, mut next) = (Vec::new(), 0);
        for &(at, removed, added) in self.hunks {
            rows.extend_from_slice(&head[next..at]);
            rows.extend_from_slice(removed);
            next = at + added;
        }
        rows.extend_from_slice(&head[next..]);
        (rows != head && !(self.hunks.len() == 1 && self.hunks[0].1.is_empty() && self.hunks[0].2 == head.len())).then(|| rows.join("\n") + "\n")
    }

    fn counts(&self) -> (u32, u32) {
        (self.hunks.iter().map(|h| h.2 as u32).sum(), self.hunks.iter().map(|h| h.1.len() as u32).sum())
    }
}

pub const NUMBER: u64 = 3344;
const COMMITS: [&str; 6] = ["Detach the byte stream before a second write", "Test an abort between chunks", "Hold the sink until flush", "Name the fields", "Drop the extra render", "Keep the status flag"];
pub const BODY: &str = "A client that aborted between two chunks left the relay writing into a closed sink. The stream now detaches on abort, and a second write does nothing.";

pub struct Relay {
    pub repo: Repo,
    pub forge: Arc<FixtureForge>,
    pub reference: lathe_forge::PullRef,
    pub head: String,
    pub base: String,
    /// The commit the reader "last reviewed up to": the third of six.
    pub review_point: String,
}

const LOG: &str = "2026-09-29T04:00:00.0000001Z Current runner version: '2.337.0'
2026-09-29T04:00:01.0000001Z Complete job name: linux-x64
2026-09-29T04:00:02.0000001Z ##[group]Run actions/checkout@v4
2026-09-29T04:00:02.0000002Z with: fetch-depth: 1
2026-09-29T04:00:03.0000001Z ##[group]Run cargo test
2026-09-29T04:00:04.0000001Z running 3 tests
2026-09-29T04:00:05.0000001Z error[E0308]: mismatched types
2026-09-29T04:00:05.0000002Z   --> src/request.rs:22:37: expected `bool`, found integer
2026-09-29T04:00:06.0000001Z ##[error]Process completed with exit code 101.
2026-09-29T04:00:07.0000001Z Post job cleanup.
2026-09-29T04:00:08.0000001Z Cleaning up orphan processes
";

const SLOW_LOG: &str = "2026-09-29T04:00:00.0000001Z Current runner version: '2.337.0'
2026-09-29T04:00:03.0000001Z ##[group]Run cargo test
2026-09-29T04:00:05.0000001Z Timed out after 50ms waiting for the socket
2026-09-29T04:00:06.0000001Z ##[error]Process completed with exit code 1.
2026-09-29T04:00:08.0000001Z Cleaning up orphan processes
";

fn job(id: u64, name: &str, steps: &[(&str, Option<Conclusion>)]) -> Job {
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

impl Relay {
    pub fn build() -> Self {
        let mut files: Vec<(&str, String)> = vec![("Cargo.toml", "[package]\nname = \"relay\"\nversion = \"0.1.0\"\nedition = \"2021\"\n".into()), (".gitignore", "/target\n".into()), ("src/lib.rs", LIB.into()), ("src/config.rs", CONFIG.into())];
        for c in CHANGED {
            if let Some(before) = c.before() {
                files.push((c.path, before));
            }
        }
        let refs: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
        let repo = Repo::new(&refs);
        let base = repo.main_tip();
        let write = |names: &'static [&'static str]| {
            move |root: &Path| {
                for c in CHANGED.iter().filter(|c| names.contains(&c.path)) {
                    super::repo::put(root, c.path, c.head);
                }
            }
        };
        let (c1, c2, c3, c4) = (write(&["src/stream.rs"]), write(&["src/response.rs"]), write(&["tests/abort.rs"]), write(&["src/request.rs"]));
        let none = |_: &Path| {};
        let head = repo.pull(NUMBER, "main", &[&c1, &c2, &c3, &c4, &none, &none]);
        let review_point = super::repo::git(repo.scratch(), &["rev-parse", &format!("{head}~3")]);
        let reference = sample::reference(NUMBER);
        let mut file_list: Vec<ChangedFile> = CHANGED
            .iter()
            .map(|c| {
                let (added, removed) = c.counts();
                ChangedFile { path: c.path.into(), additions: added, deletions: removed, change: if c.before().is_none() { Change::Added } else { Change::Modified } }
            })
            .collect();
        file_list.sort_by(|a, b| a.path.cmp(&b.path));
        let mut data = sample::data(NUMBER, &head, file_list);
        {
            let pull = data.pull.as_mut().unwrap();
            pull.base_sha = base.clone();
            pull.title = COMMITS[0].into();
            pull.body = BODY.into();
            pull.author = "Rui".into();
            pull.head = "rui/detach-stream".into();
            pull.merge_state = MergeState::Blocked;
            pull.checks = CheckCounts { passed: 3, failed: 1, running: 1 };
            pull.merge = MergeSettings { methods: vec![lathe_forge::MergeMethod::Squash], default_method: lathe_forge::MergeMethod::Squash, delete_branch_on_merge: true, ..MergeSettings::default() };
            pull.opinions = vec![Opinion { reviewer: "Ada".into(), verdict: Verdict::Comment }];
            pull.remarks = 3;
        }
        let comment = |id: &str, who: &str, text: &str, ago: u64| sample::comment(id, who, text, sample::NOW - ago);
        data.threads = vec![
            sample::thread("T-bot", "src/request.rs", 17, vec![comment("c1", "bot", "`has_written_status` is read before `detach` clears the sink, so a response that was already partly written takes this branch twice. Consider capturing it above the `if`.", 3600)]),
            sample::thread("T-ada", "src/stream.rs", 11, vec![comment("c2", "Ada", "The early return leaves pending full while the sink is gone.", 7200), comment("c3", "Rui", "Agreed, `detach` clears it now.", 5400)]),
            sample::thread("T-dario", "src/response.rs", 20, vec![comment("c4", "Dario", "Does this need to run before write_status? On a HEAD request it would not.", 9000)]),
            sample::thread("T-mia", "tests/abort.rs", 8, vec![comment("c5", "Mia", "Fifty milliseconds is going to be flaky on the Windows runners.", 10_000), comment("c6", "Rui", "I will make it a channel.", 9_000)]),
            {
                let mut t = sample::thread("T-kai", "src/request.rs", 22, vec![comment("c7", "Kai", "`aborted_mid_chunk` is the field on the flags rather than on the response.", 20_000), comment("c8", "Rui", "Fixed.", 19_000)]);
                t.resolved = true;
                t
            },
        ];
        data.remarks = vec![
            comment("r1", "canary", "linux-x64 built at 5b2c1a9. Download the canary build to try it.", 10_800),
            comment("r2", "Jarred", "Pushed the decoder fix. The abort path is the interesting one.", 9_000),
            comment("r3", "bench", "http server throughput: 118,402 req/s on main, 119,180 req/s here.", 7_200),
        ];
        let required = |mut c: lathe_forge::Check| {
            c.required = true;
            c
        };
        let mut linux = required(sample::check("linux-x64", CheckStatus::Done, Some(Conclusion::Failure)));
        linux.job = Some(JobRef { repo: reference.repo.clone(), id: 11 });
        let mut windows = sample::check("windows-x64", CheckStatus::Done, Some(Conclusion::Failure));
        windows.job = Some(JobRef { repo: reference.repo.clone(), id: 12 });
        data.checks = vec![
            linux,
            windows,
            sample::check("lint", CheckStatus::Done, Some(Conclusion::Success)),
            sample::check("clippy", CheckStatus::Done, Some(Conclusion::Success)),
            sample::check("darwin-aarch64", CheckStatus::Running, None),
        ];
        data.apply(Part::ReviewPoint(Some(review_point.clone())), sample::NOW);
        let forge = FixtureForge::new()
            .with_pull(data)
            .with_repository(super::repository(&reference.repo))
            .with_involved(involved())
            .with_job(job(11, "linux-x64", &[("Set up job", Some(Conclusion::Success)), ("Run actions/checkout@v4", Some(Conclusion::Success)), ("Run cargo test", Some(Conclusion::Failure)), ("Post Run actions/checkout@v4", Some(Conclusion::Success)), ("Complete job", Some(Conclusion::Success))]), LOG)
            .with_job(job(12, "windows-x64", &[("Set up job", Some(Conclusion::Success)), ("Run cargo test", Some(Conclusion::Failure)), ("Complete job", Some(Conclusion::Success))]), SLOW_LOG);
        Self { repo, forge: Arc::new(forge), reference, head, base, review_point }
    }

    /// A config that fetches from this repository and keeps its caches next to it.
    pub fn config(&self, local: &Path) -> PrConfig {
        PrConfig::new("alex", local).remote_data(self.repo.data.to_str().unwrap()).fetch_url(self.repo.origin.to_str().unwrap())
    }
}

/// The working set the list shows: the relay pull request and others, in each of the Courts.
pub fn involved() -> Vec<Involved> {
    let repo = |slug: &str| {
        let (owner, name) = slug.split_once('/').unwrap();
        lathe_forge::RepoRef::new("github.com", owner, name)
    };
    let item = |n: u64, slug: &str, title: &str, state: PullState, shelf: Option<Shelf>, review: ReviewDecision, checks: (u32, u32, u32), comments: u32, size: (u32, u32), ago: u64, author: &str| {
        let reference = lathe_forge::PullRef { repo: repo(slug), number: n };
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
