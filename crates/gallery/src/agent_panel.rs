//! The Agent panel story: a whole session as the panel will show it, at the panel's width. A finished
//! turn ends with its changed files; a subagent runs where it started; the subagent strip and the
//! session's pull request sit above the composer; and the reply names #3344, which the app knows, and
//! #9999, which it does not.

use std::time::Instant;

use beui::{
    ActiveTheme, AgentText, AgentTextStatus, ModelBadge, Button, ButtonSize, ButtonVariant, ChangedFiles, Checks, DiffLine,
    EntranceList, FileDiff, FileDiffStatus, IconName, MessageBubble, MessageBubbleAlign, MessageBubbleVariant,
    PrCard, ReviewState, SubagentStrip, Thinking, Todo, TodoList, TodoStatus, ToolApproval, ToolCall, ToolStatus,
    pane_header,
};
use gpui_kit::{
    AnyElement, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use lathe_agents::claude;

use super::{DIFF, Gallery, PromptInput, REPLY, TEST_OUTPUT, thinking_for};
use crate::agent_parts::{PR_TEXT, changed_files, pr_3344, resolve_pr, running_card, strip_rows};

fn sample_plan() -> Vec<Todo> {
    vec![
        Todo::new("find", "Find where hunk numbers start", TodoStatus::Done),
        Todo::new("fix", "Fix the off-by-one in hunk_starts", TodoStatus::Done),
        Todo::new("test-line-1", "Add a test for a hunk at line 1", TodoStatus::InProgress),
        Todo::new("suite", "Run the full test suite", TodoStatus::Pending),
    ]
}

/// How many items the Agent panel's session holds.
pub const SESSION_LEN: usize = 12;

/// The session's first `shown` chat items, as the panel lists them. The two reads stack tight in their
/// own list, so each still enters on its own.
fn session_list(started: Instant, replay: usize, shown: usize, tick: usize) -> EntranceList {
    let tools = [
        ("s-read", ToolCall::new("s-read", "Read file").file("crates/beui/src/file_diff.rs").meta("214 lines").status(ToolStatus::Done)),
        ("s-grep", ToolCall::new("s-grep", "Searched code").tool("fn hunk_starts").status(ToolStatus::Done)),
    ];
    let tools = tools.into_iter().take(shown.saturating_sub(1)).fold(
        EntranceList::new(ElementId::NamedInteger("session-tools".into(), replay as u64), div().flex().flex_col()),
        |list, (id, item)| list.item(id, item),
    );
    let reply = |id: &'static str, text: &'static str| {
        AgentText::new(id, text)
            .status(AgentTextStatus::Complete)
            .copy_text(text)
            .pr_resolver(resolve_pr)
            .on_open_pr(|pr, _, _| println!("open #{}", pr.number))
            .into_any_element()
    };
    let items: [(&'static str, AnyElement); SESSION_LEN - 1] = [
        ("s-user", MessageBubble::text("s-user", "The line numbers in the diff view are off by one. Can you fix it?").variant(MessageBubbleVariant::Solid).align(MessageBubbleAlign::End).into_any_element()),
        ("s-tools", tools.into_any_element()),
        ("s-reply", reply("s-reply", REPLY)),
        ("s-agent", running_card("s-agent", tick).into_any_element()),
        ("s-plan", TodoList::new("s-plan", sample_plan()).into_any_element()),
        ("s-diff", FileDiff::new("s-diff", "crates/beui/src/file_diff.rs", DiffLine::parse(DIFF)).status(FileDiffStatus::Complete).into_any_element()),
        ("s-test", ToolCall::new("s-test", "Ran tests").tool("cargo test -p beui").meta("3.1s").status(ToolStatus::Done).output(TEST_OUTPUT).into_any_element()),
        ("s-pr", reply("s-pr", PR_TEXT)),
        (
            "s-files",
            ChangedFiles::new("s-files", changed_files())
                .on_open_file(|path, _, _| println!("open {path}"))
                .on_review(|path, _, _| println!("review from {path}"))
                .into_any_element(),
        ),
        (
            "s-push",
            ToolApproval::new("s-push", "git push origin main")
                .description("Push the fix so CI can run the full test suite.")
                .parameter("Directory", "~/Documents/lathe")
                .default_open(true)
                .on_approve(|_, _, _| {})
                .on_always_allow(|_, _, _| {})
                .on_deny(|_, _, _| {})
                .into_any_element(),
        ),
        ("s-think", Thinking::new("s-think", claude::look(), thinking_for(started, 18)).elapsed("18s").tokens(3_400).subagents(2).into_any_element()),
    ];
    // The two reads share one row of this list, so once both show it holds one item fewer.
    let rows = if shown >= 3 { shown - 1 } else { shown };
    items.into_iter().take(rows).fold(
        EntranceList::new(ElementId::NamedInteger("session-list".into(), replay as u64), div().flex().flex_col().gap(px(16.))),
        |list, (id, item)| list.item(id, item),
    )
}

/// `replay` names the list, so each replay starts a fresh one; `shown` is how many items it holds so far;
/// `tick` is the live clock.
pub fn agent_panel(
    prompt: &Entity<PromptInput>,
    started: Instant,
    replay: usize,
    shown: usize,
    tick: usize,
    cx: &mut Context<Gallery>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let header = pane_header("Claude Code", cx)
        .child(ModelBadge::new("Opus 5.5").mark("p-model", crate::agent_parts::anthropic()))
        .child(div().flex_1())
        .child(
            Button::new("p-replay")
                .label("Replay session")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(|this, _, _, cx| this.start_replay(cx))),
        )
        .child(Button::new("p-new").icon(IconName::Add).variant(ButtonVariant::Ghost).size(ButtonSize::Icon))
        .child(Button::new("p-more").icon(IconName::MoreHoriz).variant(ButtonVariant::Ghost).size(ButtonSize::Icon));

    let list = session_list(started, replay, shown, tick);
    let session = div().id("session").flex_1().overflow_y_scroll().px(px(20.)).py(px(20.)).child(list);

    // The subagents arrive with the end of the replay; the strip opens as they do.
    let rows = if shown + 3 >= SESSION_LEN { strip_rows(tick) } else { Vec::new() };
    let strip = SubagentStrip::new(ElementId::NamedInteger("strip".into(), replay as u64), rows);
    let pr = PrCard::new("p-pr", pr_3344())
        .checks(Checks { passed: 3, running: 1, ..Default::default() })
        .review(ReviewState::Requested)
        .on_open(|pr, _, _| println!("open #{}", pr.number));

    div()
        .flex()
        .justify_center()
        .size_full()
        .bg(theme.card)
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(560.))
                .h_full()
                .bg(theme.background)
                .child(header)
                .child(session)
                .child(div().flex().flex_col().gap(px(8.)).p(px(12.)).child(pr).child(strip).child(prompt.clone())),
        )
}
