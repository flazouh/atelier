use std::io::{self, BufRead, BufReader, Read};

use atelier_project::{Command, Process, Project};

use crate::session::SessionError;
use super::types::STDERR_LINES;

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

/// `text` without the terminal's colour and cursor codes. A program that thinks it writes to a terminal paints its errors
/// with them (`ESC [ 36 m`), and they are not words: a row that shows them shows `[36m`.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // A control sequence: parameters, then one final byte from `@` to `~`.
            Some('[') => {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            // An operating-system command (a title, a link): up to the bell or `ESC \`.
            Some(']') => {
                chars.next();
                while let Some(next) = chars.next() {
                    if next == '\u{7}' || (next == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            // Any other escape is the escape and the one character after it.
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// The last `STDERR_LINES` lines of `stderr`, trailing blank lines dropped, and with no colour codes.
pub fn stderr_tail(stderr: &str) -> String {
    let clean = strip_ansi(stderr);
    let lines: Vec<&str> = clean.trim_end().lines().collect();
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
