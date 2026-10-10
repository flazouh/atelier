use crate::enums::{Body, Colour, Harness, Tool, Voice};
use crate::structs::{Bot, BotId, FaceChoice, Playbook, PlaybookStep, Provider};
use crate::traits::BotStore;

fn id(text: &str) -> BotId {
    text.parse().expect("a starter id is good")
}

struct Spec {
    id: &'static str,
    name: &'static str,
    role: &'static str,
    job: &'static str,
    face: (Body, Colour, Tool),
    harness: Harness,
    voice: Voice,
    skills: &'static [&'static str],
}

const SPECS: [Spec; 10] = [
    Spec {
        id: "nimbus",
        name: "Nimbus",
        role: "Planner",
        job: "Turns an idea into a spec and a QA matrix.",
        face: (Body::Feet, Colour::Yellow, Tool::Flag),
        harness: Harness::ClaudeCode,
        voice: Voice::CalmAndClear,
        skills: &[
            "eng-plan",
            "eng-grill",
            "eng-issues",
            "eng-handoff",
            "eng-velocity",
        ],
    },
    Spec {
        id: "bolt",
        name: "Bolt",
        role: "Builder",
        job: "Writes the code, test first.",
        face: (Body::Tank, Colour::Blue, Tool::Arm),
        harness: Harness::ClaudeCode,
        voice: Voice::CalmAndClear,
        skills: &[
            "eng-tdd",
            "eng-architecture",
            "eng-prototype",
            "eng-parallel",
            "stack-*",
        ],
    },
    Spec {
        id: "pip",
        name: "Pip",
        role: "Prover",
        job: "Checks the real build, never the Builder.",
        face: (Body::Wheels, Colour::Green, Tool::Lens),
        harness: Harness::ClaudeCode,
        voice: Voice::ShortAndDry,
        skills: &[
            "eng-qa",
            "eng-verify",
            "control-browser",
            "control-simulator",
            "control-desktop",
            "control-demo",
        ],
    },
    Spec {
        id: "olive",
        name: "Olive",
        role: "Reviewer",
        job: "Reads the whole change for real defects.",
        face: (Body::Spring, Colour::Purple, Tool::Periscope),
        harness: Harness::Codex,
        voice: Voice::Thorough,
        skills: &[
            "eng-review",
            "eng-quality-review",
            "stack-knip",
            "stack-pre-commit",
        ],
    },
    Spec {
        id: "skip",
        name: "Skip",
        role: "Shipper",
        job: "Lands, releases and tells people.",
        face: (Body::Hover, Colour::Orange, Tool::Parcel),
        harness: Harness::ClaudeCode,
        voice: Voice::Cheerful,
        skills: &[
            "eng-deliver",
            "eng-git",
            "stack-railway",
            "stack-secretctl",
            "comms-slack",
        ],
    },
    Spec {
        id: "dot",
        name: "Dot",
        role: "Debugger",
        job: "Finds the real cause before any fix.",
        face: (Body::Legs, Colour::Pink, Tool::Antennae),
        harness: Harness::ClaudeCode,
        voice: Voice::Thorough,
        skills: &["eng-debug", "eng-chrome-extension"],
    },
    // The faces of the next four are provisional. The faces step gives each its own body and tool.
    Spec {
        id: "quill",
        name: "Quill",
        role: "Researcher",
        job: "Finds what is known and what users say.",
        face: (Body::Feet, Colour::Blue, Tool::Lens),
        harness: Harness::ClaudeCode,
        voice: Voice::Thorough,
        skills: &[
            "research",
            "research-current-art",
            "research-literature",
            "research-user-pain",
        ],
    },
    Spec {
        id: "ink",
        name: "Ink",
        role: "Writer",
        job: "Writes posts, docs and messages in your voice.",
        face: (Body::Wheels, Colour::Purple, Tool::Flag),
        harness: Harness::ClaudeCode,
        voice: Voice::Playful,
        skills: &[
            "content-*",
            "comms-slack-voice",
            "comms-gmail",
            "comms-discord",
        ],
    },
    Spec {
        id: "mimi",
        name: "Mimi",
        role: "Designer",
        job: "Designs screens, motion and diagrams.",
        face: (Body::Hover, Colour::Pink, Tool::Antennae),
        harness: Harness::ClaudeCode,
        voice: Voice::Cheerful,
        skills: &["design-*"],
    },
    Spec {
        id: "gus",
        name: "Gus",
        role: "Operator",
        job: "Runs your tools and keeps the setup in shape.",
        face: (Body::Tank, Colour::Green, Tool::Key),
        harness: Harness::ClaudeCode,
        voice: Voice::ShortAndDry,
        skills: &["control-remote", "eng-skills"],
    },
];

/// The ten bots that ship, one for each role in `docs/bots/bots-v1.md`.
pub fn starter_crew() -> Vec<Bot> {
    SPECS
        .iter()
        .map(|s| Bot {
            id: id(s.id),
            name: s.name.to_string(),
            role: s.role.to_string(),
            job: s.job.to_string(),
            face: FaceChoice {
                body: s.face.0,
                colour: s.face.1,
                tool: s.face.2,
            },
            harness: s.harness,
            provider: Provider {
                service: if s.harness == Harness::Codex {
                    "openai"
                } else {
                    "anthropic"
                }
                .to_string(),
                model: None,
            },
            skills: s.skills.iter().map(|k| k.to_string()).collect(),
            tools: Vec::new(),
            voice: s.voice,
            version: 1,
        })
        .collect()
}

/// The playbooks that ship. Deliver: plan, build, prove, review, ship. The Prover and the Shipper stop to ask first.
pub fn starter_playbooks() -> Vec<Playbook> {
    let step = |bot: &str, asks_first: bool| PlaybookStep {
        bot: id(bot),
        asks_first,
    };
    vec![Playbook {
        id: id("deliver"),
        name: "Deliver".to_string(),
        steps: vec![
            step("nimbus", false),
            step("bolt", false),
            step("pip", false),
            step("olive", false),
            step("skip", true),
        ],
    }]
}

/// Keeps the starter bots and playbooks that the store does not hold yet. A bot or a playbook that is kept, even
/// after an edit, is not touched. Returns how many it added.
pub fn seed_starters(store: &dyn BotStore) -> Result<usize, crate::enums::BotsError> {
    let have: Vec<BotId> = store.bots()?.into_iter().map(|b| b.id).collect();
    let mut added = 0;
    for bot in starter_crew().into_iter().filter(|b| !have.contains(&b.id)) {
        store.save_bot(bot)?;
        added += 1;
    }
    let have: Vec<BotId> = store.playbooks()?.into_iter().map(|p| p.id).collect();
    for playbook in starter_playbooks()
        .into_iter()
        .filter(|p| !have.contains(&p.id))
    {
        store.save_playbook(playbook)?;
        added += 1;
    }
    Ok(added)
}
