use std::collections::{HashMap, HashSet};

use super::structs::{Origin, Turn};
use super::types::Detail;
use crate::session::{BlockId, Event, ToolCall, ToolId};

/// How long a brief may be, in characters: about 30k tokens at 4 characters each.
pub const BUDGET: usize = 120_000;

const INSTRUCTION: &str = "Read files before you change them: what they hold now counts more than what the conversation says. \
When something below is unclear, look in the full transcript.";
const PARAGRAPH: &str = "\n\n";
/// The input fields that say what a call without a file works on, the first one found.
const TARGET_FIELDS: [&str; 5] = ["command", "pattern", "query", "url", "description"];
const TOOL_LINE_MAX: usize = 120;
const ELLIPSIS: char = '…';
const FENCE_MIN: usize = 3;
/// How Claude Code opens the summary it writes when it compacts a session. Each one covers every turn before it.
const COMPACT_SUMMARY: &str = "This session is being continued from a previous conversation";

/// The history as turns: a user message starts one, and the agent's words and top-level calls fill it.
pub fn turns(events: &[Event]) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut last_block: Option<BlockId> = None;
    // Where each call's line is, so a later, fuller input can redo it.
    let mut calls: HashMap<ToolId, (usize, usize, ToolCall)> = HashMap::new();
    for event in events {
        match event {
            Event::UserMessage { text } => {
                turns.push(Turn { user: Some(text.clone()), ..Turn::default() });
                last_block = None;
            }
            Event::Text { block, delta } => {
                let turn = current(&mut turns);
                if last_block != Some(*block) && !turn.said.is_empty() {
                    turn.said.push_str(PARAGRAPH);
                }
                last_block = Some(*block);
                turn.said.push_str(delta);
            }
            Event::ToolStarted(call) if call.parent.is_none() => {
                let turn = current(&mut turns);
                turn.tools.push(tool_line(call));
                calls.insert(call.id.clone(), (turns.len() - 1, turns[turns.len() - 1].tools.len() - 1, call.clone()));
            }
            Event::ToolInput { id, input, file } => {
                if let Some((turn, tool, call)) = calls.get_mut(id) {
                    call.input = input.clone();
                    call.file = file.clone().or(call.file.take());
                    turns[*turn].tools[*tool] = tool_line(call);
                }
            }
            _ => {}
        }
    }
    turns
}

/// The first message for the next agent: where the work comes from, then the conversation from the latest compact
/// summary on, the oldest turns cut down first until it fits in `budget` characters. The newest turn stays whole
/// even over budget.
pub fn brief(events: &[Event], origin: &Origin, budget: usize) -> String {
    let mut turns = turns(events);
    if let Some(summary) = turns.iter().rposition(is_compact_summary) {
        turns.drain(..summary);
    }
    let head = head(origin);
    let sizes: Vec<[usize; 3]> = turns.iter().map(|turn| [Detail::Whole, Detail::Tools, Detail::Ask].map(|d| render(turn, d).len())).collect();
    let size_of = |turn: usize, detail: Detail| sizes[turn][detail as usize];
    let mut details = vec![Detail::Whole; turns.len()];
    let mut total = head.len() + (0..turns.len()).map(|turn| size_of(turn, Detail::Whole)).sum::<usize>();
    let older = turns.len().saturating_sub(1);
    for step in Detail::STEPS_DOWN {
        for (turn, detail) in details.iter_mut().enumerate().take(older) {
            if total <= budget {
                break;
            }
            total = total - size_of(turn, *detail) + size_of(turn, step);
            *detail = step;
        }
    }
    let mut left_out = 0;
    while total > budget && left_out < older {
        total -= size_of(left_out, details[left_out]);
        left_out += 1;
    }
    let mut text = head;
    if left_out > 0 {
        text.push_str(&format!("Some earlier turns are left out ({left_out}); read them in the full transcript.{PARAGRAPH}"));
    }
    for (turn, detail) in turns.iter().zip(details).skip(left_out) {
        text.push_str(&render(turn, detail));
    }
    text
}

