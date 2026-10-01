use std::thread;

use atelier_forge::{Forge, ForgeResult, PullRef};

use crate::data::{Part, PartKind};

pub(super) fn part<T>(kind: PartKind, result: ForgeResult<T>, make: impl FnOnce(T) -> Part) -> Part {
    match result {
        Ok(value) => make(value),
        Err(error) => Part::Failed { part: kind, error },
    }
}

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
