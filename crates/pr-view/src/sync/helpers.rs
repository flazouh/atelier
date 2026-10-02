use atelier_forge::{Forge, ForgeError, PullRef};

use crate::{
    data::{Part, PartKind, PullData},
    load::load_parts,
};
use super::structs::{Delta, Refreshed};

/// One refresh of `known`. Parts go to `send` as they arrive. An error is the header's: without it there is
/// nothing to compare, and the failure is the caller's to time.
pub fn refresh(forge: &dyn Forge, reference: &PullRef, known: &PullData, send: &(dyn Fn(Part) + Sync)) -> Result<Refreshed, ForgeError> {
    let pull = forge.pull(reference)?;
    if known.pull.as_ref() == Some(&pull) {
        return Ok(Refreshed::default());
    }
    let head_moved = known.pull.as_ref().is_none_or(|old| old.head_sha != pull.head_sha);
    send(Part::Pull(Box::new(pull)));
    let mut kinds = vec![PartKind::Threads, PartKind::Remarks, PartKind::Checks, PartKind::Held];
    if head_moved {
        kinds.extend([PartKind::Files, PartKind::ReviewPoint]);
    }
    load_parts(forge, reference, &kinds, send);
    Ok(Refreshed { changed: true, head_moved })
}

pub fn delta(old: &PullData, new: &PullData) -> Delta {
    let (Some(a), Some(b)) = (&old.pull, &new.pull) else { return Delta::default() };
    Delta {
        head_moved: a.head_sha != b.head_sha,
        new_comments: new.comment_count().saturating_sub(old.comment_count()),
        state: (a.state != b.state).then_some(b.state),
        checks_changed: a.checks != b.checks || old.checks != new.checks,
        review_changed: a.review != b.review || a.opinions != b.opinions,
    }
}

/// The file to keep open after the file list changed. The same path if it is still there; else the file that
/// now stands where it stood; else the first; `None` for no files.
pub fn keep_place(current: Option<&str>, before: &[String], after: &[String]) -> Option<String> {
    if after.is_empty() {
        return None;
    }
    let current = current?;
    if after.iter().any(|p| p == current) {
        return Some(current.to_string());
    }
    let at = before.iter().position(|p| p == current).unwrap_or(0);
    Some(after[at.min(after.len() - 1)].clone())
}
