use std::collections::HashMap;

use atelier_agents::session::Attachment;
use atelier_review::{
    Anchor,
    Change,
    Comments,
    Content,
    FileReview,
    Merged,
    ReviewComment,
    Reviewed,
    SessionReview,
    Side,
    TurnReview,
};
use serde::{Deserialize, Serialize};

use crate::review_pane::Scope;
use super::types::{Approval, ChangeRecord, Decided, KindRecord, ScopeRecord};

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
    pub pull: Option<atelier_forge::PullRef>,
    /// The reader's answer to each call's approval, by the call's id.
    pub approvals: HashMap<String, Approval>,
    /// When the agent last worked in the session, in seconds since the epoch: a resume writes to the
    /// agent's own file, so its time is no guide.
    pub last_activity: Option<u64>,
    pub(super) reviewed: Reviewed,
    /// The marks as `reviewed` holds them, which it does not list: by scope key, path and version.
    pub(super) marks: Vec<(usize, String, u64)>,
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
    pub(super) turns: Vec<Vec<FileRecord>>,
    pub(super) turn_marks: Vec<(usize, usize)>,
    pub(super) decided: Vec<DecidedRecord>,
    /// Missing from records written before commits were kept.
    #[serde(default)]
    pub(super) committed: Vec<(ScopeRecord, String, String)>,
    /// Missing from records written before pull requests were kept.
    #[serde(default)]
    pub(super) pull: Option<atelier_forge::PullRef>,
    /// Missing from records written before approvals were kept.
    #[serde(default)]
    pub(super) approvals: Vec<(String, Approval)>,
    #[serde(default)]
    pub(super) last_activity: Option<u64>,
    pub(super) marks: Vec<(usize, String, u64)>,
    pub(super) comments: Vec<CommentRecord>,
    pub(super) sent: Vec<(CommentRecord, bool)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct FileRecord {
    pub(super) path: String,
    pub(super) kind: KindRecord,
    pub(super) before: Option<String>,
    pub(super) after: Option<String>,
    pub(super) exact: bool,
    pub(super) renamed_from: Option<String>,
}

impl FileRecord {
    pub(super) fn of(file: &FileReview) -> Self {
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

    pub(super) fn rebuild(self) -> FileReview {
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
pub(super) struct DecidedRecord {
    pub(super) scope: ScopeRecord,
    pub(super) path: String,
    /// The baseline, with what the reader accepted, and the text now; `None` for a file with no text.
    pub(super) texts: Option<(String, String)>,
    pub(super) on_disk: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct CommentRecord {
    pub(super) turn: usize,
    pub(super) path: String,
    pub(super) removed: bool,
    pub(super) first_line: u32,
    pub(super) last_line: u32,
    pub(super) quote: String,
    pub(super) body: String,
    pub(super) stale: bool,
}

impl CommentRecord {
    pub(super) fn of(c: &ReviewComment) -> Self {
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

    pub(super) fn anchor(&self) -> Anchor {
        let side = if self.removed { Side::Removed } else { Side::Current };
        Anchor { side, first_line: self.first_line, last_line: self.last_line, quote: self.quote.clone() }
    }

    /// A sent comment as it went, under `id`, which is only for this launch.
    pub(super) fn rebuild(self, id: u64) -> ReviewComment {
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
