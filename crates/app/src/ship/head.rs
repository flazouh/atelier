//! Files' texts in HEAD, all in one `git cat-file --batch` (each git call is a round trip on a remote
//! project): what the commit list counts against, and the diff a draft is made from. Blocking.

use lathe_project::Project;

use crate::ship::commit::git;

/// The text of each of `paths` in HEAD, in order; `None` for one HEAD does not have (or no HEAD yet).
pub fn texts(project: &dyn Project, paths: &[&str]) -> Vec<Option<String>> {
    let asked: String = paths.iter().map(|p| format!("HEAD:{p}\n")).collect();
    let Ok(out) = git(project, &["cat-file", "--batch"], None, Some(asked.into_bytes())) else {
        return vec![None; paths.len()];
    };
    let mut rest = out.as_str();
    let mut texts = Vec::with_capacity(paths.len());
    for _ in paths {
        let Some((header, after)) = rest.split_once('\n') else { break };
        // "<sha> <type> <size>", then the object and a line end; or "<name> missing".
        let size = header.split(' ').nth(2).and_then(|s| s.parse::<usize>().ok());
        match size {
            Some(size) if after.len() >= size && after.is_char_boundary(size) => {
                texts.push(Some(after[..size].to_string()));
                rest = after.get(size + 1..).unwrap_or("");
            }
            _ => {
                texts.push(None);
                rest = after;
            }
        }
    }
    texts.resize(paths.len(), None);
    texts
}

#[cfg(test)]
mod tests;
