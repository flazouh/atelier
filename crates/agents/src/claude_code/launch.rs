//! The command line that starts `claude` as a session atelier can drive.
use atelier_project::Command;

use super::control::mode_name;
use crate::session::{OpenRequest, PermissionMode};

/// The command line for one piece of text: `claude --print` with the prompt on stdin, the answer as
/// plain text on stdout, in Plan mode (it changes nothing) and with no session saved, so it never shows
/// among the project's sessions.
pub(super) fn draft_command(program: &str, model: Option<&str>) -> Command {
    let mut args: Vec<String> = ["--print", "--no-session-persistence", "--output-format", "text", "--permission-mode", "plan"]
        .into_iter()
        .map(String::from)
        .collect();
    if let Some(model) = model {
        args.extend(["--model".into(), model.to_string()]);
    }
    Command::new(program).args(args)
}

pub(super) fn command(program: &str, request: &OpenRequest) -> Command {
    let mut args = vec![
        "--print",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        // Permission questions come to atelier as `control_request` lines, not to a terminal. `host`
        // alone denies them; the tool flag (hidden from `--help`) makes `claude` ask over stdio.
        "--permission-prompts",
        "host",
        "--permission-prompt-tool",
        "stdio",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    if let Some(session) = &request.resume {
        args.extend(["--resume".into(), session.as_str().into()]);
    }
    if let Some(model) = &request.model {
        args.extend(["--model".into(), model.clone()]);
    }
    // `Ask` is what `claude` does with no flag.
    if let Some(mode) = request.mode.filter(|mode| *mode != PermissionMode::Ask) {
        args.extend(["--permission-mode".into(), mode_name(mode).into()]);
    }
    Command::new(program).args(args)
}
