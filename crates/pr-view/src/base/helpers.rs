use crate::git::{PrGit, Prepared, is_sha};
use super::structs::Base;
use super::types::BaseChoice;

/// What to open with: since the last review when the reader has reviewed this branch before and it has
/// moved on, else the whole pull request.
pub fn opening_choice(review_point: Option<&str>, head: &str) -> BaseChoice {
    match review_point {
        Some(point) if is_sha(point) && !head.starts_with(point) && point != head => BaseChoice::LastReview,
        _ => BaseChoice::Whole,
    }
}

pub fn resolve(git: &PrGit, prepared: &Prepared, choice: &BaseChoice, review_point: Option<&str>) -> Base {
    let whole = |choice: BaseChoice, note: Option<String>| Base { choice, sha: prepared.merge_base.clone(), note };
    let candidate = match choice {
        BaseChoice::Whole => return whole(BaseChoice::Whole, None),
        BaseChoice::LastReview => match review_point {
            Some(point) => point.to_string(),
            None => return whole(BaseChoice::Whole, Some("You have not reviewed this pull request before, so this is all of it.".into())),
        },
        BaseChoice::Commit(sha) => sha.clone(),
    };
    if !is_sha(&candidate) || !git.knows(prepared, &candidate) {
        return whole(BaseChoice::Whole, Some("That commit is not on this pull request any more, so this is all of it.".into()));
    }
    if candidate == prepared.head {
        return Base { choice: choice.clone(), sha: candidate, note: Some("Nothing has been pushed since.".into()) };
    }
    if !git.is_ancestor(prepared, &candidate, &prepared.head) {
        let words = match choice {
            BaseChoice::LastReview => "Your last review is not part of this branch any more (it was rewritten), so this is all of it.",
            _ => "That commit is not part of this branch any more, so this is all of it.",
        };
        return whole(BaseChoice::Whole, Some(words.into()));
    }
    Base { choice: choice.clone(), sha: candidate, note: None }
}
