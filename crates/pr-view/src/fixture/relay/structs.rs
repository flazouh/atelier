use std::{path::Path, sync::Arc};

use atelier_forge::{
    Change, ChangedFile, CheckCounts, CheckStatus, Conclusion, JobRef, MergeSettings,
    MergeState, Opinion, Verdict,
};

use super::super::{FixtureForge, repo::Repo, sample};
use crate::{data::Part, services::PrConfig};
use super::types::{BODY, CHANGED, COMMITS, CONFIG, LIB, LOG, NUMBER, SLOW_LOG};
use super::helpers::{involved, job};

/// A changed file: its text at the head, and each hunk as the head row its added rows start at, the rows it
/// removed there, and how many rows it added.
pub(super) struct Changed {
    pub(super) path: &'static str,
    pub(super) head: &'static str,
    pub(super) hunks: &'static [(usize, &'static [&'static str], usize)],
}

impl Changed {
    /// The file before the pull request: the head with each hunk's added rows swapped for its removed ones.
    /// `None` for a file the pull request made.
    pub(super) fn before(&self) -> Option<String> {
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

pub struct Relay {
    pub repo: Repo,
    pub forge: Arc<FixtureForge>,
    pub reference: atelier_forge::PullRef,
    pub head: String,
    pub base: String,
    /// The commit the reader "last reviewed up to": the third of six.
    pub review_point: String,
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
                    super::super::repo::put(root, c.path, c.head);
                }
            }
        };
        let (c1, c2, c3, c4) = (write(&["src/stream.rs"]), write(&["src/response.rs"]), write(&["tests/abort.rs"]), write(&["src/request.rs"]));
        let none = |_: &Path| {};
        let head = repo.pull(NUMBER, "main", &[&c1, &c2, &c3, &c4, &none, &none]);
        let review_point = super::super::repo::git(repo.scratch(), &["rev-parse", &format!("{head}~3")]);
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
            pull.merge = MergeSettings { methods: vec![atelier_forge::MergeMethod::Squash], default_method: atelier_forge::MergeMethod::Squash, delete_branch_on_merge: true, ..MergeSettings::default() };
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
        let required = |mut c: atelier_forge::Check| {
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
            .with_repository(super::super::repository(&reference.repo))
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
