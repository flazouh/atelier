//! A forge that lives in memory, for tests, the gallery story and the numbers. It holds pull requests
//! as [`PullData`], answers the reads from them, and applies the writes to them, so a test can send a reply
//! and read the thread back. Every call is logged. It sends nothing anywhere.

pub mod big;
mod helpers;
pub mod real;
pub mod relay;
pub mod repo;
/// Plain pieces to build a pull request from, with every field a sensible value.
pub mod sample {
    use atelier_forge::{
        Author, CheckCounts, CheckStatus, Comment, Conclusion, MergeState, Pull, PullId, PullRef, PullState, Rights, Side, Thread,
        ThreadId,
    };

    use super::*;

    pub const NOW: u64 = 1_790_700_000;

    pub fn reference(number: u64) -> PullRef {
        PullRef { repo: RepoRef::new("github.com", "flazouh", "relay"), number }
    }

    pub fn pull(reference: &PullRef, head_sha: &str) -> Pull {
        Pull {
            id: PullId(format!("PR_{}", reference.number)),
            reference: reference.clone(),
            title: "Detach the byte stream before a second write".into(),
            body: "A client that aborted between two chunks left the relay writing into a closed sink.".into(),
            state: PullState::Open,
            url: format!("https://github.com/{}/pull/{}", reference.repo.slug(), reference.number),
            author: "Rui".into(),
            base: "main".into(),
            base_sha: String::new(),
            head: "rui/detach".into(),
            head_sha: head_sha.into(),
            created_at: NOW - 86_400,
            updated_at: NOW - 3_600,
            additions: 0,
            deletions: 0,
            changed_files: 0,
            remarks: 0,
            conflicting: false,
            merge_state: MergeState::Clean,
            review: ReviewDecision::Required,
            opinions: Vec::new(),
            requested: Vec::new(),
            checks: CheckCounts { passed: 1, failed: 0, running: 0 },
            queue: None,
            auto_merge: false,
            rights: Rights::Merge,
            can_update: true,
            merge: MergeSettings::default(),
        }
    }

    pub fn comment(id: &str, author: &str, body: &str, at: u64) -> Comment {
        Comment { id: id.into(), author: author.into(), kind: Author::Person, body: body.into(), created_at: at, unsent: false }
    }

    /// An open thread on `line` of `path`, the new side.
    pub fn thread(id: &str, path: &str, line: u32, comments: Vec<Comment>) -> Thread {
        Thread {
            id: ThreadId(id.into()),
            resolved: false,
            outdated: false,
            path: path.into(),
            file_level: false,
            line: Some(line),
            start_line: None,
            original_line: Some(line),
            side: Side::Right,
            can_resolve: true,
            can_reply: true,
            comments,
        }
    }

    pub fn check(name: &str, status: CheckStatus, conclusion: Option<Conclusion>) -> Check {
        Check { name: name.into(), status, conclusion, url: None, required: false, started_at: Some(NOW - 600), completed_at: conclusion.map(|_| NOW - 60), job: None, run: None }
    }

    /// A pull request with these files, all read.
    pub fn data(number: u64, head_sha: &str, files: Vec<ChangedFile>) -> PullData {
        let reference = reference(number);
        let mut data = PullData::new(reference.clone());
        let mut pull = pull(&reference, head_sha);
        pull.changed_files = files.len() as u32;
        pull.additions = files.iter().map(|f| f.additions).sum();
        pull.deletions = files.iter().map(|f| f.deletions).sum();
        data.apply(crate::Part::Pull(Box::new(pull)), NOW);
        data.apply(crate::Part::Files(files), NOW);
        data.apply(crate::Part::Threads(Vec::new()), NOW);
        data.apply(crate::Part::Remarks(Vec::new()), NOW);
        data.apply(crate::Part::Checks(Vec::new()), NOW);
        data.apply(crate::Part::ReviewPoint(None), NOW);
        data.apply(crate::Part::Held(Vec::new()), NOW);
        data
    }
}
mod structs;
mod types;

pub use helpers::repository;
pub use structs::FixtureForge;
pub use types::Write;

use atelier_forge::{ChangedFile, Check, MergeSettings, RepoRef, ReviewDecision};
use crate::data::PullData;
