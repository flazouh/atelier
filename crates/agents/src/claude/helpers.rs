use atelier_ui::{AgentLook, Mark, PhaseLabels, Sprite};
use gpui_kit::{ElementId, Hsla, rgb};

pub use super::spark::SparkState;
use super::types::{CLAY, GLIMMER_CLAY, MESSAGE_CLAY};

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
    PhaseLabels { waiting: atelier_i18n::t(&crate::strings::WAITING_FOR_CLAUDE).into(), ..PhaseLabels::default() }
}

/// Claude's whole look, for [`atelier_ui::Thinking`] and [`atelier_ui::SubagentRow`].
pub fn look() -> AgentLook {
    AgentLook { mark: mark(), message: color(MESSAGE_CLAY), glimmer: color(GLIMMER_CLAY), labels: labels() }
}

/// A spark playing `state` in clay, resting on the still mark.
pub fn spark(id: impl Into<ElementId>, state: SparkState) -> Sprite {
    mark().sprite(id, state.strip())
}

pub(super) fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}
