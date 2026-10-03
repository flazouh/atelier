use serde_json::json;

use super::{BUDGET, Origin, brief, transcript, turns};
use crate::session::{BlockId, Event, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus};

const AGENT: &str = "Claude Code";
const TITLE: &str = "Add a subtract function";
const TRANSCRIPT: &str = "/data/handoffs/s1.md";
const STATE: &str = "On branch subtract\n M src/lib.rs";
const ASK: &str = "Add a subtract function to src/lib.rs";
const ANSWER: &str = "I added subtract and a test.";
const FILE: &str = "src/lib.rs";
const COMMAND: &str = "cargo test";
const OUTPUT: &str = "test result: ok. 3 passed";
const SUBAGENT_FILE: &str = "src/hidden.rs";
const LEFT_OUT: &str = "earlier turns are left out";
const SUMMARY: &str = "This session is being continued from a previous conversation that ran out of context. It added subtract.";

fn origin() -> Origin {
    Origin { agent: AGENT.into(), title: TITLE.into(), transcript: Some(TRANSCRIPT.into()), working_state: Some(STATE.into()) }
}

fn user(text: &str) -> Event {
    Event::UserMessage { text: text.into() }
}

fn said(block: u64, text: &str) -> Event {
    Event::Text { block: BlockId(block), delta: text.into() }
}

fn call(id: &str, name: &str, kind: ToolKind, input: serde_json::Value, file: Option<&str>) -> Event {
    Event::ToolStarted(ToolCall {
        id: ToolId(id.into()),
        name: name.into(),
        kind,
        input,
        file: file.map(str::to_string),
        parent: None,
        status: ToolStatus::Running,
    })
}

fn edit(id: &str) -> Event {
    call(id, "Edit", ToolKind::Edit, json!({}), Some(FILE))
}

fn shell(id: &str) -> Event {
    call(id, "Bash", ToolKind::Shell, json!({ "command": COMMAND }), None)
}

fn finished(id: &str, text: &str) -> Event {
    Event::ToolFinished { id: ToolId(id.into()), output: ToolOutput::head(text, false) }
}

fn by_subagent(id: &str) -> Event {
    let Event::ToolStarted(mut call) = call(id, "Read", ToolKind::Read, json!({}), Some(SUBAGENT_FILE)) else { unreachable!() };
    call.parent = Some(ToolId("task".into()));
    Event::ToolStarted(call)
}

fn first_block(turn: usize) -> u64 {
    turn as u64 * 2
}

/// One turn: the ask, two blocks of text, an edit and a test run.
fn one_turn(n: usize) -> Vec<Event> {
    vec![
        user(&format!("{ASK} {n}")),
        said(first_block(n), "Looking."),
        edit(&format!("e{n}")),
        shell(&format!("s{n}")),
        finished(&format!("s{n}"), OUTPUT),
        said(first_block(n) + 1, &format!("{ANSWER} {n}")),
    ]
}

fn many_turns(count: usize) -> Vec<Event> {
    (0..count).flat_map(one_turn).collect()
}

#[test]
fn a_turn_keeps_the_ask_the_agents_words_and_one_line_per_tool() {
    let turns = turns(&one_turn(1));

    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].user.as_deref(), Some(format!("{ASK} 1").as_str()));
    assert_eq!(turns[0].said, format!("Looking.\n\n{ANSWER} 1"));
    assert_eq!(turns[0].tools, vec![format!("Edit: {FILE}"), format!("Bash: {COMMAND}")]);
}

#[test]
fn calls_a_subagent_made_stay_out_of_the_brief() {
    let turns = turns(&[user(ASK), by_subagent("r1")]);

    assert!(turns[0].tools.is_empty());
}

#[test]
fn the_brief_says_where_it_comes_from_and_how_to_go_on() {
    let text = brief(&one_turn(1), &origin(), BUDGET);

    for part in [AGENT, TITLE, TRANSCRIPT, STATE, "Read files before you change them"] {
        assert!(text.contains(part), "the brief names {part:?}:\n{text}");
    }
}

#[test]
fn a_short_session_goes_whole_into_the_brief() {
    let text = brief(&many_turns(3), &origin(), BUDGET);

    for n in 0..3 {
        assert!(text.contains(&format!("{ASK} {n}")));
        assert!(text.contains(&format!("{ANSWER} {n}")));
    }
    assert!(!text.contains(LEFT_OUT));
}

#[test]
fn over_budget_the_oldest_turns_lose_the_agents_words_first_then_their_tools() {
    let whole = brief(&many_turns(3), &origin(), BUDGET).len();
    let tight = brief(&many_turns(3), &origin(), whole - 1);

    assert!(!tight.contains(&format!("{ANSWER} 0")), "the oldest words go first");
    assert!(tight.contains(&format!("{ANSWER} 2")), "the newest turn stays whole");
    assert!(tight.contains(&format!("{ASK} 0")), "every ask stays while it can");
}

#[test]
fn far_over_budget_the_oldest_turns_go_and_the_brief_says_so() {
    let text = brief(&many_turns(40), &origin(), 2_000);

    assert!(text.contains(LEFT_OUT));
    assert!(!text.contains(&format!("{ASK} 0\n")), "the oldest ask goes");
    assert!(text.contains(&format!("{ASK} 39")), "the newest ask stays");
    assert!(text.contains(&format!("{ANSWER} 39")), "and the newest turn stays whole");
}

#[test]
fn the_transcript_keeps_what_the_tools_returned() {
    let text = transcript(&one_turn(1), &origin());

    for part in [TITLE, ASK, ANSWER, COMMAND, OUTPUT] {
        assert!(text.contains(part), "the transcript has {part:?}:\n{text}");
    }
}


#[test]
fn a_compact_summary_stands_for_every_turn_before_it() {
    let events: Vec<Event> = one_turn(0).into_iter().chain([user(SUMMARY)]).chain(one_turn(1)).collect();

    let text = brief(&events, &origin(), BUDGET);

    assert!(text.contains(SUMMARY));
    assert!(!text.contains(&format!("{ASK} 0")), "the summary covers the first turn");
    assert!(text.contains(&format!("{ANSWER} 1")));
}
