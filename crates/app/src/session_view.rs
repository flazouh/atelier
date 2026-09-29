//! A session's panel: its header, what the agent said and did as a virtual list, the todos and the
//! subagents still running, and the composer. Every row comes from the session's `Conversation`, drawn
//! with the agent panel's own parts, so a live session and one read back from history look the same.
//!
//! The list lays out only the rows on screen; a question shows `ToolApproval` in its place in the
//! conversation, and its answer goes back through the session.

use std::time::Instant;

use beui::{
    AgentText, AgentTextStatus, MessageBubble, MessageBubbleAlign, MessageBubbleVariant, SubagentCard, SubagentRow,
    SubagentStrip, Thinking, ThinkingPhase, Todo as TodoRow, TodoList, TodoStatus as RowStatus, ToolApproval,
    ToolApprovalStatus, ToolCall as ToolRow, ToolStatus as RowToolStatus,
    button::{Button, ButtonVariant},
    changed_files::ChangedFiles,
    select::{Select, SelectOption},
    icon::{Icon, IconName},
    session_status::SessionStatus,
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Window, div, list, prelude::FluentBuilder, px,
};
use lathe_agents::session::{Answer, Call, ChoiceKind, Item, SubagentStatus, TodoStatus, ToolStatus};

use crate::{
    agent_session::{AgentSession, SessionEvent},
    list_diff::Row,
};

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
        let note = match (&output.full_at, output.truncated) {
            (Some(path), _) => format!("\n… the whole output is at {path}"),
            (None, true) => "\n… cut; the agent keeps the rest".into(),
            (None, false) => String::new(),
        };
        row = row.output(format!("{}{note}", output.text));
    }
    row
}

/// One row of the list: a conversation item, or a turn's changed files.
fn row(session: &Entity<AgentSession>, ix: usize, cx: &App) -> AnyElement {
    match session.read(cx).shown.get(ix).copied() {
        Some(Row::Item(item)) => item_row(session, item, cx),
        Some(Row::Changes { turn }) => changes_row(session, turn, cx),
        None => div().into_any_element(),
    }
}

/// The files turn `turn` changed, with its `+a −r`: Review opens the review at a file, and a file's
/// name opens it in the editor.
fn changes_row(session: &Entity<AgentSession>, turn: usize, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let Some(files) = s.reviews.turns.turns().get(turn).map(|t| lathe_review::present::changed_files(t.files())) else {
        return div().into_any_element();
    };
    let (review, open) = (session.clone(), session.clone());
    let card = ChangedFiles::new(gpui_kit::ElementId::Name(format!("{}-changes-{turn}", s.key).into()), files)
        .on_review(move |path, _, cx| {
            review.update(cx, |_, cx| cx.emit(SessionEvent::Review { turn: Some(turn), path: Some(path.to_string()) }))
        })
        .on_open_file(move |path, _, cx| open.update(cx, |_, cx| cx.emit(SessionEvent::OpenFile(path.to_string()))));
    div().px(px(16.)).pb(px(14.)).child(card).into_any_element()
}

