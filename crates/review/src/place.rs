//! Where a reviewed file lives. A path in a review is relative to the project's folder, or absolute when the agent
//! changed a file beyond it (`/tmp/test-file.txt`): a relative path never starts with `/`, so the first character says
//! which place. Reading, writing and removing go through here, so the rest of the review never asks.
//!
//! A file beyond the folder is touched only when the review names it, and the review names only files an agent changed
//! in the turn.
use std::io;

use atelier_project::Project;

/// Whether `path` names a file beyond the project's folder.
pub fn is_outside(path: &str) -> bool {
    path.starts_with('/')
}

pub fn read(project: &dyn Project, path: &str) -> io::Result<Vec<u8>> {
    if is_outside(path) { project.read_outside(path) } else { project.read(path) }
}

pub fn write(project: &dyn Project, path: &str, bytes: &[u8]) -> io::Result<()> {
    if is_outside(path) { project.write_outside(path, bytes) } else { project.write(path, bytes) }
}

pub fn remove(project: &dyn Project, path: &str) -> io::Result<()> {
    if is_outside(path) { project.remove_outside(path) } else { project.remove(path) }
}

#[cfg(test)]
mod tests;
