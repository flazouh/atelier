use super::types::BaseChoice;

/// A base, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    pub choice: BaseChoice,
    /// The commit the diff starts from.
    pub sha: String,
    /// Why the choice could not be honored, or what is worth knowing about it.
    pub note: Option<String>,
}