/// The whole history as Markdown, with what each top-level call returned, for the next agent to search.
pub fn transcript(events: &[Event], origin: &Origin) -> String {
    let mut text = format!("# {}{PARAGRAPH}Worked on by {}.{PARAGRAPH}", origin.title, origin.agent);
    let mut last_block: Option<BlockId> = None;
    let mut top_level: HashSet<ToolId> = HashSet::new();
    for event in events {
        match event {
            Event::UserMessage { text: said } => {
                text.push_str(&format!("## User{PARAGRAPH}{said}{PARAGRAPH}"));
                last_block = None;
            }
            Event::Text { block, delta } => {
                if last_block != Some(*block) {
                    text.push_str(&format!("{PARAGRAPH}## Agent{PARAGRAPH}"));
                }
                last_block = Some(*block);
                text.push_str(delta);
            }
            Event::ToolStarted(call) if call.parent.is_none() => {
                top_level.insert(call.id.clone());
                text.push_str(&format!("{PARAGRAPH}**{}**{PARAGRAPH}", tool_line(call)));
                last_block = None;
            }
            Event::ToolFinished { id, output } if top_level.contains(id) => {
                let fence = fence_for(&output.text);
                text.push_str(&format!("{fence}text\n{}\n{fence}{PARAGRAPH}", output.text.trim_end()));
            }
            _ => {}
        }
    }
    text
}

fn is_compact_summary(turn: &Turn) -> bool {
    turn.user.as_deref().is_some_and(|user| user.starts_with(COMPACT_SUMMARY))
}

fn current(turns: &mut Vec<Turn>) -> &mut Turn {
    if turns.is_empty() {
        turns.push(Turn::default());
    }
    let last = turns.len() - 1;
    &mut turns[last]
}

/// `Edit: src/lib.rs`, `Bash: cargo test`: the tool and what it works on, on one short line.
fn tool_line(call: &ToolCall) -> String {
    let target = call.file.clone().or_else(|| TARGET_FIELDS.iter().find_map(|field| call.input.get(field)?.as_str().map(str::to_string)));
    let line = match target {
        Some(target) => format!("{}: {}", call.name, target.lines().next().unwrap_or_default()),
        None => call.name.clone(),
    };
    shortened(&line, TOOL_LINE_MAX)
}

fn shortened(line: &str, max: usize) -> String {
    match line.char_indices().nth(max) {
        Some((cut, _)) => format!("{}{ELLIPSIS}", &line[..cut]),
        None => line.to_string(),
    }
}

fn head(origin: &Origin) -> String {
    let mut text = format!(
        "You are continuing work that another agent started: {}, in the session \"{}\". {INSTRUCTION}{PARAGRAPH}",
        origin.agent, origin.title
    );
    if let Some(transcript) = &origin.transcript {
        text.push_str(&format!("Full transcript: {transcript}{PARAGRAPH}"));
    }
    if let Some(state) = &origin.working_state {
        text.push_str(&format!("## Working state now{PARAGRAPH}{}{PARAGRAPH}", state.trim_end()));
    }
    text.push_str(&format!("## The conversation so far{PARAGRAPH}"));
    text
}

fn render(turn: &Turn, detail: Detail) -> String {
    let mut text = String::new();
    if let Some(user) = &turn.user {
        text.push_str(&format!("**User:** {}\n\n", user.trim_end()));
    }
    if detail == Detail::Whole && !turn.said.is_empty() {
        text.push_str(&format!("**Agent:** {}\n\n", turn.said.trim_end()));
    }
    if detail != Detail::Ask && !turn.tools.is_empty() {
        for line in &turn.tools {
            text.push_str(&format!("- {line}\n"));
        }
        text.push('\n');
    }
    text
}

/// A code fence longer than any run of backticks in `text`, so the text cannot close it.
fn fence_for(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(FENCE_MIN.max(longest + 1))
}
