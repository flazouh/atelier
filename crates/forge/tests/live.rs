//! Reads a public pull request from GitHub through the real `gh`, end to end. Read-only: it never
//! creates, comments, reviews, merges or pushes anything. Ignored by default, since it needs a network
//! and a signed-in `gh`:
//!   cargo test -p atelier-forge --test live -- --ignored --nocapture
//! With `ATELIER_FORGE_RECORD=<folder>` it also records every answer as a fixture for `tests/replay.rs`.
use std::{path::PathBuf, sync::Arc};

use atelier_forge::{
    Conclusion, Forge, PullRef, PullState, RepoRef,
    github::{GhCli, GitHub, testing::Recording},
};
use atelier_project::LocalProject;

/// The forge, and the folder its `gh` runs in, which must outlive it.
fn forge() -> (GitHub, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let project = Arc::new(LocalProject::open(dir.path()).unwrap());
    let gh = GhCli::new(project);
    let forge = match std::env::var("ATELIER_FORGE_RECORD") {
        Ok(folder) => GitHub::with_transport(Recording::new(gh, PathBuf::from(folder))),
        Err(_) => GitHub::with_transport(gh),
    };
    (forge, dir)
}

/// A merged pull request of a public repository, with reviews, threads and a run of checks.
fn public_pull() -> PullRef {
    PullRef { repo: RepoRef::new("github.com", "oven-sh", "bun"), number: 44169 }
}

#[test]
#[ignore = "reads GitHub through the real gh"]
fn reads_a_public_pull_end_to_end() {
    let (forge, _folder) = forge();
    let reference = public_pull();

    let repository = forge.repository("https://github.com/oven-sh/bun.git").unwrap();
    assert_eq!(repository.reference.slug(), "oven-sh/bun");
    assert!(repository.default_branch.is_some() && !repository.merge.methods.is_empty());
    println!("repository: {} default {:?} methods {:?}", repository.reference.slug(), repository.default_branch, repository.merge.methods);

    let pull = forge.pull(&reference).unwrap();
    assert_eq!(pull.state, PullState::Merged);
    assert!(!pull.title.is_empty() && !pull.head_sha.is_empty() && pull.created_at > 0);
    println!("pull: {} ({:?}), {} files, checks {:?}", pull.title, pull.state, pull.changed_files, pull.checks);

    let files = forge.files(&reference).unwrap();
    assert_eq!(files.len(), pull.changed_files as usize);
    let added: u32 = files.iter().map(|f| f.additions).sum();
    assert_eq!(added, pull.additions);

    let threads = forge.threads(&reference).unwrap();
    let remarks = forge.remarks(&reference).unwrap();
    assert!(threads.iter().all(|t| !t.comments.is_empty()));
    println!("threads: {}, remarks: {}", threads.len(), remarks.len());

    let checks = forge.checks(&reference).unwrap();
    assert!(!checks.is_empty());
    // A skipped job never ran, so it has steps and no log.
    let with_job = checks
        .iter()
        .find(|c| c.job.is_some() && c.conclusion == Some(Conclusion::Success))
        .expect("a job of a workflow run that ran");
    let job = forge.job(with_job.job.as_ref().unwrap()).unwrap();
    assert_eq!(job.name, with_job.name);
    let log = forge.job_log(with_job.job.as_ref().unwrap()).unwrap();
    println!("checks: {}, job {} has {} steps, its log is {} bytes", checks.len(), job.name, job.steps.len(), log.len());

    let point = forge.last_review_point(&reference).unwrap();
    println!("last review point: {point:?}");

    let briefs = forge.briefs(&reference.repo, &[44169, 1, 44032]).unwrap();
    assert_eq!(briefs.len(), 3);
    assert_eq!(briefs[0].as_ref().unwrap().state, PullState::Merged);
    assert!(briefs[1].is_none(), "#1 is not a pull request");

    // Fifty numbers, one request: how long GitHub takes to answer it.
    let numbers: Vec<u64> = (44_100..44_150).collect();
    let start = std::time::Instant::now();
    let fifty = forge.briefs(&reference.repo, &numbers).unwrap();
    println!("50 numbers, one request: {} pull requests found, {:.0} ms", fifty.iter().flatten().count(), start.elapsed().as_secs_f64() * 1000.);

    // The reader's own working set reads private data, so it is counted and never recorded.
    if std::env::var("ATELIER_FORGE_RECORD").is_err() {
        let start = std::time::Instant::now();
        let involved = forge.involved().unwrap();
        let mut shelves = std::collections::BTreeMap::new();
        for row in &involved {
            *shelves.entry(format!("{:?}", row.shelf)).or_insert(0) += 1;
        }
        println!("working set: {} pull requests {shelves:?}, {:.0} ms", involved.len(), start.elapsed().as_secs_f64() * 1000.);
    }
}
