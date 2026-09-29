//! A project's pull requests: alex-31's `PrHub` (`crates/pr-view`, `docs/pr-view.md`), in the right pane
//! in place of the editor while it shows. It opens from the project's ⋯ menu ("Pull requests") or
//! ⌘⇧P, and from a PR chip in an agent's text. Read-only until Alex approves a scratch repository:
//! nothing it does is sent to GitHub. Its services (a small database and caches) open off the UI
//! thread, and the reader's login comes from the host's `gh`.

use std::{io::Read, path::PathBuf, sync::Arc};

use gpui_kit::{Entity, Subscription};
use lathe_lsp::Workers;
use lathe_pr_view::{hub::PrHub, services::{PrConfig, Services}};
use lathe_project::{Command, Project};

pub struct Pulls {
    pub hub: Entity<PrHub>,
    /// In the right pane now; the hub keeps its state while hidden.
    pub shown: bool,
    pub _events: Subscription,
}

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
pub fn open_services(project: Arc<dyn Project>, me: String, local_data: PathBuf, workers: Arc<Workers>) -> Result<Arc<Services>, String> {
    let forge = Arc::new(lathe_forge::github::GitHub::new(project.clone()));
    let config = PrConfig::new(me, local_data).read_only(true).workers(workers);
    Services::open(project, forge, config)
}
