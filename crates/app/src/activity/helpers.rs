use atelier_agents::session::{Item, ToolKind};

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The kinds of call in the order a summary names them, each with its word.
const KINDS: [(ToolKind, &str); 7] = [
    (ToolKind::Edit, "Edited"),
    (ToolKind::Write, "Wrote"),
    (ToolKind::Shell, "Ran"),
    (ToolKind::Search, "Searched"),
    (ToolKind::Read, "Read"),
    (ToolKind::Fetch, "Fetched"),
    (ToolKind::Other, "Used"),
];

/// The summary of items `from..to` that draw (`visible`): the time spent thinking, the calls of each kind
/// ("Edited 1 · Searched 2 · Read 4"), the subagents.
pub fn summary(items: &[Item], from: usize, to: usize, visible: &dyn Fn(usize) -> bool) -> String {
    let (mut seconds, mut thought, mut agents) = (0u64, false, 0usize);
    let mut counts = [0usize; KINDS.len()];
    let mut count = |kind: ToolKind| {
        if let Some(at) = KINDS.iter().position(|(k, _)| *k == kind) {
            counts[at] += 1;
        }
    };
    for ix in (from..to).filter(|&ix| visible(ix)) {
        match &items[ix] {
            Item::Thinking { took, .. } => {
                thought = true;
                seconds += took.map_or(0, |t| t.as_secs());
            }
            Item::Tool(call) => count(call.call.kind),
            Item::Subagent { calls, .. } => {
                agents += 1;
                calls.iter().for_each(|call| count(call.call.kind));
            }
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if thought {
        parts.push(format!("Thought for {seconds}s"));
    }
    parts.extend(KINDS.iter().zip(counts).filter(|(_, n)| *n > 0).map(|((_, word), n)| format!("{word} {n}")));
    if agents > 0 {
        parts.push(plural(agents, "subagent", "subagents"));
    }
    if parts.is_empty() { "Worked".into() } else { parts.join(" · ") }
}
