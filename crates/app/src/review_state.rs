//! A session's review, as one piece of state: the turns it tracked, where each turn's card sits, what the
//! reader decided and marked, and the comments waiting for the next message or already sent. It is kept
//! in the project's data folder with the session (`review/<session id>.json`), so a session resumed after
//! a restart opens its review as it was left.
//!
//! lathe-review's types are not serializable, so the record holds what rebuilds them: each file's texts
//! before and after its turn, each decided file's baseline and current text (`Merged::diff` gives the
//! hunks still to decide), each mark with the file's version (a mark on a file that changed since stays
//! expired), and each comment's anchor.

use std::collections::HashMap;

use lathe_agents::session::Attachment;
use lathe_review::{Anchor, Change, Comments, Content, FileReview, Merged, ReviewComment, Reviewed, SessionReview, Side, TurnReview};
use serde::{Deserialize, Serialize};

use crate::review_pane::Scope;

/// Where the record of session `id` lives in the project's data folder.
pub fn record_path(id: &str) -> String {
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("review/{safe}.json")
}

/// Each reviewed file as the review left it, by scope and path: its hunks after the reader's decisions
/// and edits (`None` for a file with no text), and the text last written or read on disk.
pub type Decided = HashMap<(Scope, String), (Option<Merged>, Option<String>)>;

/// How the reader answered a call's approval, kept across a resume: the history the agent replays has
/// no approvals in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Approval {
    Approved,
    AlwaysAllowed,
    Denied,
}

impl Approval {
    pub fn of(kind: lathe_agents::session::ChoiceKind) -> Self {
        use lathe_agents::session::ChoiceKind;
        match kind {
            ChoiceKind::Allow => Self::Approved,
            ChoiceKind::AllowAlways => Self::AlwaysAllowed,
            ChoiceKind::Deny => Self::Denied,
        }
    }
}

#[derive(Default)]
pub struct ReviewState {
    /// Every finished turn of the session.
    pub turns: SessionReview,
    /// After how many conversation items each turn's card shows, and which turn.
    pub turn_marks: Vec<(usize, usize)>,
    pub decided: Decided,
    /// The files whose decisions went into a commit, by scope and path: the commit's short id.
    pub committed: HashMap<(Scope, String), String>,
    /// The pull request the session opened, if it opened one.
    pub pull: Option<lathe_forge::PullRef>,
    /// The reader's answer to each call's approval, by the call's id.
    pub approvals: HashMap<String, Approval>,
    /// When the agent last worked in the session, in seconds since the epoch: a resume writes to the
    /// agent's own file, so its time is no guide.
    pub last_activity: Option<u64>,
    reviewed: Reviewed,
    /// The marks as `reviewed` holds them, which it does not list: by scope key, path and version.
    marks: Vec<(usize, String, u64)>,
    /// The comments that go with the next message.
    pub comments: Comments,
    /// Comments sent with a message, and whether the agent's turn after it ended.
    pub sent: Vec<(ReviewComment, bool)>,
}

impl ReviewState {
    /// A turn ended at conversation item `at`: it is kept, its card follows that item when it changed
    /// files, and the comments sent before it are answered.
    pub fn finish_turn(&mut self, turn: TurnReview, at: usize) {
        let index = self.turns.push(turn);
        if !self.turns.turns()[index].files().is_empty() {
            self.turn_marks.push((at, index));
        }
        for (_, answered) in &mut self.sent {
            *answered = true;
        }
    }

    /// The waiting comments, as the next message's attachments; they are sent from now on.
    pub fn send_comments(&mut self) -> Vec<Attachment> {
        self.sent.extend(self.comments.all().iter().cloned().map(|c| (c, false)));
        self.comments.take_attachments()
    }

    pub fn is_reviewed(&self, key: usize, file: &FileReview) -> bool {
        self.reviewed.is_reviewed(key, file)
    }

