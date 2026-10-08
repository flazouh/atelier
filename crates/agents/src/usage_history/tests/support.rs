use std::fs;
use std::path::{Path, PathBuf};

use crate::usage_history::{Provider, Roots};

/// One Claude `assistant` line. `usage` is (input, output, cache_write, cache_read).
pub(super) fn assistant(id: &str, request: &str, model: &str, ts: &str, usage: (u64, u64, u64, u64)) -> String {
    format!(
        r#"{{"type":"assistant","timestamp":"{ts}","sessionId":"s","cwd":"/home/a/projA","requestId":"{request}","message":{{"id":"{id}","model":"{model}","usage":{{"input_tokens":{},"output_tokens":{},"cache_creation_input_tokens":{},"cache_read_input_tokens":{}}},"content":[{{"type":"text","text":"hi"}}]}}}}"#,
        usage.0, usage.1, usage.2, usage.3
    )
}

pub(super) fn user(text: &str) -> String {
    format!(
        r#"{{"type":"user","timestamp":"2026-09-25T09:59:00.000Z","cwd":"/home/a/projA","message":{{"role":"user","content":{}}}}}"#,
        serde_json::to_string(text).unwrap()
    )
}

/// Writes `<config>/projects/<project>/<session>.jsonl`.
pub(super) fn claude_session(config: &Path, project: &str, session: &str, lines: &[String]) -> PathBuf {
    let dir = config.join("projects").join(project);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{session}.jsonl"));
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

pub(super) fn claude_roots(config: &Path) -> Roots {
    Roots::new().with_dir(Provider::Claude, config)
}

pub(super) fn config_dir(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    dir
}
