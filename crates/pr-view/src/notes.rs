//! The one line under the header of the rail: the most pressing thing to tell. What is being sent or
//! what just changed comes first, then a part that could not be read, then a problem with git, then a
//! note about the diff's base.
use atelier_forge::ForgeError;

use crate::data::PartKind;

pub fn compose(notice: Option<&str>, git_error: Option<&str>, base_note: Option<&str>, errors: &[(PartKind, ForgeError)]) -> Option<String> {
    if let Some(notice) = notice.filter(|n| !n.is_empty()) {
        return Some(notice.to_string());
    }
    if let Some((part, error)) = errors.first() {
        return Some(format!("Could not read {}: {error}", part.words()));
    }
    if let Some(error) = git_error {
        return Some(format!("The files cannot be shown: {error}"));
    }
    base_note.map(str::to_string)
}