    pub fn set_reviewed(&mut self, key: usize, file: &FileReview, on: bool) {
        self.marks.retain(|(k, path, _)| !(*k == key && *path == file.path));
        if on {
            self.reviewed.mark(key, file);
            self.marks.push((key, file.path.clone(), file.version()));
        } else {
            self.reviewed.unmark(key, &file.path);
        }
    }

    pub fn record(&self) -> Record {
        Record {
            turns: self.turns.turns().iter().map(|t| t.files().iter().map(FileRecord::of).collect()).collect(),
            turn_marks: self.turn_marks.clone(),
            decided: self
                .decided
                .iter()
                .map(|((scope, path), (merged, on_disk))| DecidedRecord {
                    scope: ScopeRecord::of(*scope),
                    path: path.clone(),
                    texts: merged.as_ref().map(|m| (m.baseline(), m.current())),
                    on_disk: on_disk.clone(),
                })
                .collect(),
            committed: {
                let mut committed: Vec<_> = self.committed.iter().map(|((scope, path), sha)| (ScopeRecord::of(*scope), path.clone(), sha.clone())).collect();
                // One order, so the same state gives the same record.
                committed.sort_by(|a, b| (&a.1, &a.2).cmp(&(&b.1, &b.2)));
                committed
            },
            pull: self.pull.clone(),
            last_activity: self.last_activity,
            approvals: {
                let mut kept: Vec<_> = self.approvals.iter().map(|(id, a)| (id.clone(), *a)).collect();
                kept.sort_by(|a, b| a.0.cmp(&b.0));
                kept
            },
            marks: self.marks.clone(),
            comments: self.comments.all().iter().map(CommentRecord::of).collect(),
            sent: self.sent.iter().map(|(c, answered)| (CommentRecord::of(c), *answered)).collect(),
        }
    }

    pub fn from_record(record: Record) -> Self {
        let mut state = Self::default();
        for files in record.turns {
            state.turns.push(TurnReview::new(files.into_iter().map(FileRecord::rebuild).collect()));
        }
        state.turn_marks = record.turn_marks;
        state.decided = record
            .decided
            .into_iter()
            .map(|d| ((d.scope.rebuild(), d.path), (d.texts.map(|(baseline, current)| Merged::diff(&baseline, &current)), d.on_disk)))
            .collect();
        state.pull = record.pull;
        state.last_activity = record.last_activity;
        state.approvals = record.approvals.into_iter().collect();
        state.committed = record.committed.into_iter().map(|(scope, path, sha)| ((scope.rebuild(), path), sha)).collect();
        for (key, path, version) in record.marks {
            let file = match key {
                usize::MAX => state.turns.whole().into_iter().find(|f| f.path == path),
                turn => state.turns.turns().get(turn).and_then(|t| t.file(&path)).cloned(),
            };
            // A mark counts for the version it was made on, as `Reviewed` holds it.
            if let Some(file) = file.filter(|f| f.version() == version) {
                state.set_reviewed(key, &file, true);
            }
        }
        for c in record.comments {
            let (turn, path, body) = (c.turn, c.path.clone(), c.body.clone());
            state.comments.add(turn, path, c.anchor(), body);
        }
        // Sent comments take ids from the top, so they never meet a waiting one's.
        state.sent = record.sent.into_iter().enumerate().map(|(i, (c, answered))| (c.rebuild(u64::MAX - i as u64), answered)).collect();
        state
    }
}

