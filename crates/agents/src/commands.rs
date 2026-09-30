//! The commands a composer offers after `/`, from three sources, as neutral data: the agent's own (its
//! init lists them), lathe's own, and the project's skills (`.claude/skills/*/SKILL.md`) and command files
//! (`.claude/commands/*.md`). A command lathe runs itself is lathe's even when the agent lists it too:
//! the agent runs headless and cannot run `/login`, for one.

/// Where a command comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandSource {
    /// The agent's own: sent to it as `/name args`.
    Agent,
    /// Lathe runs it.
    Lathe,
    /// A skill or a command file of the project: sent to the agent.
    Skill,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandInfo {
    /// Without the slash.
    pub name: String,
    pub source: CommandSource,
    /// One line of what it does.
    pub summary: String,
    /// What goes after the name, such as "<what the session is for>".
    pub args_hint: Option<String>,
}

impl CommandInfo {
    fn new(name: &str, source: CommandSource, summary: &str, args_hint: Option<&str>) -> Self {
        Self { name: name.into(), source, summary: summary.into(), args_hint: args_hint.map(Into::into) }
    }
}

/// The commands lathe runs itself.
pub fn lathe_commands() -> Vec<CommandInfo> {
    use CommandSource::Lathe;
    vec![
        CommandInfo::new("goal", Lathe, "Set what this session is for, or show it", Some("<the goal>")),
        CommandInfo::new("clear-goal", Lathe, "Clear this session's goal", None),
        CommandInfo::new("login", Lathe, "Sign in to the agent, in a terminal", None),
        CommandInfo::new("review", Lathe, "Review this session's changes", None),
        CommandInfo::new("tasks", Lathe, "Show the project's tasks", None),
        CommandInfo::new("files", Lathe, "Show the Files view", None),
    ]
}

/// The front matter's `key: value`, from a `---` block at the top.
fn front_matter<'a>(text: &'a str, key: &str) -> Option<&'a str> {
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

/// One list, each name once: lathe's first, then the project's, then the agent's own. The agent's list
/// holds the project's skills and commands too; those show as the project's.
pub fn merge(lathe: Vec<CommandInfo>, project: Vec<CommandInfo>, agent: &[String]) -> Vec<CommandInfo> {
    let mut all: Vec<CommandInfo> = Vec::new();
    for command in lathe.into_iter().chain(project) {
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

#[cfg(test)]
mod tests;
