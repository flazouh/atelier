use super::types::PutBack;

/// What a pull and rebase did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rebased {
    /// Each of the branch's own commits, by its id before the rebase and after it.
    pub moved: Vec<(String, String)>,
    /// What became of the edits set aside, when the reader set them aside.
    pub edits: Option<PutBack>,
}
