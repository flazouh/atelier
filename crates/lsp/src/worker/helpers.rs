use std::{
    path::{Path, PathBuf},
    sync::{atomic::Ordering, mpsc::Receiver},
    thread,
    time::{Duration, Instant},
};

use crate::LspError;
use super::structs::Session;
use super::types::{CONTENT_MODIFIED, Job, SERVER_CANCELLED};

/// `path` with its links and `..` resolved, so one file is always one key, whatever path a caller or
/// a server names it by. A file that does not exist keeps the path it was given.
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn run(mut session: Session, queue: Receiver<Job>) {
    while let Ok(first) = queue.recv() {
        // Take everything already waiting. A newer diagnostics request makes an older one for the
        // same document moot: the text it asked about is gone.
        for job in triage(std::iter::once(first).chain(queue.try_iter()).collect()) {
            session.run(job);
        }
    }
    session.alive.store(false, Ordering::Relaxed);
    let _ = session.client.shutdown(Duration::from_secs(2));
}

/// The jobs worth running from a batch, in order. A newer question of the same kind about the same
/// document makes an older one moot: the text or the pointer it was about has moved on. So only the
/// newest navigate, hover, diagnostics and symbols job per document runs, and only the newest query
/// for project names, as the user types it; the rest are answered `Superseded`. References, which the
/// user asked for by name, always run.
pub(super) fn triage(batch: Vec<Job>) -> Vec<Job> {
    let key = |job: &Job| match job {
        Job::Navigate { doc, .. } => Some((0, doc.path.clone())),
        Job::Hover { doc, .. } => Some((1, doc.path.clone())),
        Job::Diagnostics { doc, .. } => Some((2, doc.path.clone())),
        Job::Symbols { doc, .. } => Some((3, doc.path.clone())),
        Job::ProjectSymbols { .. } => Some((4, PathBuf::new())),
        Job::References { .. } => None,
    };
    let keys: Vec<_> = batch.iter().map(key).collect();
    let mut keep = Vec::with_capacity(batch.len());
    for (index, job) in batch.into_iter().enumerate() {
        let newer = keys[index].as_ref().is_some_and(|k| keys[index + 1..].iter().any(|later| later.as_ref() == Some(k)));
        if newer {
            job.fail(LspError::Superseded);
        } else {
            keep.push(job);
        }
    }
    keep
}

/// Asks again while the server says the text moved under it, as the spec tells a client to, backing
/// off a little each time. Any other answer, and the last one once `ask` has passed, goes back as is.
pub fn until_settled<T>(ask: Duration, mut request: impl FnMut() -> Result<T, LspError>) -> Result<T, LspError> {
    let deadline = Instant::now() + ask;
    let mut pause = Duration::from_millis(50);
    loop {
        match request() {
            Err(LspError::Server { code: CONTENT_MODIFIED | SERVER_CANCELLED, .. }) if Instant::now() + pause < deadline => {
                thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(800));
            }
            answer => return answer,
        }
    }
}
