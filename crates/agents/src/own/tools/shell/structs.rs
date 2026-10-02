use std::{
    io::Read as _,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use atelier_project::Command;
use serde_json::{Value, json};

use super::super::{Access, MAX_RESULT, Tool, ToolContext, ToolResult, object};
use crate::session::ToolKind;
use super::types::{DEFAULT_TIMEOUT, KEEP, MAX_TIMEOUT};

/// What the command wrote, kept up to [`KEEP`] bytes, with its group id taken off the front.
#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    total: usize,
    /// The first line, until it is whole.
    pub(super) head: Vec<u8>,
    seen_head: bool,
    group: Option<String>,
}

impl Output {
    fn take(&mut self, mut chunk: &[u8]) {
        if !self.seen_head {
            self.head.extend_from_slice(chunk);
            let Some(end) = self.head.iter().position(|b| *b == b'\n') else { return };
            self.seen_head = true;
            let head = std::mem::take(&mut self.head);
            let line = String::from_utf8_lossy(&head[..end]).into_owned();
            match line.strip_prefix("@@atelier-group ") {
                Some(group) => self.group = Some(group.trim().to_string()),
                // Not the word we expect: it is the command's output, whole.
                None => self.push(&head[..=end]),
            }
            let rest = head[end + 1..].to_vec();
            self.push(&rest);
            return;
        }
        if chunk.is_empty() {
            chunk = &[];
        }
        self.push(chunk);
    }

    fn push(&mut self, chunk: &[u8]) {
        let room = KEEP.saturating_sub(self.bytes.len());
        self.bytes.extend_from_slice(&chunk[..chunk.len().min(room)]);
        self.total += chunk.len();
    }
}

pub struct Shell;

impl Tool for Shell {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Runs a shell command (`sh -c`) in the project's folder and returns its output (stdout and stderr together) and exit code. A command that runs longer than `timeout_secs` is stopped. Do not start commands that wait for input or run forever."
    }

    fn schema(&self) -> Value {
        object(
            json!({
                "command": {"type": "string", "description": "The command line."},
                "timeout_secs": {"type": "integer", "description": "How long it may run. 120 by default, 600 at most."}
            }),
            &["command"],
        )
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Shell
    }

    fn access(&self) -> Access {
        Access::Execute
    }

    fn file(&self, _: &Value) -> Option<String> {
        None
    }

    fn run(&self, ctx: &ToolContext<'_>, input: &Value) -> ToolResult {
        let Some(line) = input["command"].as_str().filter(|c| !c.trim().is_empty()) else { return ToolResult::err("`command` is required") };
        let timeout = Duration::from_secs(input["timeout_secs"].as_u64().unwrap_or(DEFAULT_TIMEOUT).clamp(1, MAX_TIMEOUT));
        // The shell's first word is its own process id, so that stopping the command can stop everything it
        // started, and not only the shell (see below). stderr joins stdout, so the model sees the two in the
        // order they were written.
        const SCRIPT: &str = "printf '@@atelier-group %s\\n' \"$$\"\nexec 2>&1\nexec sh -c \"$1\"";
        let command = Command::new("sh").args(["-c", SCRIPT, "sh", line]);
        let mut process = match ctx.project.spawn(&command) {
            Ok(process) => process,
            Err(e) => return ToolResult::err(format!("cannot start the shell: {e}")),
        };
        drop(std::mem::replace(&mut process.stdin, Box::new(std::io::sink())));
        let out = Arc::new(Mutex::new(Output::default()));
        let sink = out.clone();
        let mut stdout = std::mem::replace(&mut process.stdout, Box::new(std::io::empty()));
        let reader = thread::Builder::new().name("atelier-shell-out".into()).spawn(move || {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = stdout.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap_or_else(|p| p.into_inner()).take(&chunk[..n]);
            }
        });
        let started = Instant::now();
        let mut stopped = None;
        while process.control.running() {
            if ctx.cancel.is_set() {
                stopped = Some("stopped by the user");
                break;
            }
            if started.elapsed() > timeout {
                stopped = Some("timed out");
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if stopped.is_some() {
            // The shell says who it is as its first act; a stop that comes at once waits a moment for it.
            let wait_until = Instant::now() + Duration::from_millis(500);
            let mut root = out.lock().unwrap_or_else(|p| p.into_inner()).group.clone();
            while root.is_none() && Instant::now() < wait_until && process.control.running() {
                thread::sleep(Duration::from_millis(5));
                root = out.lock().unwrap_or_else(|p| p.into_inner()).group.clone();
            }
            if let Some(root) = root.filter(|g| g.chars().all(|c| c.is_ascii_digit())) {
                // Children first, then the shell, through the project so it reaches an SSH host too. Without
                // `pgrep` on the host only the shell stops.
                let tree = "k() { for c in $(pgrep -P \"$1\" 2>/dev/null); do k \"$c\"; done; kill -KILL \"$1\" 2>/dev/null; }; k \"$1\"";
                if let Ok(mut killer) = ctx.project.spawn(&Command::new("sh").args(["-c", tree, "sh", root.as_str()])) {
                    let _ = killer.control.wait();
                }
            }
            let _ = process.control.kill();
        }
        let code = process.control.wait().ok().flatten();
        if let Ok(reader) = reader {
            // A process that got away from the kill may hold the pipe; do not wait for it for long.
            let give_up = Instant::now() + Duration::from_secs(1);
            while !reader.is_finished() && Instant::now() < give_up {
                thread::sleep(Duration::from_millis(5));
            }
            if reader.is_finished() {
                let _ = reader.join();
            }
        }
        let (bytes, total) = {
            let kept = out.lock().unwrap_or_else(|p| p.into_inner());
            (kept.bytes.clone(), kept.total)
        };
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if total > bytes.len() {
            text.push_str(&format!("\n[output cut: {} of {total} bytes kept]", bytes.len()));
        }
        let text = super::super::cap(&text, MAX_RESULT);
        match (stopped, code) {
            (Some(why), _) => ToolResult::err(format!("{text}\n[the command was {why} after {:.1} s]", started.elapsed().as_secs_f64())),
            (None, Some(0)) => ToolResult::ok(if text.trim().is_empty() { "(no output; exit code 0)".to_string() } else { format!("{text}\n[exit code 0]") }),
            (None, Some(code)) => ToolResult::err(format!("{text}\n[exit code {code}]")),
            (None, None) => ToolResult::err(format!("{text}\n[the command was ended by a signal]")),
        }
    }
}
