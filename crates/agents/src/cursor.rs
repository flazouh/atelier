//! Cursor: its agent CLI spoken to over ACP (`agent acp`), and its look for atelier-ui. What `agent` sends was
//! checked live; `docs/agents.md` says what it offers and how atelier's modes map to its own.

use atelier_ui::{AgentLook, Mark, PhaseLabels, Strip};
use gpui_kit::{Hsla, rgb};

use crate::{
    acp::AcpAgent,
    session::{ModelChoice, PermissionMode},
};

/// The name `agent acp` reports and a session records.
pub const BACKEND: &str = "cursor";

/// Cursor's agent, as the host's `PATH` finds it. The installer puts `agent` (and `cursor-agent`, the
/// same program) in `~/.local/bin`.
///
/// Its `agent` mode asks before a command outside its allowlist, which is atelier's Ask; `plan` is Plan.
/// Its `ask` mode answers questions and changes nothing, and atelier has no mode for that. The models are
/// ones Cursor offered on 2026-10-01 (one account checked), by their names without Cursor's bracketed
/// settings; `default` is Cursor's Auto. A model an account lacks is refused with a warning.
pub fn agent() -> AcpAgent {
    let model = |id: &str, label: &str| ModelChoice { id: id.into(), label: label.into() };
    AcpAgent {
        name: BACKEND,
        program: "agent".into(),
        args: vec!["acp".into()],
        modes: vec![(PermissionMode::Ask, "agent".into()), (PermissionMode::Plan, "plan".into())],
        models: vec![
            model("default", "Auto"),
            model("composer-2.5", "Composer 2.5"),
            model("claude-opus-5-5", "Opus 5.5"),
            model("claude-sonnet-5-5", "Sonnet 5.5"),
            model("gpt-5.6-sol", "GPT-5.6 Sol"),
            model("gpt-5.5", "GPT-5.5"),
            model("grok-4.7", "Grok 4.7"),
            model("gemini-3.8-flash", "Gemini 3.8 Flash"),
        ],
        thinking: true,
        todos: true,
        resume: true,
    }
}

/// Cursor's cube, held still: Cursor has no animated mark.
const CUBE: Strip = Strip {
    path: "coding/cursor-dark.svg",
    bytes: include_bytes!("../assets/coding/cursor-dark.svg"),
    frames: 1,
    frame_ms: 1000,
    loops: false,
};

/// A neutral grey that reads on light and dark pages: Cursor's own mark is black or white.
const GREY: u32 = 0x8C8A84;
/// The lighter grey that walks across the label.
const GLIMMER: u32 = 0xBDBAB2;

/// Cursor's whole look, for [`atelier_ui::Thinking`] and a session row.
pub fn look() -> AgentLook {
    AgentLook {
        mark: Mark { working: CUBE, orbiting: CUBE, color: color(GREY), icon_frame: 0 },
        message: color(GREY),
        glimmer: color(GLIMMER),
        labels: PhaseLabels { waiting: "Waiting for Cursor…".into(), ..PhaseLabels::default() },
    }
}

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

#[cfg(test)]
mod tests;
