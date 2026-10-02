use super::structs::CommandInfo;
use super::types::CommandSource;

/// The commands atelier runs itself.
pub fn atelier_commands() -> Vec<CommandInfo> {
    use CommandSource::Atelier;
    vec![
        CommandInfo::new("goal", Atelier, "Set what this session is for, or show it", Some("<the goal>")),
        CommandInfo::new("clear-goal", Atelier, "Clear this session's goal", None),
        CommandInfo::new("login", Atelier, "Sign in to the agent, in a terminal", None),
        CommandInfo::new("review", Atelier, "Review this session's changes", None),
        CommandInfo::new("tasks", Atelier, "Show the project's tasks", None),
        CommandInfo::new("files", Atelier, "Show the Files view", None),
    ]
}

/// The front matter's `key: value`, from a `---` block at the top.
pub(super) fn front_matter<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = text.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    rest[..end].lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"')).filter(|v| !v.is_empty())
    })
}

/// A skill from its `SKILL.md`: the front matter's name, else its folder's; its description.
pub fn skill_from(path: &str, text: &str) -> Option<CommandInfo> {
    let folder = path.rsplit('/').nth(1)?;
    let name = front_matter(text, "name").unwrap_or(folder);
    let summary = front_matter(text, "description").unwrap_or("A skill of this project");
    Some(CommandInfo::new(name, CommandSource::Skill, summary, None))
}

/// A command file (`.claude/commands/<name>.md`): its file's name; its description, else its first line.
pub fn command_file_from(path: &str, text: &str) -> Option<CommandInfo> {
    let file = path.rsplit('/').next()?;
    let name = file.strip_suffix(".md")?;
    let body = match text.strip_prefix("---").and_then(|rest| rest.find("\n---").map(|end| &rest[end + 4..])) {
        Some(body) => body,
        None => text,
    };
    let first = body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("A command of this project");
    let summary = front_matter(text, "description").unwrap_or(first);
    Some(CommandInfo::new(name, CommandSource::Skill, summary, None))
}

/// One list, each name once: atelier's first, then the project's, then the agent's own. The agent's list
/// holds the project's skills and commands too; those show as the project's.
pub fn merge(atelier: Vec<CommandInfo>, project: Vec<CommandInfo>, agent: &[String]) -> Vec<CommandInfo> {
    let mut all: Vec<CommandInfo> = Vec::new();
    for command in atelier.into_iter().chain(project) {
        if !all.iter().any(|c| c.name == command.name) {
            all.push(command);
        }
    }
    for name in agent {
        if !all.iter().any(|c| c.name == *name) {
            all.push(CommandInfo::new(name, CommandSource::Agent, "The agent's own command", None));
        }
    }
    all
}
