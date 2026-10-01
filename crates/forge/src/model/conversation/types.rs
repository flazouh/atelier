use super::structs::Comment;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Author {
    Person,
    /// An automated reviewer.
    Bot,
}

/// Something said about the pull request as a whole.
pub type Remark = Comment;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Side {
    /// The old file.
    Left,
    /// The new file.
    Right,
}

/// The reviewer's verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Verdict {
    Comment,
    Approve,
    RequestChanges,
}
