use std::collections::{HashMap, HashSet};

use atelier_project::Entry;

use super::helpers::{name, parent};

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
    pub(super) children: HashMap<String, Vec<usize>>,
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

    /// Whether the tree lists `path`, a file or a folder.
    pub fn has(&self, path: &str) -> bool {
        self.children.get(parent(path)).is_some_and(|list| list.iter().any(|&i| self.entries[i].path == path))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many files and folders the tree holds.
    pub fn len(&self) -> usize {
        self.entries.len()
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
