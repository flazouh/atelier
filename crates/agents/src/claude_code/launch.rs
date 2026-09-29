//! The command line that starts `claude` as a session lathe can drive.
use lathe_project::Command;

use super::control::mode_name;
use crate::session::{OpenRequest, PermissionMode};

pub(super) fn command(program: &str, request: &OpenRequest) -> Command {
    let mut args = vec![
        "--print",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        // Permission questions come to lathe as `control_request` lines, not to a terminal. `host`
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
