use std::path::{Path, PathBuf};

use crate::usage_history::structs::{AccountRoot, Roots};
use crate::usage_history::types::Provider;

impl Roots {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accounts(&self) -> &[AccountRoot] {
        &self.accounts
    }

    /// Adds a folder (kept as given, even when it does not exist yet). A folder already added is not added twice.
    pub fn with_dir(mut self, provider: Provider, dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let key = canonical(&dir);
        if self.accounts.iter().any(|a| a.provider == provider && canonical(&a.dir) == key) {
            return self;
        }
        let label = label_of(&dir);
        self.accounts.push(AccountRoot { provider, label, dir });
        self
    }

    /// `~/.claude` and every other `~/.claude*` folder with a `projects/` folder, `~/.codex` when it has
    /// `sessions/`, and the folder named by `claude_config_dir` (the `CLAUDE_CONFIG_DIR` variable) when it exists.
    pub fn detect(home: &Path, claude_config_dir: Option<&Path>) -> Self {
        let mut found: Vec<(Provider, PathBuf)> = Vec::new();
        if let Ok(read) = std::fs::read_dir(home) {
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let path = entry.path();
                if name.starts_with(".claude") && path.join("projects").is_dir() {
                    found.push((Provider::Claude, path));
                } else if name == ".codex" && path.join("sessions").is_dir() {
                    found.push((Provider::Codex, path));
                }
            }
        }
        found.sort_by(|a, b| a.1.cmp(&b.1));
        let mut roots = found.into_iter().fold(Self::new(), |r, (p, d)| r.with_dir(p, d));
        if let Some(dir) = claude_config_dir.filter(|d| d.is_dir()) {
            roots = roots.with_dir(Provider::Claude, dir);
        }
        roots
    }

    /// [`Roots::detect`] with `$HOME` and `$CLAUDE_CONFIG_DIR`.
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let config = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()).map(PathBuf::from);
        match home {
            Some(home) => Self::detect(&home, config.as_deref()),
            None => match config {
                Some(dir) => Self::new().with_dir(Provider::Claude, dir),
                None => Self::new(),
            },
        }
    }
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn label_of(dir: &Path) -> String {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let label = name.trim_start_matches('.');
    if label.is_empty() { "account".to_owned() } else { label.to_owned() }
}
