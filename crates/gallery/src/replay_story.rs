//! The "Agent replay" story: a captured `claude` run played back through the Claude Code mapper into a
//! `Conversation`, drawn with the agent panel's own parts. It is the path a live session takes, minus the
//! process: lines become events, events go through the queue that coalesces them, and the panel draws what
//! the conversation holds. `REPLAY_FIXTURE=<name>` picks the run (`subagent_foreground` by default);
//! `REPLAY_LINES` sets how many lines play each frame (1 by default).
use std::time::{Duration, Instant};

use beui::{
    ActiveTheme, AgentText, AgentTextStatus, SubagentCard, Todo as TodoRow, TodoList, TodoStatus as RowStatus,
    ToolApproval, ToolApprovalStatus, ToolCall as ToolRow, ToolStatus as RowToolStatus,
};
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use atelier_agents::{
    claude, claude_code::Mapper,
    session::{
        Answer, Call, ChoiceKind, Conversation, EventQueue, Item, PermissionRequest, SubagentStatus, TodoStatus,
        ToolStatus,
    },
};

/// How long a question shows before the story answers it, as the recorded run was answered.
const ASK_HOLD: Duration = Duration::from_millis(700);

/// A captured run, and what the user answered when it asked.
const RUNS: [(&str, &str, ChoiceKind); 9] = [
    ("plain", include_str!("../../agents/tests/fixtures/claude_code/plain.jsonl"), ChoiceKind::Allow),
    ("tool_read", include_str!("../../agents/tests/fixtures/claude_code/tool_read.jsonl"), ChoiceKind::Allow),
    ("permission_allow", include_str!("../../agents/tests/fixtures/claude_code/permission_allow.jsonl"), ChoiceKind::Allow),
    ("permission_deny", include_str!("../../agents/tests/fixtures/claude_code/permission_deny.jsonl"), ChoiceKind::Deny),
    ("permission_interrupt", include_str!("../../agents/tests/fixtures/claude_code/permission_interrupt.jsonl"), ChoiceKind::Deny),
    ("task_list", include_str!("../../agents/tests/fixtures/claude_code/task_list.jsonl"), ChoiceKind::Allow),
    ("subagent_background", include_str!("../../agents/tests/fixtures/claude_code/subagent_background.jsonl"), ChoiceKind::Allow),
    ("subagent_foreground", include_str!("../../agents/tests/fixtures/claude_code/subagent_foreground.jsonl"), ChoiceKind::Allow),
    ("long_output", include_str!("../../agents/tests/fixtures/claude_code/long_output.jsonl"), ChoiceKind::Allow),
];

pub struct ReplayStory {
    lines: Vec<&'static str>,
    /// What the recorded user answered.
    decision: ChoiceKind,
    /// When the question on screen appeared.
    asking_since: Option<Instant>,
    next: usize,
    per_frame: usize,
    mapper: Mapper,
    queue: EventQueue,
    conversation: Conversation,
    page: ScrollHandle,
    started: Instant,
}

impl ReplayStory {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let wanted = std::env::var("REPLAY_FIXTURE").unwrap_or_else(|_| "subagent_foreground".into());
        let run = RUNS.iter().find(|(name, ..)| *name == wanted).unwrap_or(&RUNS[7]);
        let mut conversation = Conversation::new();
        conversation.user_sent("Replay of a captured run");
        let mut mapper = Mapper::new();
        mapper.user_sent();
        Self {
            lines: run.1.lines().collect(),
            decision: run.2,
            asking_since: None,
            next: 0,
            per_frame: std::env::var("REPLAY_LINES").ok().and_then(|n| n.parse().ok()).unwrap_or(1),
            mapper,
            queue: EventQueue::new(|| {}),
            conversation,
            page: ScrollHandle::new(),
            started: Instant::now(),
        }
    }

    /// Plays the next lines and folds what the queue holds: one frame's work.
    fn advance(&mut self) {
        let asking = self.conversation.items().iter().rev().find_map(|item| match item {
            Item::Permission { request, answer: Answer::Asking } => Some(request.id.clone()),
            _ => None,
        });
        if let Some(id) = asking {
            // The recorded run holds no answer; the story gives the one its user gave, after a pause.
            let since = *self.asking_since.get_or_insert_with(Instant::now);
            if since.elapsed() < ASK_HOLD {
                return;
            }
            self.conversation.answered(&id, self.decision);
        }
        self.asking_since = None;
        let sink = self.queue.sink();
        for line in self.lines.iter().skip(self.next).take(self.per_frame) {
            self.mapper.line(line, Instant::now()).into_iter().for_each(|event| sink(event));
        }
        self.next = (self.next + self.per_frame).min(self.lines.len());
        for event in self.queue.drain() {
            self.conversation.apply(&event);
        }
    }

    fn answer(&mut self, request: &PermissionRequest, kind: ChoiceKind, cx: &mut Context<Self>) {
        self.conversation.answered(&request.id, kind);
        cx.notify();
    }
}

