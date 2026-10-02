use std::{io, path::PathBuf, thread};

use crate::Project;
use super::structs::{TreeState, Upstream, Worktree};

/// The worktrees of `project`'s repository, the main checkout first, each with what it holds. A folder that
/// is no repository has none. Each worktree is read on a thread of its own, as a remote host is slow to ask.
pub fn worktrees(project: &dyn Project) -> io::Result<Vec<Worktree>> {
    let listed = project.git(&["worktree", "list", "--porcelain", "-z"])?;
    if !listed.ok() {
        return Ok(Vec::new());
    }
    let mut trees = parse_list(&listed.stdout);
    let default = default_branch(project);
    thread::scope(|s| {
        for tree in trees.iter_mut().filter(|t| t.prunable.is_none()) {
            let default = default.as_deref();
            s.spawn(move || tree.state = state_of(project, tree, default));
        }
    });
    Ok(trees)
}

/// Reads `git worktree list --porcelain -z`: a record per worktree, its fields ended by NULs and the record
/// by an empty field. The first is the main checkout.
pub(super) fn parse_list(text: &str) -> Vec<Worktree> {
    let mut trees: Vec<Worktree> = Vec::new();
    let mut open = false;
    for field in text.split('\0') {
        if field.is_empty() {
            open = false;
            continue;
        }
        let (key, value) = field.split_once(' ').unwrap_or((field, ""));
        if key == "worktree" {
            let main = trees.is_empty();
            trees.push(Worktree { path: PathBuf::from(value), branch: None, head: None, main, locked: None, prunable: None, state: None });
            open = true;
            continue;
        }
        let Some(tree) = trees.last_mut().filter(|_| open) else { continue };
        match key {
            "HEAD" if value.bytes().any(|b| b != b'0') => tree.head = Some(value.to_string()),
            "branch" => tree.branch = Some(value.strip_prefix("refs/heads/").unwrap_or(value).to_string()),
            "locked" => tree.locked = Some(value.to_string()),
            "prunable" => tree.prunable = Some(value.to_string()),
            _ => {}
        }
    }
    trees
}

/// The branch a worktree is merged into: the remote's default (`origin/main`), else a local `main` or `master`.
fn default_branch(project: &dyn Project) -> Option<String> {
    let ask = |args: &[&str]| answer(project, args);
    ask(&["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"]).or_else(|| {
        ["main", "master"].into_iter().find(|b| ask(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{b}")]).is_some()).map(String::from)
    })
}

/// What git printed for `args`, trimmed, when it succeeded and printed something.
fn answer(project: &dyn Project, args: &[&str]) -> Option<String> {
    project.git(args).ok().filter(|o| o.ok()).map(|o| o.stdout.trim().to_string()).filter(|s| !s.is_empty())
}

/// What `tree` holds, or `None` when any of it cannot be read: a guess here could cost work on removal.
fn state_of(project: &dyn Project, tree: &Worktree, default: Option<&str>) -> Option<TreeState> {
    let here = project.at(&tree.path).ok()?;
    let status = here.git(&["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"]).ok().filter(|o| o.ok())?;
    let mut state = parse_status(&status.stdout);
    if tree.head.is_none() {
        return Some(state);
    }
    let own = tree.branch.as_ref().map(|b| format!("--exclude={b}"));
    let mut args = vec!["rev-list", "--count", "HEAD", "--not"];
    args.extend(own.as_deref());
    args.extend(["--branches", "--remotes"]);
    state.only_here = answer(&*here, &args)?.parse().ok()?;
    state.merged = default.map(|d| merged_into(&*here, d));
    Some(state)
}

/// Whether merging HEAD into `default` would change nothing: HEAD is in it, or its changes are, as after a
/// squash merge.
fn merged_into(here: &dyn Project, default: &str) -> bool {
    if here.git(&["merge-base", "--is-ancestor", "HEAD", default]).is_ok_and(|o| o.ok()) {
        return true;
    }
    let merged = answer(here, &["merge-tree", "--write-tree", default, "HEAD"]);
    let tree = answer(here, &["rev-parse", &format!("{default}^{{tree}}")]);
    matches!((merged, tree), (Some(m), Some(t)) if m.lines().next() == Some(t.as_str()))
}

/// Reads `git status --porcelain=v2 --branch -z`: the branch headers for the upstream, then an entry per file.
fn parse_status(text: &str) -> TreeState {
    let mut state = TreeState::default();
    let (mut follows, mut ab) = (false, None);
    let mut fields = text.split('\0');
    while let Some(field) = fields.next() {
        let mut xy = field.get(2..4).unwrap_or("..").chars();
        let (x, y) = (xy.next().unwrap_or('.'), xy.next().unwrap_or('.'));
        match field.split(' ').next().unwrap_or("") {
            "#" if field.starts_with("# branch.upstream ") => follows = true,
            "#" if field.starts_with("# branch.ab ") => {
                let mut counts = field["# branch.ab ".len()..].split(' ').map(|n| n.trim_start_matches(['+', '-']).parse::<usize>().ok());
                ab = counts.next().flatten().zip(counts.next().flatten());
            }
            "1" | "2" => {
                state.staged += usize::from(x != '.');
                state.changed += usize::from(y != '.');
                // A rename or a copy carries the path it came from in the next field.
                if field.starts_with('2') {
                    fields.next();
                }
            }
            "u" => state.conflicted += 1,
            "?" => state.untracked += 1,
            _ => {}
        }
    }
    state.upstream = match (follows, ab) {
        (false, _) => Upstream::None,
        (true, Some((ahead, behind))) => Upstream::Tracking { ahead, behind },
        (true, None) => Upstream::Gone,
    };
    state
}
