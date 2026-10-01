use std::{path::Path, process::Command, sync::Arc};

use atelier_forge::{Forge, PullRef, github::GitHub};
use atelier_project::{LocalProject, Project};

use super::helpers::parse;

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
