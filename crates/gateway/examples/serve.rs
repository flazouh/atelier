//! A gateway over a memory provider with two tasks, for the live smoke test (`tools/gateway-smoke.sh`).
//!
//! `cargo run -p atelier-gateway --example serve -- <file> [seconds]` writes the path of an MCP config to `<file>`,
//! then serves for `seconds` (60 when left out) and stops. Nothing here touches a real tracker.
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use atelier_capabilities::{
    Actor, Registry,
    tasks::{MemoryTasks, NewTask, TasksProvider},
};
use atelier_gateway::{Gateway, TasksTools, ToolSet};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("the file to write the config path to");
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60);

    let memory = Arc::new(MemoryTasks::new("smoke"));
    let me = Actor::person("alex", "Alex");
    for title in ["Write the release notes", "Fix the login on Safari"] {
        memory.create(&NewTask::titled(title), &me).expect("a seeded task");
    }
    let mut registry = Registry::new();
    registry.add_tasks(memory);
    let tools: Arc<dyn ToolSet> = Arc::new(TasksTools::new(Arc::new(RwLock::new(registry))));
    let gateway = Gateway::start(vec![tools])?;
    let dir = std::env::temp_dir().join(format!("atelier-smoke-{}", std::process::id()));
    let grant = gateway.grant(Actor::agent("claude-code", "Claude Code", "alex"), &dir)?;
    std::fs::write(&out, grant.config_path().to_string_lossy().as_bytes())?;
    std::thread::sleep(Duration::from_secs(seconds));
    drop(grant);
    let _ = std::fs::remove_dir(&dir);
    Ok(())
}