/// What the data folder keeps of a session's review.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    turns: Vec<Vec<FileRecord>>,
    turn_marks: Vec<(usize, usize)>,
    decided: Vec<DecidedRecord>,
    /// Missing from records written before commits were kept.
    #[serde(default)]
    committed: Vec<(ScopeRecord, String, String)>,
    /// Missing from records written before pull requests were kept.
    #[serde(default)]
    pull: Option<lathe_forge::PullRef>,
    /// Missing from records written before approvals were kept.
    #[serde(default)]
    approvals: Vec<(String, Approval)>,
    #[serde(default)]
    last_activity: Option<u64>,
    marks: Vec<(usize, String, u64)>,
    comments: Vec<CommentRecord>,
    sent: Vec<(CommentRecord, bool)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct FileRecord {
    path: String,
    kind: KindRecord,
    before: Option<String>,
    after: Option<String>,
    exact: bool,
    renamed_from: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum KindRecord {
    Text,
    /// Not text, and whether it was added, deleted or changed.
    Binary(ChangeRecord),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum ChangeRecord {
    Modified,
    Added,
    Deleted,
}

impl FileRecord {
    fn of(file: &FileReview) -> Self {
        let change = match &file.change {
            Change::Added => ChangeRecord::Added,
            Change::Deleted => ChangeRecord::Deleted,
            Change::Modified | Change::Renamed { .. } => ChangeRecord::Modified,
        };
        let kind = match &file.content {
            Content::Text(_) => KindRecord::Text,
            Content::Binary => KindRecord::Binary(change),
            Content::Unknown => KindRecord::Unknown,
        };
        let renamed_from = match &file.change {
            Change::Renamed { from } => Some(from.clone()),
            _ => None,
        };
        Self { path: file.path.clone(), kind, before: file.before.clone(), after: file.after.clone(), exact: file.exact, renamed_from }
    }

    fn rebuild(self) -> FileReview {
        let file = match self.kind {
            KindRecord::Text => FileReview::from_texts(self.path, self.before, self.after, self.exact),
            KindRecord::Binary(change) => FileReview::binary(
                self.path,
                match change {
                    ChangeRecord::Modified => Change::Modified,
                    ChangeRecord::Added => Change::Added,
                    ChangeRecord::Deleted => Change::Deleted,
                },
            ),
            KindRecord::Unknown => FileReview::unknown(self.path, self.after),
        };
        match self.renamed_from {
            Some(from) => file.renamed(from),
            None => file,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct DecidedRecord {
    scope: ScopeRecord,
    path: String,
    /// The baseline, with what the reader accepted, and the text now; `None` for a file with no text.
    texts: Option<(String, String)>,
    on_disk: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum ScopeRecord {
    Turn(usize),
    Whole,
}

impl ScopeRecord {
    fn of(scope: Scope) -> Self {
        match scope {
            Scope::Turn(turn) => Self::Turn(turn),
            Scope::Whole => Self::Whole,
        }
    }

    fn rebuild(self) -> Scope {
        match self {
            Self::Turn(turn) => Scope::Turn(turn),
            Self::Whole => Scope::Whole,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct CommentRecord {
    turn: usize,
    path: String,
    removed: bool,
    first_line: u32,
    last_line: u32,
    quote: String,
    body: String,
    stale: bool,
}

impl CommentRecord {
    fn of(c: &ReviewComment) -> Self {
        Self {
            turn: c.turn,
            path: c.path.clone(),
            removed: c.side == Side::Removed,
            first_line: c.first_line,
            last_line: c.last_line,
            quote: c.quote.clone(),
            body: c.body.clone(),
            stale: c.stale,
        }
    }

    fn anchor(&self) -> Anchor {
        let side = if self.removed { Side::Removed } else { Side::Current };
        Anchor { side, first_line: self.first_line, last_line: self.last_line, quote: self.quote.clone() }
    }

    /// A sent comment as it went, under `id`, which is only for this launch.
    fn rebuild(self, id: u64) -> ReviewComment {
        let anchor = self.anchor();
        ReviewComment {
            id,
            turn: self.turn,
            path: self.path,
            side: anchor.side,
            first_line: anchor.first_line,
            last_line: anchor.last_line,
            quote: anchor.quote,
            body: self.body,
            stale: self.stale,
        }
    }
}

#[cfg(test)]
mod tests;
