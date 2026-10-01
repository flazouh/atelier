//! The pull request as the models beui draws. Times are words ("2h ago"), so every function takes `now`.
use beui::{
    ChangedFile, CommitData, RemarkSummary, ThreadSummary,
    changed_files::FileChange,
    line_comment::Comment as UiComment,
    verdict::Decision,
};
use gpui_kit::SharedString;
use atelier_forge::{Change, Comment, Pull, Remark, Thread, Verdict, time};

use crate::git::{Commit, FileEntry};

pub fn file_change(change: Change, from: Option<&str>) -> FileChange {
    match change {
        Change::Added => FileChange::Added,
        Change::Deleted => FileChange::Deleted,
        Change::Renamed | Change::Copied => match from {
            Some(from) => FileChange::Renamed { from: from.to_string().into() },
            None => FileChange::Modified,
        },
        Change::Modified => FileChange::Modified,
    }
}

/// The changed files git found, for the tree.
pub fn changed_files(entries: &[FileEntry]) -> Vec<ChangedFile> {
    entries
        .iter()
        .map(|e| ChangedFile::new(e.path.clone(), e.additions as usize, e.deletions as usize).change(file_change(e.change, e.old_path.as_deref())))
        .collect()
}

/// The changed files the forge listed, for the moment before git has answered.
pub fn forge_files(files: &[atelier_forge::ChangedFile]) -> Vec<ChangedFile> {
    files.iter().map(|f| ChangedFile::new(f.path.clone(), f.additions as usize, f.deletions as usize).change(file_change(f.change, None))).collect()
}

pub fn commit_data(commits: &[Commit], now: u64) -> Vec<CommitData> {
    commits
        .iter()
        .map(|c| CommitData { sha: c.sha.clone().into(), title: c.title.clone().into(), author: c.author.clone().into(), age: time::ago(now, c.at).into(), at: c.at })
        .collect()
}

pub fn comment(comment: &Comment, now: u64) -> UiComment {
    UiComment::new(comment.author.clone(), time::ago(now, comment.created_at), comment.body.clone())
}

/// A comment's first words for a line of the list: the first line with something in it, cut at 90
/// characters.
pub fn first_words(body: &str) -> SharedString {
    let plain = without_html(body);
    let line = plain.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let cut: String = line.chars().take(90).collect();
    if line.chars().count() > 90 { format!("{cut}…").into() } else { cut.into() }
}

/// The text of a comment with its HTML comments and tags taken out: bots write `<div><sup>Updated…</sup></div>`
/// and `<!-- marker -->`, and a line of the list should say the words, not the markup. A `<` that does not
/// start a tag (`a < b`) stays.
pub fn without_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        if let Some(inner) = after.strip_prefix("<!--") {
            match inner.find("-->") {
                Some(end) => rest = &inner[end + 3..],
                None => return out,
            }
            continue;
        }
        let starts_tag = after[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '/');
        match (starts_tag, after.find('>')) {
            (true, Some(end)) => rest = &after[end + 1..],
            _ => {
                out.push('<');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn thread_summary(thread: &Thread, now: u64) -> ThreadSummary {
    let mut people: Vec<SharedString> = Vec::new();
    for c in &thread.comments {
        let author = SharedString::from(c.author.clone());
        if !people.contains(&author) {
            people.push(author);
        }
    }
    ThreadSummary {
        people,
        comments: thread.comments.iter().map(|c| comment(c, now)).collect(),
        first: thread.comments.first().map(|c| first_words(&c.body)).unwrap_or_default(),
        resolved: thread.resolved,
    }
}

pub fn remark_summary(remark: &Remark, now: u64) -> RemarkSummary {
    RemarkSummary { author: remark.author.clone().into(), comment: comment(remark, now), first: first_words(&remark.body) }
}

/// What the reader said, from the opinions on the pull request. `me` is the reader's login. The forge does
/// not say which commit a review was about, so it counts as about the current one.
pub fn stated_verdict(pull: &Pull, me: &str) -> Option<(Decision, bool)> {
    pull.opinions.iter().find(|o| o.reviewer == me).map(|o| {
        let decision = match o.verdict {
            Verdict::Approve => Decision::Approved,
            Verdict::RequestChanges => Decision::ChangesRequested,
            Verdict::Comment => Decision::Commented,
        };
        (decision, true)
    })
}
