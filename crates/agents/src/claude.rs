//! Claude's look: its spark, the Claude Code CLI's clay colours, and Claude's words for each phase.

pub mod spark;

use atelier_ui::{AgentLook, Mark, PhaseLabels, Sprite};
use gpui_kit::{ElementId, Hsla, rgb};

pub use spark::SparkState;

/// Claude's clay, `var(--cds-clay, #d97757)`: the spark's colour in the desktop app.
pub const CLAY: u32 = 0xD97757;
/// The CLI's `claude` message colour: the status label and a running subagent's name.
pub const MESSAGE_CLAY: u32 = 0xD77757;
/// The CLI's `claudeShimmer`: the lighter clay that walks across the label.
pub const GLIMMER_CLAY: u32 = 0xF59575;

/// The spark as a atelier-ui mark: thinking while Claude works, orbiting while subagents run, and the first
/// thinking frame at rest, as the desktop app falls back to its static mark.
pub fn mark() -> Mark {
    Mark {
        working: SparkState::Thinking.strip(),
        orbiting: SparkState::Orbiting.strip(),
        color: color(CLAY),
        // The fourth frame of the thinking strip is the spark at full size.
        icon_frame: 3,
    }
}

/// The strips the loading mark picks among at random, one per run: every spark that loops while Claude works.
/// Orbiting is left out, since it means subagents run, and so is waiting, which moves too slowly to read as loading.
pub fn loading_strips() -> Vec<atelier_ui::Strip> {
    [SparkState::Thinking, SparkState::Writing, SparkState::Shimmer].map(SparkState::strip).to_vec()
}

/// Claude's words. The thinking labels are the desktop app's `Wne`, which are also beui's neutral ones;
/// only waiting names Claude.
pub fn labels() -> PhaseLabels {
    PhaseLabels { waiting: "Waiting for Claude…".into(), ..PhaseLabels::default() }
}

/// Claude's whole look, for [`atelier_ui::Thinking`] and [`atelier_ui::SubagentRow`].
pub fn look() -> AgentLook {
    AgentLook { mark: mark(), message: color(MESSAGE_CLAY), glimmer: color(GLIMMER_CLAY), labels: labels() }
}

/// A spark playing `state` in clay, resting on the still mark.
pub fn spark(id: impl Into<ElementId>, state: SparkState) -> Sprite {
    mark().sprite(id, state.strip())
}

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

#[cfg(test)]
mod tests;
