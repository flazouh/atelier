use std::{
    collections::HashMap,
    fs,
    io,
    path::{Path, PathBuf},
    sync::{Mutex, Weak},
};

use ignore::{WalkBuilder, gitignore::Gitignore};

/// Writes `target` whole, through a temporary file beside it, so a reader never sees half of it. The
/// file keeps its permissions, so saving a script keeps it runnable.
pub(crate) fn write_whole(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let temporary = target.with_file_name(format!(".{name}.atelier-save"));
    fs::write(&temporary, bytes)?;
    if let Ok(meta) = fs::metadata(target) {
        fs::set_permissions(&temporary, meta.permissions())?;
    }
    fs::rename(&temporary, target).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

/// Copies a file, or a folder with all it holds, to a place that is not there yet.
pub(crate) fn copy_all(from: &Path, to: &Path) -> io::Result<()> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} is there already", to.display())));
    }
    if fs::metadata(from)?.is_dir() {
        fs::create_dir(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_all(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(drop)
    }
}

/// Does `op` in the folder `root`.
pub(crate) fn apply_op(root: &Path, op: &crate::FsOp) -> io::Result<()> {
    use crate::{FsOp, host_path};
    let there = |path: &str| host_path(root, path);
    let free = |path: &Path| {
        if fs::symlink_metadata(path).is_ok() {
            Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} is there already", path.display())))
        } else {
            Ok(())
        }
    };
    match op {
        FsOp::NewFolder { path } => {
            let at = there(path)?;
            free(&at)?;
            fs::create_dir_all(at)
        }
        FsOp::Rename { from, to } => {
            let (from, to) = (there(from)?, there(to)?);
            free(&to)?;
            fs::rename(from, to)
        }
        FsOp::Copy { from, to } => copy_all(&there(from)?, &there(to)?),
        FsOp::Delete { path } => {
            let at = there(path)?;
            if at == root {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "the project's own folder stays"));
            }
            if fs::symlink_metadata(&at)?.is_dir() { fs::remove_dir_all(at) } else { fs::remove_file(at) }
        }
    }
}

/// The walk the listing makes: dotfiles show, as editors show them, and `.git` never does. A .gitignore counts even
/// before the folder is a git repository, and each nested checkout's own counts below it.
pub(crate) fn walker(root: &Path) -> WalkBuilder {
    let mut walk = WalkBuilder::new(root);
    walk.hidden(false).require_git(false).filter_entry(|e| e.file_name() != ".git");
    walk
}

/// `root` and every folder under it the listing shows: the folders a watch registers one by one where each costs the
/// host an inotify watch. A folder of checkouts holds ten times as many in their build output and packages.
pub(crate) fn watched_dirs(root: &Path) -> Vec<PathBuf> {
    walker(root).build().flatten().filter(|e| e.file_type().is_some_and(|t| t.is_dir())).map(|e| e.into_path()).collect()
}

/// What the .gitignore files from the project's root down to a path say of it, read once per folder.
pub(crate) struct Ignores {
    root: PathBuf,
    read: HashMap<PathBuf, Option<Gitignore>>,
}

impl Ignores {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root, read: HashMap::new() }
    }

    /// Whether the listing leaves `path` out: it is in a `.git`, or a .gitignore in one of its folders ignores it.
    pub(crate) fn ignored(&mut self, path: &Path, dir: bool) -> bool {
        let Ok(rel) = path.strip_prefix(&self.root) else { return true };
        if rel.components().any(|c| c.as_os_str() == ".git") {
            return true;
        }
        let mut folder = self.root.clone();
        let mut parts = rel.components().peekable();
        while let Some(part) = parts.next() {
            let matcher = self.read.entry(folder.clone()).or_insert_with(|| {
                let file = folder.join(".gitignore");
                file.is_file().then(|| Gitignore::new(&file).0)
            });
            if matcher.as_ref().is_some_and(|m| m.matched_path_or_any_parents(path, dir).is_ignore()) {
                return true;
            }
            if parts.peek().is_none() {
                break;
            }
            folder.push(part);
        }
        false
    }

    /// The .gitignore in `folder` changed: it is read again when next asked.
    pub(crate) fn forget(&mut self, folder: &Path) {
        self.read.remove(folder);
    }
}

/// Watches `folder` and the folders under it the listing shows, each one by itself, for a platform that watches folder
/// by folder. A folder that went before it could be watched is skipped; it stops when the watch is dropped.
pub(crate) fn follow(watcher: &Weak<Mutex<notify::RecommendedWatcher>>, folder: &Path) {
    use notify::{RecursiveMode, Watcher};
    for dir in watched_dirs(folder) {
        let Some(watcher) = watcher.upgrade() else { return };
        let _ = lock(&watcher).watch(&dir, RecursiveMode::NonRecursive);
    }
}

pub(crate) fn lock<T>(held: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    held.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
