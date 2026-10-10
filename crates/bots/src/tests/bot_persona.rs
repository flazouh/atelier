use crate::{Voice, starter_crew};

fn dot() -> crate::Bot {
    starter_crew()
        .into_iter()
        .find(|b| b.id.as_str() == "dot")
        .unwrap()
}

#[test]
fn the_persona_says_the_name_the_role_the_job_the_voice_and_the_skills() {
    assert_eq!(
        dot().persona(),
        "In this session you are Dot, a bot that works for the person you talk to.\n\
         Role: Debugger.\n\
         Job: Finds the real cause before any fix.\n\
         Voice: thorough. Say what you checked and what you found, and leave nothing out.\n\
         Skills: eng-debug, eng-chrome-extension. When a task fits one of these skills and you have it, use it. \
         A name that ends in * stands for every skill whose name starts that way.\n\
         When you say who you are, you are Dot."
    );
}

#[test]
fn a_bot_with_no_job_and_no_skill_gets_no_line_for_them() {
    let mut bot = dot();
    bot.job = "  ".into();
    bot.skills.clear();
    let persona = bot.persona();
    assert!(!persona.contains("Job:") && !persona.contains("Skills:"), "{persona}");
    assert!(persona.contains("Role: Debugger.") && persona.contains("Voice: thorough."), "{persona}");
}

#[test]
fn each_voice_has_its_own_words() {
    let voices = [
        Voice::CalmAndClear,
        Voice::Cheerful,
        Voice::ShortAndDry,
        Voice::Thorough,
        Voice::Playful,
    ];
    let mut lines: Vec<String> = voices
        .into_iter()
        .map(|voice| {
            let mut bot = dot();
            bot.voice = voice;
            let persona = bot.persona();
            persona.lines().find(|l| l.starts_with("Voice: ")).unwrap().to_string()
        })
        .collect();
    lines.sort();
    lines.dedup();
    assert_eq!(lines.len(), 5, "{lines:?}");
}
