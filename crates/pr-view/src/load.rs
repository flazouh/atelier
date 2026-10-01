//! Reading a pull request from the forge. The reads are independent, so they run at once, each on a
//! thread of its own; every part is sent the moment it arrives. This blocks until all are done, so call
//! it from a background task.
use std::thread;

use atelier_forge::{Forge, ForgeResult, PullRef};

use crate::data::{Part, PartKind};

fn part<T>(kind: PartKind, result: ForgeResult<T>, make: impl FnOnce(T) -> Part) -> Part {
    match result {
        Ok(value) => make(value),
        Err(error) => Part::Failed { part: kind, error },
    }
}

/// The parts of a refresh: the ones that change while people work. The header, the threads, the remarks,
/// the checks and the unsent comments.
pub const LIVE: [PartKind; 5] = [PartKind::Pull, PartKind::Threads, PartKind::Remarks, PartKind::Checks, PartKind::Held];

/// Reads `kinds` of `reference` and sends each part as it arrives.
pub fn load_parts(forge: &dyn Forge, reference: &PullRef, kinds: &[PartKind], send: &(dyn Fn(Part) + Sync)) {
    thread::scope(|scope| {
        for kind in kinds.iter().copied() {
            scope.spawn(move || {
                send(match kind {
                    PartKind::Pull => part(kind, forge.pull(reference), |p| Part::Pull(Box::new(p))),
                    PartKind::Files => part(kind, forge.files(reference), Part::Files),
                    PartKind::Threads => part(kind, forge.threads(reference), Part::Threads),
                    PartKind::Remarks => part(kind, forge.remarks(reference), Part::Remarks),
                    PartKind::Checks => part(kind, forge.checks(reference), Part::Checks),
                    PartKind::ReviewPoint => part(kind, forge.last_review_point(reference), Part::ReviewPoint),
                    PartKind::Held => part(kind, forge.held_comments(reference), Part::Held),
                });
            });
        }
    });
}

/// Every part.
pub fn load_all(forge: &dyn Forge, reference: &PullRef, send: &(dyn Fn(Part) + Sync)) {
    load_parts(forge, reference, &PartKind::ALL, send);
}
