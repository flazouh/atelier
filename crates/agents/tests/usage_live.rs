//! Reads the real Claude and Codex allowances of whoever runs it, through a local project, and prints how much of
//! each window is used (never the token). Ignored unless asked for, since it needs a sign-in and the network:
//! `cargo test -p atelier-agents --test usage_live -- --ignored --nocapture`
use atelier_agents::usage::{ClaudeUsage, CodexUsage, UsageSource};
use atelier_project::LocalProject;

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

fn show(source: &dyn UsageSource) {
    let dir = tempfile::tempdir().unwrap();
    let project = LocalProject::open(dir.path()).unwrap();
    match source.read(&project, now()) {
        Ok(reading) => println!("{}: {reading:?}", source.name()),
        Err(why) => println!("{}: {why}", source.name()),
    }
}

#[test]
#[ignore = "reads the real allowances"]
fn the_real_allowances_are_read() {
    show(&ClaudeUsage);
    show(&CodexUsage::new("codex"));
}

#[test]
#[ignore = "asks Anthropic for the models of the Claude sign-in"]
fn the_real_claude_models_are_listed() {
    use atelier_agents::{claude_code::ClaudeCode, session::Backend};
    let dir = tempfile::tempdir().unwrap();
    let project = LocalProject::open(dir.path()).unwrap();
    match ClaudeCode::new().list_models(&project, now()) {
        Some(Ok(models)) => {
            for model in &models {
                println!("{} | {}", model.id, model.label);
            }
            assert!(models.iter().any(|m| m.label.starts_with("Opus")), "an Opus is listed");
        }
        other => panic!("the list: {other:?}"),
    }
}
