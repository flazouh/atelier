//! A large pull request for the numbers: many changed files, each with a small edit, and many review
//! threads spread across them. Real git, an in-memory forge.
use std::{path::Path, sync::Arc};

use atelier_forge::{Change, ChangedFile, PullRef};

use super::{FixtureForge, repo::{Repo, put}, sample};
use crate::services::PrConfig;

pub struct Big {
    pub repo: Repo,
    pub forge: Arc<FixtureForge>,
    pub reference: PullRef,
    pub head: String,
    pub files: usize,
    pub comments: usize,
}

fn text(file: usize, edited: bool) -> String {
    (0..40).map(|l| if edited && l == 20 { format!("fn f{file}_{l}() {{ changed() }}\n") } else { format!("fn f{file}_{l}() {{}}\n") }).collect()
}

pub fn path(file: usize) -> String {
    format!("src/m{}/f{file}.rs", file % 20)
}

impl Big {
    /// `files` changed files and `threads` threads of `per_thread` comments each.
    pub fn build(files: usize, threads: usize, per_thread: usize) -> Self {
        let names: Vec<String> = (0..files).map(path).collect();
        let before: Vec<(String, String)> = names.iter().enumerate().map(|(i, p)| (p.clone(), text(i, false))).collect();
        let refs: Vec<(&str, &str)> = before.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
        let repo = Repo::new(&refs);
        let base = repo.main_tip();
        let head = repo.pull(7, "main", &[&|r: &Path| {
            for (i, p) in names.iter().enumerate() {
                put(r, p, &text(i, true));
            }
        }]);
        let changed: Vec<ChangedFile> = names.iter().map(|p| ChangedFile { path: p.clone(), additions: 1, deletions: 1, change: Change::Modified }).collect();
        let mut data = sample::data(7, &head, changed);
        data.pull.as_mut().unwrap().base_sha = base;
        data.threads = (0..threads)
            .map(|t| {
                let comments = (0..per_thread)
                    .map(|c| sample::comment(&format!("c{t}_{c}"), "Ada", "A reasonably long review comment that says something about this line and why.", sample::NOW - 600))
                    .collect();
                sample::thread(&format!("T{t}"), &names[t % files.max(1)], 21, comments)
            })
            .collect();
        let reference = data.reference.clone();
        let forge = Arc::new(FixtureForge::new().with_pull(data));
        Self { repo, forge, reference, head, files, comments: threads * per_thread }
    }

    pub fn config(&self, local: &Path) -> PrConfig {
        PrConfig::new("alex", local).remote_data(self.repo.data.to_str().unwrap()).fetch_url(self.repo.origin.to_str().unwrap())
    }
}
