//! The user's comments on rows of the review. A comment keeps the lines it was written on, quoted, so it
//! still makes sense when the file moves on, and it becomes part of the next message to the agent.
use atelier_agents::session::Attachment;

use crate::merged::{Anchor, Side};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewComment {
    pub id: u64,
    pub turn: usize,
    pub path: String,
    pub side: Side,
    /// 1-based, in the version the side names.
    pub first_line: u32,
    pub last_line: u32,
    /// The rows the comment is about, as they were when it was written.
    pub quote: String,
    pub body: String,
    /// The quoted rows are no longer in the file, so the lines above may point at something else.
    pub stale: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Comments {
    next: u64,
    list: Vec<ReviewComment>,
}

impl Comments {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a comment on `anchor` and gives its id.
    pub fn add(&mut self, turn: usize, path: impl Into<String>, anchor: Anchor, body: impl Into<String>) -> u64 {
        self.next += 1;
        self.list.push(ReviewComment {
            id: self.next,
            turn,
            path: path.into(),
            side: anchor.side,
            first_line: anchor.first_line,
            last_line: anchor.last_line,
            quote: anchor.quote,
            body: body.into(),
            stale: false,
        });
        self.next
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.list.len();
        self.list.retain(|c| c.id != id);
        self.list.len() != before
    }

    pub fn set_body(&mut self, id: u64, body: impl Into<String>) -> bool {
        self.list.iter_mut().find(|c| c.id == id).map(|c| c.body = body.into()).is_some()
    }

    pub fn all(&self) -> &[ReviewComment] {
        &self.list
    }

    pub fn for_file<'a>(&'a self, path: &'a str) -> impl Iterator<Item = &'a ReviewComment> {
        self.list.iter().filter(move |c| c.path == path)
    }

    /// The file's text changed: each comment on its current rows follows its quote to where the rows are
    /// now, the nearest match to where they were. A comment whose rows are gone is marked stale and
    /// keeps its old lines. Comments on removed rows do not move: those rows are in the text before the
    /// turn, which does not change.
    pub fn reanchor(&mut self, path: &str, current: &str) {
        let rows: Vec<&str> = current.split('\n').collect();
        for comment in self.list.iter_mut().filter(|c| c.path == path && c.side == Side::Current) {
            let quote: Vec<&str> = comment.quote.split('\n').collect();
            let old = comment.first_line as usize - 1;
            let found = (0..rows.len().saturating_sub(quote.len() - 1))
                .filter(|start| rows[*start..*start + quote.len()] == quote[..])
                .min_by_key(|start| start.abs_diff(old));
            match found {
                Some(start) => {
                    comment.first_line = start as u32 + 1;
                    comment.last_line = (start + quote.len()) as u32;
                    comment.stale = false;
                }
                None => comment.stale = true,
            }
        }
    }

    /// The comments as what a message carries, in the order they were written.
    pub fn attachments(&self) -> Vec<Attachment> {
        self.list
            .iter()
            .map(|c| Attachment::LineComment {
                path: c.path.clone(),
                first_line: c.first_line,
                last_line: c.last_line,
                removed: c.side == Side::Removed,
                quote: c.quote.clone(),
                body: c.body.clone(),
            })
            .collect()
    }

    /// The attachments, and the comments gone: they are sent.
    pub fn take_attachments(&mut self) -> Vec<Attachment> {
        let out = self.attachments();
        self.list.clear();
        out
    }
}

#[cfg(test)]
mod tests;
