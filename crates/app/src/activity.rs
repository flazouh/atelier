//! The words of an activity group's summary row: what the agent did in a run of thinking, tool calls and
//! subagents, in one line ("Thought for 12s · 3 tool calls"). Pure.
use atelier_agents::session::Item;

/// The viewport's height while the agent works: the group shows its newest items in this much room.
pub const LIVE_HEIGHT: f32 = 208.;

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The summary of items `from..to` that draw (`visible`): the time spent thinking, the tool calls, the subagents.
pub fn summary(items: &[Item], from: usize, to: usize, visible: &dyn Fn(usize) -> bool) -> String {
    let (mut seconds, mut thought, mut tools, mut agents) = (0u64, false, 0usize, 0usize);
    for ix in (from..to).filter(|&ix| visible(ix)) {
        match &items[ix] {
            Item::Thinking { took, .. } => {
                thought = true;
                seconds += took.map_or(0, |t| t.as_secs());
            }
            Item::Tool(_) => tools += 1,
            Item::Subagent { calls, .. } => {
                agents += 1;
                tools += calls.len();
            }
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if thought {
        parts.push(format!("Thought for {seconds}s"));
    }
    if tools > 0 {
        parts.push(plural(tools, "tool call", "tool calls"));
    }
    if agents > 0 {
        parts.push(plural(agents, "subagent", "subagents"));
    }
    if parts.is_empty() { "Worked".into() } else { parts.join(" · ") }
}

#[cfg(test)]
mod tests;