/// One item of the conversation.
fn item_row(session: &Entity<AgentSession>, ix: usize, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let theme = cx.theme().clone();
    let items = s.conversation.items();
    let Some(item) = items.get(ix) else { return div().into_any_element() };
    let working = s.conversation.working();
    let last = ix + 1 == items.len();
    let look = s.agent.look.clone();
    let key = s.key.clone();
    let id = |what: &str| gpui_kit::ElementId::Name(format!("{key}-{what}-{ix}").into());
    let body = match item {
        // The bubble aligns itself to the end of the row.
        Item::User { text } => MessageBubble::text(id("user"), text.clone())
            .variant(MessageBubbleVariant::Solid)
            .align(MessageBubbleAlign::End)
            .into_any_element(),
        Item::Text { text, .. } => {
            let status = if working && last { AgentTextStatus::Streaming } else { AgentTextStatus::Complete };
            AgentText::new(id("text"), SharedString::from(text.clone())).status(status).copy_text(text.clone()).into_any_element()
        }
        Item::Thinking { block, took, .. } => {
            let phase = match took {
                Some(took) => ThinkingPhase::Thought { seconds: took.as_secs() },
                None if working => ThinkingPhase::Thinking { since: s.thinking_since.get(block).copied().unwrap_or_else(Instant::now) },
                None => ThinkingPhase::Thought { seconds: 0 },
            };
            let running = if last { s.conversation.running_subagents() } else { 0 };
            Thinking::new(id("think"), look, phase).subagents(running).into_any_element()
        }
        Item::Tool(call) => tool_row(id("tool"), call).into_any_element(),
        Item::Subagent { subagent, status, activity, calls, summary } => {
            let name = subagent.kind.clone().unwrap_or_else(|| "Subagent".into());
            let mut card = SubagentCard::new(id("sub"), look, name, subagent.task.clone())
                .tool_calls(calls.len() as u64)
                .calls(calls.iter().enumerate().map(|(n, c)| tool_row(id(&format!("sub-call-{n}")), c)).collect());
            if let Some(model) = &subagent.model {
                card = card.model(model.clone());
                if let Some(mark) = lathe_agents::registry::model_mark(model) {
                    card = card.model_mark(mark);
                }
            }
            match status {
                SubagentStatus::Running => {
                    if let Some(activity) = activity {
                        card = card.live_tool(activity.clone());
                    }
                }
                SubagentStatus::Done | SubagentStatus::Failed => card = card.finished(None),
            }
            let summary = summary.clone().filter(|_| *status != SubagentStatus::Running);
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(card)
                .children(summary.map(|t| div().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(t)))
                .into_any_element()
        }
        Item::Permission { request, answer } => {
            let mut approval = ToolApproval::new(id("ask"), request.call.name.clone())
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
            if let Some(file) = &request.call.file {
                approval = approval.parameter("File", file.clone());
            }
            for (name, value) in request.call.input.as_object().into_iter().flatten() {
                let shown = value.as_str().map_or_else(|| value.to_string(), str::to_string);
                // A long value (a file's new text) reads as code, not as one line.
                approval = if shown.contains('\n') || shown.len() > 120 {
                    approval.parameter_code(name.clone(), shown)
                } else {
                    approval.parameter(name.clone(), shown)
                };
            }
            let on = |kind| {
                let (session, request) = (session.clone(), request.id.clone());
                move |_: &_, _: &mut Window, cx: &mut App| session.update(cx, |s, cx| s.answer(&request, kind, cx))
            };
            approval.on_approve(on(ChoiceKind::Allow)).on_always_allow(on(ChoiceKind::AllowAlways)).on_deny(on(ChoiceKind::Deny)).into_any_element()
        }
        Item::Notice(text) => div()
            .flex()
            .items_start()
            .gap(px(8.))
            .text_size(TextSize::Xs.font_size())
            .text_color(theme.danger)
            .child(Icon::new(IconName::Error).size(px(14.)).color(theme.danger))
            .child(SharedString::from(text.clone()))
            .into_any_element(),
    };
    div().px(px(16.)).pb(px(14.)).child(body).into_any_element()
}

/// The panel for `session`.
pub fn session_view(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    // The "Show details" fold keeps its state across frames, by the session's key.
    let key = session.read(cx).key.clone();
    let open = window.use_keyed_state(gpui_kit::ElementId::Name(format!("{key}-details").into()), cx, |_, _| false);
    let shown = *open.read(cx);
    let s = session.read(cx);
    let empty = s.conversation.items().is_empty();
    let rows = {
        let session = session.clone();
        list(s.list.clone(), move |ix, _, cx| row(&session, ix, cx)).size_full()
    };
    // Why it stopped: the reason, and the agent's own last words behind a fold.
    // The whole reason: the row keeps the cut one.
    // A turn that failed in a live session says so in the conversation; the box is for an agent
    // that is gone, or never started.
    let gone = !s.running() && !s.starting;
    let failure = match (&s.problem, &s.status, &s.stderr) {
        (Some(problem), _, _) => Some(problem.clone()),
        (None, SessionStatus::Failed(_), _) if !gone => None,
        (None, SessionStatus::Failed(_), Some(stderr)) => Some(crate::status::last_line(stderr).to_string().into()),
        (None, SessionStatus::Failed(why), None) => Some(why.clone()),
        _ => None,
    };
    let details = s.stderr.clone();
    let failure = failure.map(|why| {
        div()
            .mx(px(12.))
            .mb(px(8.))
            .px(px(12.))
            .py(px(8.))
            .rounded(radius::LG)
            .bg(theme.card_strong)
            .flex()
            .flex_col()
            .gap(px(6.))
            .text_size(TextSize::Xs.font_size())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(Icon::new(IconName::Error).size(px(14.)).color(theme.danger))
                    // The reason wraps inside the box instead of running past the panel.
                    .child(div().flex_1().min_w_0().whitespace_normal().child(why))
                    .children(details.is_some().then(|| {
                        Button::new(gpui_kit::ElementId::Name(format!("{}-show-details", s.key).into()))
                            .label(if shown { "Hide details" } else { "Show details" })
                            .variant(ButtonVariant::Ghost)
                            .on_click(move |_, _, cx| open.update(cx, |o, cx| {
                                *o = !*o;
                                cx.notify();
                            }))
                    })),
            )
            .when(shown, |d| {
                d.children(details.map(|t| {
                    div().font_family(beui::typography::MONO_FONT_FAMILY).text_color(muted).whitespace_normal().child(t)
                }))
            })
    });
    let todos = s.conversation.todos();
    let todos = (!todos.is_empty()).then(|| {
        TodoList::new(
            gpui_kit::ElementId::Name(format!("{}-todos", s.key).into()),
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
        )
    });
    let strip: Vec<SubagentRow> = s
        .conversation
        .items()
        .iter()
        .enumerate()
        .filter_map(|(i, item)| match item {
            Item::Subagent { subagent, status: SubagentStatus::Running, activity, calls, .. } => {
                let name = subagent.kind.clone().unwrap_or_else(|| "Subagent".into());
                let row = SubagentRow::new(gpui_kit::ElementId::Name(format!("{}-strip-{i}", s.key).into()), s.agent.look.clone(), name, subagent.task.clone())
                    .tool_calls(calls.len() as u64);
                Some(match activity {
                    Some(a) => row.tool(a.clone()),
                    None => row,
                })
            }
            _ => None,
        })
        .collect();
    let strip = SubagentStrip::new(gpui_kit::ElementId::Name(format!("{}-strip", s.key).into()), strip);
    let starting = s.starting;
    let agent_name = s.agent.name;
    let body = if empty {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .child(div().text_size(TextSize::Sm.font_size()).child(if starting { "Starting…" } else { "A new session" }))
            .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child(format!("Ask {agent_name} anything about this project.")))
            .children(s.can_choose_agent().then(|| agent_picker(session, cx)).flatten())
            .into_any_element()
    } else {
        div().flex_1().min_h_0().pt(px(12.)).child(rows).into_any_element()
    };
    let composer = s.composer.clone();
    let header = header(session, cx);
    let interrupt = session.clone();
    div()
        .key_context("AgentSession")
        // Escape while the agent works interrupts its turn; otherwise it goes on to the input.
        .on_action(move |_: &gpui_kit::base::input::Escape, _, cx| {
            if interrupt.read(cx).conversation.working() {
                cx.stop_propagation();
                interrupt.update(cx, |s, cx| s.interrupt(cx));
            } else {
                cx.propagate();
            }
        })
        .flex()
        .flex_col()
        .size_full()
        .child(header)
        .child(body)
        .children(failure)
        .child(div().flex().flex_col().gap(px(8.)).px(px(12.)).pb(px(12.)).children(todos).child(strip).child(composer))
        .into_any_element()
}

