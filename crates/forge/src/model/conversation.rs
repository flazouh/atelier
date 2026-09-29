//! What people said on a pull request: remarks about the whole, threads on lines, and the review that
//! carries a verdict.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Author {
    Person,
    /// An automated reviewer.
    Bot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub kind: Author,
    pub body: String,
    pub created_at: u64,
    /// Held: written by the reader, shown to nobody else until the review is submitted.
    pub unsent: bool,
}

/// Something said about the pull request as a whole.
pub type Remark = Comment;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThreadId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The old file.
    Left,
    /// The new file.
    Right,
}

/// A conversation on one line, one range or one whole file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    pub id: ThreadId,
    pub resolved: bool,
    /// The code it hangs on has changed since.
    pub outdated: bool,
    pub path: String,
    /// The whole file is the subject: a File Remark, not a line.
    pub file_level: bool,
    /// The line it hangs on now. `None` when the code moved or went, and the thread is outdated.
    pub line: Option<u32>,
    pub start_line: Option<u32>,
    /// The line it was written on.
    pub original_line: Option<u32>,
    pub side: Side,
    pub can_resolve: bool,
    pub can_reply: bool,
    pub comments: Vec<Comment>,
}

/// The reviewer's verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Comment,
    Approve,
    RequestChanges,
}

/// A comment on a line, before it is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewLine {
    pub path: String,
    pub line: u32,
    pub start_line: Option<u32>,
    pub side: Side,
    pub body: String,
}

/// A comment the forge holds for the reader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldComment {
    pub thread: ThreadId,
    pub comment: Comment,
    pub path: String,
    pub line: Option<u32>,
}
