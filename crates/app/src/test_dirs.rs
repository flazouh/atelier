//! Temporary folders for tests that must hand out a plain path (a repository a session works in, a bare remote).
//! The test's thread owns each folder, so it goes when the test ends, whether the test passed or failed. Nothing
//! is left in the temporary folder: `tools/check.sh` counts what is left after the workspace tests.
use std::{cell::RefCell, path::PathBuf};

thread_local! {
    static OWNED: RefCell<Vec<tempfile::TempDir>> = const { RefCell::new(Vec::new()) };
}

/// A new empty folder that lives until the calling test thread ends.
pub fn path() -> PathBuf {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = dir.path().to_path_buf();
    OWNED.with(|owned| owned.borrow_mut().push(dir));
    path
}

#[cfg(test)]
mod tests;
