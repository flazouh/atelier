//! A project's file tree as rows: folders first, then files, each by name without regard to case, as
//! VS Code and Zed list them. Only open folders show their children. Pure, so the whole shape is
//! tested without a window; the view draws [`ProjectTree::rows`] as a virtual list.

use std::collections::{HashMap, HashSet};

use lathe_project::Entry;

/// One row of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Relative to the root, `/` between parts.
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub dir: bool,
    /// For a folder: whether its children show.
    pub open: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectTree {
    entries: Vec<Entry>,
    /// Each folder's children, as indexes into `entries`, in the order they show. `""` is the root.
    children: HashMap<String, Vec<usize>>,
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

impl ProjectTree {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut children: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            children.entry(parent(&entry.path).to_string()).or_default().push(i);
        }
        for list in children.values_mut() {
            list.sort_by(|&a, &b| {
                let (a, b) = (&entries[a], &entries[b]);
                b.dir.cmp(&a.dir).then_with(|| name(&a.path).to_lowercase().cmp(&name(&b.path).to_lowercase())).then_with(|| a.path.cmp(&b.path))
            });
        }
        Self { entries, children }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn files(&self) -> usize {
        self.entries.iter().filter(|e| !e.dir).count()
    }

    /// Every file's path, in path order, for Go to file.
    pub fn file_paths(&self) -> Vec<String> {
        self.entries.iter().filter(|e| !e.dir).map(|e| e.path.clone()).collect()
    }

    /// The rows that show with the folders in `open` open.
    pub fn rows(&self, open: &HashSet<String>) -> Vec<Row> {
        let mut rows = Vec::new();
        self.walk("", 0, open, &mut rows);
        rows
    }

    fn walk(&self, folder: &str, depth: usize, open: &HashSet<String>, rows: &mut Vec<Row>) {
        for &i in self.children.get(folder).map(Vec::as_slice).unwrap_or_default() {
            let entry = &self.entries[i];
            let is_open = entry.dir && open.contains(&entry.path);
            rows.push(Row { path: entry.path.clone(), name: name(&entry.path).to_string(), depth, dir: entry.dir, open: is_open });
            if is_open {
                self.walk(&entry.path, depth + 1, open, rows);
            }
        }
    }
}

/// Every folder above `path`, so opening them shows it.
pub fn ancestors(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = parent(path);
    while !at.is_empty() {
        out.push(at.to_string());
        at = parent(at);
    }
    out
}

#[cfg(test)]
mod tests;
