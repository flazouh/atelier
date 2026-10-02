use std::{io::Read, path::PathBuf, sync::Arc};

use atelier_ui::PrChipData;
use atelier_lsp::Workers;
use atelier_pr_view::services::{PrConfig, Services};
use atelier_project::{Command, Project};

/// The reader's GitHub login, from `gh` on the project's host; `None` when gh is missing or signed out.
pub fn login(project: &dyn Project) -> Option<String> {
    let mut process = project.spawn(&Command::new("gh").args(["api", "user", "--jq", ".login"])).ok()?;
    let mut out = String::new();
    process.stdout.read_to_string(&mut out).ok()?;
    let login = out.trim();
    (!login.is_empty()).then(|| login.to_string())
}

/// The view's services for `project`, read-only, with the project's language servers. Blocking: it
/// opens the reader's database and the caches in `local_data`.
/// The list holds only `repo`'s pull requests: the project's own.
pub fn open_services(project: Arc<dyn Project>, me: String, local_data: PathBuf, workers: Arc<Workers>, repo: atelier_forge::RepoRef) -> Result<Arc<Services>, String> {
    let forge = Arc::new(atelier_forge::github::GitHub::new(project.clone()));
    let config = PrConfig::new(me, local_data).read_only(true).workers(workers).repo(repo);
    Services::open(project, forge, config)
}

/// The pull requests a `#N` in an agent's text can name: those of the project's own repository
/// (`owner/name`). While the repository is not known, those whose number no other repository holds, so
/// a `#N` never opens whichever came first.
pub fn chips_of(rows: impl IntoIterator<Item = PrChipData>, repo: Option<&str>) -> Vec<PrChipData> {
    let rows: Vec<PrChipData> = rows.into_iter().collect();
    match repo {
        Some(repo) => rows.into_iter().filter(|chip| chip.repo.as_ref() == repo).collect(),
        None => {
            let held_once = |number: u64| rows.iter().filter(|c| c.number == number).count() == 1;
            rows.iter().filter(|chip| held_once(chip.number)).cloned().collect()
        }
    }
}
