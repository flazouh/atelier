//! A real pull request on GitHub, read through the signed-in `gh`, for QA. The project is an empty
//! repository whose `origin` names the real one, so git fetches the pull request into the cache the way it
//! would for a checkout. Nothing is ever written: use it with `PrConfig::read_only(true)`.
use std::{path::Path, process::Command, sync::Arc};

use atelier_forge::{Forge, PullRef, RepoRef, github::GitHub};
use atelier_project::{LocalProject, Project};

/// `owner/name#number`, on github.com.
pub fn parse(spec: &str) -> Result<PullRef, String> {
    let (slug, number) = spec.split_once('#').ok_or("write it as owner/name#number")?;
    let (owner, name) = slug.split_once('/').ok_or("write it as owner/name#number")?;
    let number = number.parse::<u64>().map_err(|_| "the number after # is not a number".to_string())?;
    Ok(PullRef { repo: RepoRef::new("github.com", owner, name), number })
}

pub struct Real {
    pub project: Arc<dyn Project>,
    pub forge: Arc<dyn Forge>,
    pub reference: PullRef,
}

impl Real {
    /// Makes (or reuses) `folder/<name>` as the project and asks GitHub for the pull request.
    pub fn open(spec: &str, folder: &Path) -> Result<Self, String> {
        let reference = parse(spec)?;
        let dir = folder.join(format!("{}-{}", reference.repo.owner, reference.repo.name));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let git = |args: &[&str]| Command::new("git").arg("-C").arg(&dir).args(args).output().map_err(|e| e.to_string());
        if !dir.join(".git").exists() {
            git(&["init", "-q"])?;
            git(&["remote", "add", "origin", &format!("https://github.com/{}.git", reference.repo.slug())])?;
        }
        let project: Arc<dyn Project> = Arc::new(LocalProject::open(&dir).map_err(|e| e.to_string())?);
        let forge: Arc<dyn Forge> = Arc::new(GitHub::new(project.clone()));
        Ok(Self { project, forge, reference })
    }
}
