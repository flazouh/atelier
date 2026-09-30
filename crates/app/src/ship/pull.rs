//! Opening the pull request: the bases to pick from, the prompt the agent drafts the title and the
//! body from, and the draft's split into the two. Blocking: call it off the UI thread.
use lathe_project::Project;

use crate::ship::{branch, commit::git, drafts};

/// The most of the diff the prompt carries.
const DIFF_CAP: usize = 60_000;

/// Origin's branches a pull request from `head` can go into: the default branch first, then the rest
/// by name.
pub fn bases(project: &dyn Project, head: &str) -> Vec<String> {
    let listed = git(project, &["for-each-ref", "--format=%(refname:short)", "refs/remotes/origin"], None, None).unwrap_or_default();
    let mut names: Vec<String> = listed
        .lines()
        .filter_map(|l| l.strip_prefix("origin/"))
        .filter(|name| *name != "HEAD" && *name != head)
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    let default = branch::default_branch(project).or_else(|| names.iter().find(|n| matches!(n.as_str(), "main" | "master")).cloned());
    if let Some(default) = default
        && let Some(at) = names.iter().position(|n| *n == default)
    {
        let first = names.remove(at);
        names.insert(0, first);
    }
    names
}

/// The prompt for the pull request's title and body: the commits from `base` to HEAD, and their diff.
pub fn prompt(project: &dyn Project, base: &str) -> String {
    let range = format!("origin/{base}..HEAD");
    let log = git(project, &["log", "--no-merges", "--format=- %s%n%b", &range], None, None).unwrap_or_default();
    let mut diff = git(project, &["diff", "--stat", "-p", &format!("origin/{base}...HEAD")], None, None).unwrap_or_default();
    if diff.len() > DIFF_CAP {
        let mut cut = DIFF_CAP;
        while !diff.is_char_boundary(cut) {
            cut -= 1;
        }
        diff.truncate(cut);
        diff.push_str("\n[the rest of the diff is left out]\n");
    }
    format!(
        "Write a pull request for these commits. Answer with the title on the first line, a blank line, \
         then a short body in Markdown that says what changes and why. No preamble, no trailers.\n\n\
         Commits:\n{log}\nDiff:\n{diff}"
    )
}

/// The agent's draft as a title and a body: the first line, less a heading mark or a "Title:", and the
/// rest, cleaned as a commit message is.
pub fn title_and_body(draft: &str) -> (String, String) {
    let cleaned = drafts::message(draft);
    let (first, rest) = cleaned.split_once('\n').unwrap_or((&cleaned, ""));
    let title = first.trim().trim_start_matches('#').trim();
    let title = title.strip_prefix("Title:").unwrap_or(title).trim().to_string();
    (title, rest.trim().to_string())
}

#[cfg(test)]
mod tests;