fn row_status(status: ToolStatus) -> RowToolStatus {
    match status {
        ToolStatus::Pending | ToolStatus::Running => RowToolStatus::Running,
        ToolStatus::Done => RowToolStatus::Done,
        ToolStatus::Failed => RowToolStatus::Failed,
    }
}

fn tool_row(id: impl Into<gpui_kit::ElementId>, call: &Call) -> ToolRow {
    let mut row = ToolRow::new(id, SharedString::from(call.call.name.clone())).status(row_status(call.call.status));
    if let Some(file) = &call.call.file {
        row = row.file(file.clone());
    }
    if let Some(output) = &call.output {
        let note = if output.truncated { "\n… cut, the rest is kept by the agent" } else { "" };
        row = row.output(format!("{}{note}", output.text));
    }
    row
}

impl Render for ReplayStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.next < self.lines.len() || self.asking_since.is_some() {
            self.advance();
            self.page.scroll_to_bottom();
            window.request_animation_frame();
        }
        let theme = cx.theme().clone();
        let this = cx.entity();
        let last = self.conversation.items().len().saturating_sub(1);
        let working = self.conversation.working();
        let items: Vec<AnyElement> = self
            .conversation
            .items()
            .iter()
            .enumerate()
            .map(|(i, item)| match item {
                Item::User { text } => div()
                    .p(px(10.))
                    .rounded(px(8.))
                    .bg(theme.card)
                    .text_color(theme.foreground)
                    .child(SharedString::from(text.clone()))
                    .into_any_element(),
                Item::Text { block, text } => {
                    let status = if working && i == last { AgentTextStatus::Streaming } else { AgentTextStatus::Complete };
                    AgentText::new(("replay-text", block.0), SharedString::from(text.clone())).status(status).into_any_element()
                }
                Item::Thinking { text, took, .. } => {
                    let said = match took {
                        Some(took) => format!("Thought for {:.1}s", took.as_secs_f32()),
                        None => "Thinking…".to_string(),
                    };
                    let said = if text.is_empty() { said } else { format!("{said}: {text}") };
                    div().text_color(theme.muted_foreground).child(SharedString::from(said)).into_any_element()
                }
                Item::Tool(call) => tool_row(("replay-tool", i), call).into_any_element(),
                Item::Subagent { subagent, status, activity, calls, .. } => {
                    let name = subagent.kind.clone().unwrap_or_else(|| "Subagent".into());
                    let mut card = SubagentCard::new(("replay-sub", i), claude::look(), name, subagent.task.clone())
                        .tool_calls(calls.len() as u64)
                        .calls(calls.iter().enumerate().map(|(n, c)| tool_row(("replay-sub-call", i * 1000 + n), c)).collect());
                    if let Some(model) = &subagent.model {
                        card = card.model(model.clone());
                    }
                    if let Some(activity) = activity {
                        card = card.live_tool(activity.clone());
                    }
                    if *status != SubagentStatus::Running {
                        card = card.finished(Some(self.started.elapsed().min(Duration::from_secs(3600)).as_secs()));
                    }
                    card.into_any_element()
                }
                Item::Permission { request, answer } => {
                    let mut approval = ToolApproval::new(("replay-ask", i), request.call.name.clone())
                        .title(request.call.name.clone())
                        .default_open(true)
                        .status(match answer {
                            Answer::Asking => ToolApprovalStatus::Pending,
                            Answer::Answered(ChoiceKind::Deny) | Answer::Withdrawn => ToolApprovalStatus::Denied,
                            Answer::Answered(_) => ToolApprovalStatus::Approved,
                        });
                    if let Some(reason) = &request.reason {
                        approval = approval.description(reason.clone());
                    }
                    for (key, value) in request.call.input.as_object().into_iter().flatten() {
                        let shown = value.as_str().map_or_else(|| value.to_string(), str::to_string);
                        approval = approval.parameter(key.clone(), shown);
                    }
                    let on = |kind| {
                        let (this, request) = (this.clone(), request.clone());
                        move |_: &_, _: &mut Window, cx: &mut gpui_kit::App| {
                            this.update(cx, |story, cx| story.answer(&request, kind, cx));
                        }
                    };
                    approval.on_approve(on(ChoiceKind::Allow)).on_always_allow(on(ChoiceKind::AllowAlways)).on_deny(on(ChoiceKind::Deny)).into_any_element()
                }
                Item::Notice(text) => div().text_color(theme.danger).child(SharedString::from(text.clone())).into_any_element(),
            })
            .collect();

        let todos = self.conversation.todos();
        div()
            .id("replay-page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.page)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .p(px(16.))
                    .max_w(px(720.))
                    .children(items)
                    .when_some((!todos.is_empty()).then_some(todos), |d, todos| {
                        d.child(TodoList::new(
                            "replay-todos",
                            todos
                                .iter()
                                .map(|t| {
                                    let status = match t.status {
                                        TodoStatus::Pending => RowStatus::Pending,
                                        TodoStatus::InProgress => RowStatus::InProgress,
                                        TodoStatus::Done => RowStatus::Done,
                                    };
                                    TodoRow::new(t.id.clone(), t.text.clone(), status)
                                })
                                .collect(),
                        ))
                    }),
            )
    }
}
