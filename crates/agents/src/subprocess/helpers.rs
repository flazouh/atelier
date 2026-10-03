use std::io::{self, BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};

use atelier_project::{Command, Process, Project};

use crate::session::SessionError;
use super::types::{CANCELLED, STDERR_LINES};

/// Starts `command` in the project. A program the host does not have is [`SessionError::Missing`].
pub fn start(project: &dyn Project, command: &Command) -> Result<Process, SessionError> {
    project.spawn(command).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => SessionError::Missing { program: command.program.display().to_string() },
        _ => SessionError::Start(error.to_string()),
    })
}

/// The lines of a stream, each as text (bytes that are not UTF-8 become U+FFFD) without its line end.
/// A read error ends the stream.
pub fn lines(stream: impl Read) -> impl Iterator<Item = String> {
    let mut reader = BufReader::with_capacity(64 * 1024, stream);
    let mut bytes = Vec::new();
    std::iter::from_fn(move || {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => None,
            Ok(_) => {
                let text = String::from_utf8_lossy(&bytes);
                Some(text.trim_end_matches(['\n', '\r']).to_string())
            }
        }
    })
}

/// The last `STDERR_LINES` lines of `stderr`, trailing blank lines dropped.
pub fn stderr_tail(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.trim_end().lines().collect();
    lines[lines.len().saturating_sub(STDERR_LINES)..].join("\n")
}

/// Why the agent's process ended, for a turn it cut short: its exit code (`None` for a signal) and the
/// last line of `tail` that says something.
pub fn exit_why(code: Option<i32>, tail: &str) -> String {
    let how = match code {
        Some(code) => format!("the agent exited with code {code}"),
        None => "the agent was stopped by a signal".to_string(),
    };
    match tail.lines().rev().find(|line| !line.trim().is_empty()).map(str::trim) {
        Some(line) => format!("{how}: {line}"),
        None => how,
    }
}

/// Runs `command` to the end and returns what it wrote to stdout.
pub fn output(project: &dyn Project, command: &Command) -> Result<String, SessionError> {
    let Process { stdin, stdout, mut control } = start(project, command)?;
    drop(stdin);
    let mut text = String::new();
    for line in lines(stdout) {
        text.push_str(&line);
        text.push('\n');
    }
    let _ = control.wait();
    Ok(text)
}

/// How often a running command is checked for a cancel.
const CANCEL_CHECK: std::time::Duration = std::time::Duration::from_millis(100);

/// Runs `command` to the end, for a command that does its work by running (a sign-in): done when it exits with code 0,
/// else the reason, in words for the reader: the program is missing, or how it ended and the last thing it wrote to
/// stderr. Setting `cancelled` kills it, and the reason is `cancelled`.
pub fn run(project: &dyn Project, command: &Command, cancelled: &AtomicBool) -> Result<(), String> {
    let Process { stdin, stdout, mut control } = start(project, command).map_err(|error| match error {
        SessionError::Start(why) => why,
        other => other.to_string(),
    })?;
    drop(stdin);
    // Not joined: a child of the command can keep the pipe open past the command's end.
    std::thread::spawn(move || lines(stdout).for_each(drop));
    while control.running() {
        if cancelled.load(Ordering::SeqCst) {
            let _ = control.kill();
            let _ = control.wait();
            return Err(CANCELLED.into());
        }
        std::thread::sleep(CANCEL_CHECK);
    }
    let code = control.wait().map_err(|error| error.to_string())?;
    match code {
        Some(0) => Ok(()),
        _ => {
            let how = code.map_or_else(|| "it was stopped by a signal".to_string(), |code| format!("it exited with code {code}"));
            let tail = stderr_tail(&control.stderr());
            Err(match tail.lines().rev().find(|line| !line.trim().is_empty()).map(str::trim) {
                Some(line) => format!("{how}: {line}"),
                None => how,
            })
        }
    }
}
