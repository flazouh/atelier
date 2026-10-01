use atelier_agents::commands::{
    CommandInfo,
    CommandSource,
    atelier_commands,
    command_file_from,
    merge,
    skill_from,
};
use atelier_project::Project;

use super::structs::Lists;
use super::types::{MOST_FILES, RUNNING};

/// Reads the skills, the command files and the tracked files of `project`. What cannot be read is left out.
pub fn read(project: &dyn Project) -> Lists {
    let root = project.root().display().to_string();
    let mut project_commands = Vec::new();
    for entry in project.read_dir(&format!("{root}/.claude/skills")).unwrap_or_default() {
        let path = format!(".claude/skills/{}/SKILL.md", entry.name);
        if let Some(info) = entry.dir.then(|| read_text(project, &path)).flatten().and_then(|text| skill_from(&path, &text)) {
            project_commands.push(info);
        }
    }
    for entry in project.read_dir(&format!("{root}/.claude/commands")).unwrap_or_default() {
        let path = format!(".claude/commands/{}", entry.name);
        if let Some(info) = (!entry.dir).then(|| read_text(project, &path)).flatten().and_then(|text| command_file_from(&path, &text)) {
            project_commands.push(info);
        }
    }
    let files = match project.git(&["ls-files"]) {
        Ok(out) if out.ok() => out.stdout.lines().take(MOST_FILES).map(str::to_string).collect(),
        _ => Vec::new(),
    };
    Lists { project_commands, files }
}

pub(super) fn read_text(project: &dyn Project, path: &str) -> Option<String> {
    String::from_utf8(project.read(path).ok()?).ok()
}

/// The list the composer offers: atelier's running commands, the project's, then the agent's own.
pub fn commands(project: Vec<CommandInfo>, agent: &[String]) -> Vec<CommandInfo> {
    let atelier = atelier_commands().into_iter().filter(|c| RUNNING.contains(&c.name.as_str())).collect();
    merge(atelier, project, agent)
}

/// Whether atelier runs `name` itself, rather than the agent.
pub fn atelier_runs(name: &str) -> bool {
    atelier_commands().iter().any(|c| c.name == name && c.source == CommandSource::Atelier)
}

/// The text the agent gets for a command it runs: `/name args`.
pub fn agent_text(name: &str, args: &str) -> String {
    if args.is_empty() { format!("/{name}") } else { format!("/{name} {args}") }
}
