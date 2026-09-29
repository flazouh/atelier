//! A real git repository on disk, for tests, the story and the numbers: a bare "forge" repository, a project cloned from it, and pull requests
//! made in a scratch clone and pushed as `refs/pull/N/head`, the way GitHub keeps them. The project never
//! has the pull request's objects, so `prepare` must fetch them.
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use lathe_forge::{Pull, PullRef};
use lathe_project::{LocalProject, Project};

use crate::{fixture::sample, git::PrGit};

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Rui")
        .env("GIT_AUTHOR_EMAIL", "rui@example.test")
        .env("GIT_COMMITTER_NAME", "Rui")
        .env("GIT_COMMITTER_EMAIL", "rui@example.test")
        .env("GIT_AUTHOR_DATE", "2026-09-01T10:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-09-01T10:00:00Z")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Whether git succeeds, for a question that may be answered no.
pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").args(args).current_dir(dir).output().unwrap().status.success()
}

/// The repositories live in a folder of their own, removed when this is dropped.
pub struct Repo {
    dir: PathBuf,
    pub origin: PathBuf,
    /// The reader's project: cloned from the origin, at main.
    pub work: PathBuf,
    pub data: PathBuf,
    scratch: PathBuf,
}

impl Repo {
    /// `files` are on main.
    pub fn new(files: &[(&str, &str)]) -> Self {
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("lathe-pr-view-{}-{}", std::process::id(), COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let root = std::fs::canonicalize(&dir).unwrap();
        let origin = root.join("origin.git");
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "--bare", "-q", "-b", "main"]);
        let scratch = root.join("scratch");
        git(&root, &["clone", "-q", origin.to_str().unwrap(), "scratch"]);
        git(&scratch, &["checkout", "-q", "-b", "main"]);
        for (path, text) in files {
            put(&scratch, path, text);
        }
        git(&scratch, &["add", "-A"]);
        git(&scratch, &["commit", "-q", "-m", "Start"]);
        git(&scratch, &["push", "-q", "origin", "main"]);
        git(&root, &["clone", "-q", origin.to_str().unwrap(), "work"]);
        let data = root.join("data");
        Self { dir, origin, work: root.join("work"), data, scratch }
    }

    /// The scratch clone the pull requests are made in.
    pub fn scratch(&self) -> &Path {
        &self.scratch
    }

    /// The folder that holds them all.
    pub fn root(&self) -> &Path {
        &self.dir
    }

    pub fn main_tip(&self) -> String {
        git(&self.scratch, &["rev-parse", "HEAD"])
    }

    /// Makes a pull request: a branch off `from` (default main) with `edit` applied and committed once
    /// per closure, pushed as `refs/pull/N/head`. Returns the head.
    pub fn pull(&self, number: u64, from: &str, commits: &[&dyn Fn(&Path)]) -> String {
        git(&self.scratch, &["checkout", "-q", "-B", &format!("pr-{number}"), from]);
        for (i, edit) in commits.iter().enumerate() {
            edit(&self.scratch);
            git(&self.scratch, &["add", "-A"]);
            git(&self.scratch, &["commit", "-q", "--allow-empty", "-m", &format!("Change {} of #{number}", i + 1)]);
        }
        let head = git(&self.scratch, &["rev-parse", "HEAD"]);
        git(&self.scratch, &["push", "-q", "origin", &format!("+HEAD:refs/pull/{number}/head")]);
        git(&self.scratch, &["checkout", "-q", "main"]);
        head
    }

    /// Moves main on, as others merging do.
    pub fn advance_main(&self, path: &str, text: &str) -> String {
        git(&self.scratch, &["checkout", "-q", "main"]);
        put(&self.scratch, path, text);
        git(&self.scratch, &["add", "-A"]);
        git(&self.scratch, &["commit", "-q", "-m", "Main moves on"]);
        git(&self.scratch, &["push", "-q", "origin", "main"]);
        self.main_tip()
    }

    /// Pushes a new head for the pull request, as a force push does.
    pub fn force_push(&self, number: u64, from: &str, edit: &dyn Fn(&Path)) -> String {
        self.pull(number, from, &[edit])
    }

    pub fn project(&self) -> Arc<dyn Project> {
        Arc::new(LocalProject::open(&self.work).unwrap().with_data_dir(&self.root().join("project-data")))
    }

    pub fn prgit(&self) -> PrGit {
        PrGit::new(self.project(), self.data.to_str().unwrap()).with_remote(self.origin.to_str().unwrap())
    }

    /// The pull request as the forge would tell it.
    pub fn pull_data(&self, number: u64, head: &str, base_sha: &str) -> (PullRef, Pull) {
        let reference = sample::reference(number);
        let mut pull = sample::pull(&reference, head);
        pull.base_sha = base_sha.to_string();
        (reference, pull)
    }
}

pub fn put(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, text).unwrap();
}

pub fn remove(root: &Path, path: &str) {
    std::fs::remove_file(root.join(path)).unwrap();
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
