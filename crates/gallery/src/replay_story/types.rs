use std::time::Duration;

use atelier_agents::{
    session::{ChoiceKind},
};

/// How long a question shows before the story answers it, as the recorded run was answered.
pub(super) const ASK_HOLD: Duration = Duration::from_millis(700);

/// A captured run, and what the user answered when it asked.
pub(super) const RUNS: [(&str, &str, ChoiceKind); 9] = [
    ("plain", include_str!("../../../agents/tests/fixtures/claude_code/plain.jsonl"), ChoiceKind::Allow),
    ("tool_read", include_str!("../../../agents/tests/fixtures/claude_code/tool_read.jsonl"), ChoiceKind::Allow),
    ("permission_allow", include_str!("../../../agents/tests/fixtures/claude_code/permission_allow.jsonl"), ChoiceKind::Allow),
    ("permission_deny", include_str!("../../../agents/tests/fixtures/claude_code/permission_deny.jsonl"), ChoiceKind::Deny),
    ("permission_interrupt", include_str!("../../../agents/tests/fixtures/claude_code/permission_interrupt.jsonl"), ChoiceKind::Deny),
    ("task_list", include_str!("../../../agents/tests/fixtures/claude_code/task_list.jsonl"), ChoiceKind::Allow),
    ("subagent_background", include_str!("../../../agents/tests/fixtures/claude_code/subagent_background.jsonl"), ChoiceKind::Allow),
    ("subagent_foreground", include_str!("../../../agents/tests/fixtures/claude_code/subagent_foreground.jsonl"), ChoiceKind::Allow),
    ("long_output", include_str!("../../../agents/tests/fixtures/claude_code/long_output.jsonl"), ChoiceKind::Allow),
];
