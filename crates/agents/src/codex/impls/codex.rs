use atelier_ui::{AgentLook, Mark, PhaseLabels};
use gpui_kit::{Hsla, rgb};

use crate::{
    acp::AcpAgent,
    session::{ModelChoice, PermissionMode},
};
use super::super::{
    consts::{ADAPTER, BACKEND, GLIMMER, GREY, KNOT, LOGIN, PROGRAM},
    structs::Codex,
};

impl Codex {
    /// Codex through its ACP adapter.
    ///
    /// The adapter's modes are `read-only` (asks before an edit, which is atelier's Ask), `workspace-write` (edits the
    /// workspace, asks for the rest: Accept edits), `agent` (asks only for what looks unsafe: Auto) and
    /// `agent-full-access` (Bypass). Plan is a switch of Codex's, not a mode, so atelier offers none. The models are
    /// ones the adapter listed on 2026-10-04 (one account checked), by their names without the reasoning effort the
    /// adapter adds in brackets; an account that lacks one has it refused with a warning.
    pub fn agent() -> AcpAgent {
        let model = |id: &str, label: &str| ModelChoice { id: id.into(), label: label.into() };
        AcpAgent {
            name: BACKEND,
            program: PROGRAM.into(),
            args: ADAPTER.iter().map(|word| word.to_string()).collect(),
            modes: vec![
                (PermissionMode::Ask, "read-only".into()),
                (PermissionMode::AcceptEdits, "workspace-write".into()),
                (PermissionMode::Auto, "agent".into()),
                (PermissionMode::Bypass, "agent-full-access".into()),
            ],
            models: vec![
                model("gpt-6-luna", "GPT-6 Luna"),
                model("gpt-5.6-terra", "GPT-5.6 Terra"),
                model("gpt-5.6-luna", "GPT-5.6 Luna"),
                model("gpt-5.5", "GPT-5.5"),
            ],
            thinking: true,
            todos: true,
            resume: true,
            login: LOGIN.iter().map(|word| word.to_string()).collect(),
        }
    }

    /// Codex's whole look, for [`atelier_ui::Thinking`] and a session row.
    pub fn look() -> AgentLook {
        AgentLook {
            mark: Mark { working: KNOT, orbiting: KNOT, color: color(GREY), icon_frame: 0 },
            message: color(GREY),
            glimmer: color(GLIMMER),
            labels: PhaseLabels { waiting: "Waiting for Codex…".into(), ..PhaseLabels::default() },
        }
    }
}

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}
