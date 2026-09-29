use std::{
    io::Read as _,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use lathe_project::Command;
use serde_json::{Value, json};

use super::{Access, MAX_RESULT, Tool, ToolContext, ToolResult, object};
use crate::session::ToolKind;

const DEFAULT_TIMEOUT: u64 = 120;
const MAX_TIMEOUT: u64 = 600;
/// The most output kept in memory. A command that prints more keeps the head; the rest is read and dropped
/// so the process never blocks on a full pipe.
const KEEP: usize = 256 * 1024;

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
        // stderr joins stdout, so the model sees the two in the order they were written.
        let script = format!("exec 2>&1\n{line}");
        let command = Command::new("sh").args(["-c", script.as_str()]);
        let mut process = match ctx.project.spawn(&command) {
            Ok(process) => process,
            Err(e) => return ToolResult::err(format!("cannot start the shell: {e}")),
        };
        drop(std::mem::replace(&mut process.stdin, Box::new(std::io::sink())));
        let out = Arc::new(Mutex::new((Vec::<u8>::new(), 0usize)));
        let sink = out.clone();
        let mut stdout = std::mem::replace(&mut process.stdout, Box::new(std::io::empty()));
        let reader = thread::Builder::new().name("lathe-shell-out".into()).spawn(move || {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = stdout.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut kept = sink.lock().unwrap_or_else(|p| p.into_inner());
                let room = KEEP.saturating_sub(kept.0.len());
                kept.0.extend_from_slice(&chunk[..n.min(room)]);
                kept.1 += n;
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
            let _ = process.control.kill();
        }
        let code = process.control.wait().ok().flatten();
        if let Ok(reader) = reader {
            let _ = reader.join();
        }
        let (bytes, total) = {
            let kept = out.lock().unwrap_or_else(|p| p.into_inner());
            (kept.0.clone(), kept.1)
        };
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if total > bytes.len() {
            text.push_str(&format!("\n[output cut: {} of {total} bytes kept]", bytes.len()));
        }
        let text = super::cap(&text, MAX_RESULT);
        match (stopped, code) {
            (Some(why), _) => ToolResult::err(format!("{text}\n[the command was {why} after {:.1} s]", started.elapsed().as_secs_f64())),
            (None, Some(0)) => ToolResult::ok(if text.trim().is_empty() { "(no output; exit code 0)".to_string() } else { format!("{text}\n[exit code 0]") }),
            (None, Some(code)) => ToolResult::err(format!("{text}\n[exit code {code}]")),
            (None, None) => ToolResult::err(format!("{text}\n[the command was ended by a signal]")),
        }
    }
}
