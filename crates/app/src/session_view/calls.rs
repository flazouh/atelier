//! One row per tool call. While a call waits for its approval, the approval stands in its place, so the
//! call's own row hides; once answered, the approval hides and the call's row carries the answer as a
//! mark, such as "Approved".
use lathe_agents::session::{Answer, ChoiceKind, Item, ToolId};

/// The approval of the call `id`, if the call asked for one.
fn approval<'a>(items: &'a [Item], id: &ToolId) -> Option<&'a Answer> {
    items.iter().rev().find_map(|item| match item {
        Item::Permission { request, answer } if request.call.id == *id => Some(answer),
        _ => None,
    })
}

/// Whether item `ix` draws a row.
pub fn shows(items: &[Item], ix: usize) -> bool {
    match items.get(ix) {
        Some(Item::Tool(call)) => approval(items, &call.call.id) != Some(&Answer::Asking),
        Some(Item::Permission { answer, .. }) => *answer == Answer::Asking,
        _ => true,
    }
}

/// The mark the row of call `id` carries for its answered approval.
pub fn mark(items: &[Item], id: &ToolId) -> Option<&'static str> {
    match approval(items, id)? {
        Answer::Asking => None,
        Answer::Answered(ChoiceKind::Allow) => Some("Approved"),
        Answer::Answered(ChoiceKind::AllowAlways) => Some("Always allowed"),
        Answer::Answered(ChoiceKind::Deny) => Some("Denied"),
        Answer::Withdrawn => Some("Not answered"),
    }
}

/// As `mark`, with the answers the session's record kept for a call the history no longer asks about.
pub fn mark_kept(items: &[Item], id: &ToolId, kept: &std::collections::HashMap<String, crate::review_state::Approval>) -> Option<&'static str> {
    use crate::review_state::Approval;
    mark(items, id).or_else(|| {
        kept.get(id.as_str()).map(|a| match a {
            Approval::Approved => "Approved",
            Approval::AlwaysAllowed => "Always allowed",
            Approval::Denied => "Denied",
        })
    })
}

#[cfg(test)]
mod tests;
