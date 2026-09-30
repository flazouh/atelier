use super::*;

#[test]
fn lathe_offers_its_own_commands() {
    let commands = lathe_commands();
    let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
    for name in ["goal", "clear-goal", "login", "review", "tasks", "files"] {
        assert!(names.contains(&name), "{name} in {names:?}");
    }
}

#[test]
fn a_skill_takes_its_name_and_description_from_its_front_matter() {
    let text = "---\nname: pdf-tools\ndescription: Read and fill PDF forms\n---\n\nBody.";
    let skill = skill_from(".claude/skills/pdf/SKILL.md", text).unwrap();
    assert_eq!((skill.name.as_str(), skill.summary.as_str(), skill.source), ("pdf-tools", "Read and fill PDF forms", CommandSource::Skill));
    let bare = skill_from(".claude/skills/notes/SKILL.md", "No front matter.").unwrap();
    assert_eq!(bare.name, "notes", "with no name, the folder names it");
}

#[test]
fn a_project_command_takes_its_file_name_and_its_description() {
    let text = "---\ndescription: Run the release checklist\n---\nDo the steps.";
    let command = command_file_from(".claude/commands/release.md", text).unwrap();
    assert_eq!((command.name.as_str(), command.summary.as_str()), ("release", "Run the release checklist"));
    let plain = command_file_from(".claude/commands/tidy.md", "Tidy the imports.\nMore.").unwrap();
    assert_eq!(plain.summary, "Tidy the imports.", "with no description, its first line");
    assert!(command_file_from(".claude/commands/readme.txt", "x").is_none(), "only .md files");
}

#[test]
fn the_list_has_each_name_once_and_lathe_answers_for_its_own() {
    let skills = vec![skill_from(".claude/skills/pdf/SKILL.md", "---\nname: pdf\n---").unwrap()];
    let agent = vec!["compact".to_string(), "login".to_string(), "pdf".to_string(), "review".to_string()];
    let all = merge(lathe_commands(), skills, &agent);
    let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
    for name in ["compact", "login", "pdf", "review"] {
        assert_eq!(names.iter().filter(|n| **n == name).count(), 1, "{name} once in {names:?}");
    }
    let login = all.iter().find(|c| c.name == "login").unwrap();
    assert_eq!(login.source, CommandSource::Lathe, "lathe runs /login, the agent cannot headless");
    let compact = all.iter().find(|c| c.name == "compact").unwrap();
    assert_eq!(compact.source, CommandSource::Agent);
    let pdf = all.iter().find(|c| c.name == "pdf").unwrap();
    assert_eq!(pdf.source, CommandSource::Skill, "a skill the agent also lists is shown as the skill");
}