/// The agents this build can start, as a picker a new session shows until its first message.
fn agent_picker(session: &Entity<AgentSession>, cx: &App) -> Option<AnyElement> {
    let agents = lathe_agents::registry::agents();
    if agents.len() < 2 {
        return None;
    }
    let s = session.read(cx);
    let current = agents.iter().position(|a| a.backend.name() == s.agent.backend.name());
    let options: Vec<SelectOption> = agents
        .iter()
        .map(|a| {
            let option = SelectOption::from(a.name);
            match a.mark.clone() {
                Some(mark) => option.mark(mark),
                None => option,
            }
        })
        .collect();
    let backends: Vec<String> = agents.iter().map(|a| a.backend.name().to_string()).collect();
    let pick = session.clone();
    Some(
        div()
            .mt(px(12.))
            .w(px(220.))
            .child(
                Select::new(gpui_kit::ElementId::Name(format!("{}-agent", s.key).into()), options)
                    .selected(current)
                    .on_change(move |ix, _, cx| {
                        let Some(backend) = backends.get(ix).cloned() else { return };
                        pick.update(cx, |_, cx| cx.emit(SessionEvent::ChooseAgent(backend)));
                    }),
            )
            .into_any_element(),
    )
}

/// The panel's top line: the title (a press renames it), what the session is doing, and Stop while
/// its agent runs.
fn header(session: &Entity<AgentSession>, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let s = session.read(cx);
    let key = s.key.clone();
    let title = match &s.renaming {
        Some(input) => div()
            .flex_1()
            .min_w_0()
            .rounded(radius::MD)
            .bg(theme.card_strong)
            .child(gpui_kit::component::input::Input::new(input).appearance(false).px(px(8.)).text_size(TextSize::Sm.font_size()))
            .into_any_element(),
        None => {
            let rename = session.clone();
            div()
                .id(gpui_kit::ElementId::Name(format!("{key}-title").into()))
                .flex_1()
                .min_w_0()
                .truncate()
                .cursor_text()
                .text_size(TextSize::Sm.font_size())
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .tooltip(beui::tooltip::Tooltip::text("Rename"))
                .on_click(move |_, window, cx| rename.update(cx, |s, cx| s.start_rename(window, cx)))
                .child(s.shown_title())
                .into_any_element()
        }
    };
    let stop = s.running().then(|| {
        let stop = session.clone();
        Button::new(gpui_kit::ElementId::Name(format!("{key}-stop").into()))
            .label("Stop")
            .variant(ButtonVariant::Ghost)
            .tooltip("Stop the agent. The session stays in the list and can be resumed.")
            .on_click(move |_, _, cx| stop.update(cx, |s, cx| s.stop(cx)))
    });
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(40.))
        .px(px(16.))
        .child(title)
        .child(div().flex_none().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(s.status.words()))
        .children(stop)
}

#[cfg(test)]
mod tests;
