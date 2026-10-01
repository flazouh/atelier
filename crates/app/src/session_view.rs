//! A session's panel: its header, what the agent said and did as a virtual list, the todos and the
//! subagents still running, and the composer. Every row comes from the session's `Conversation`, drawn
//! with the agent panel's own parts, so a live session and one read back from history look the same.
//!
//! The list lays out only the rows on screen; a question shows `ToolApproval` in its place in the
//! conversation, and its answer goes back through the session.

use std::{rc::Rc, time::Instant};

use beui::{
    PressStop,
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
    Styled, Window, div, list, prelude::FluentBuilder, 
};
use beui::scale::px;
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

/// The row of `call`: what it is about (the command, the file, the pattern, with an icon for its kind), its paths
/// relative to `root`, and the mark its answered approval left.
fn tool_row(id: impl Into<gpui_kit::ElementId>, call: &Call, root: &str, mark: Option<&'static str>) -> ToolRow {
    let about = summary::summary(call, |path| beui::tool_preview::relative_path(path, root).to_string());
    let icon = match about.kind {
        lathe_agents::session::ToolKind::Shell => IconName::Terminal,
        lathe_agents::session::ToolKind::Read => IconName::Description,
        lathe_agents::session::ToolKind::Edit | lathe_agents::session::ToolKind::Write => IconName::Edit,
        lathe_agents::session::ToolKind::Search => IconName::Search,
        lathe_agents::session::ToolKind::Fetch => IconName::Public,
        lathe_agents::session::ToolKind::Other => IconName::Build,
    };
    let mut row = ToolRow::new(id, SharedString::from(about.title)).icon(icon).status(row_status(call.call.status));
    if let Some(detail) = about.detail {
        row = row.tool(detail);
    }
    if let Some(file) = about.file {
        row = row.file(file);
    }
    if let Some(mark) = mark {
        row = row.meta(mark);
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
        Some(Row::Activity { from, to }) => activity_row(session, from, to, cx),
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

/// One item of the conversation, in its row's padding.
fn item_row(session: &Entity<AgentSession>, ix: usize, cx: &App) -> AnyElement {
    match item_body(session, ix, cx) {
        Some(body) => div().px(px(16.)).pb(px(14.)).child(body).into_any_element(),
        None => div().into_any_element(),
    }
}

/// An activity group: the run of thinking, tool calls and subagents `from..to`. While the agent works and the
/// group is the last row, its newest items show in a viewport that holds the end in view; once the turn is over
/// it folds to one line of words that a press opens.
fn activity_row(session: &Entity<AgentSession>, from: usize, to: usize, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let theme = cx.theme().clone();
    let items = s.conversation.items();
    let visible = |ix: usize| calls::shows(items, ix);
    let (live, open) = (s.group_is_live(to), s.group_is_open(from, to));
    let words = crate::activity::summary(items, from, to, &visible);
    let bodies: Vec<AnyElement> = if open { (from..to).filter(|&ix| visible(ix)).filter_map(|ix| item_body(session, ix, cx)).collect() } else { Vec::new() };
    let key = s.key.clone();
    let list = div().flex().flex_col().flex_none().w_full().gap(px(8.)).children(bodies);
    let body = if live {
        // The end stays in view: the room is filled from its bottom, so what does not fit runs off the top. There is no fade
        // over that edge: the panel behind is a different tone when it is the active one, and a fade of a fixed tone showed
        // as a dark band.
        div()
            .debug_selector(|| "activity-live".into())
            .relative()
            .flex()
            .flex_col()
            .justify_end()
            .w_full()
            .max_h(px(crate::activity::LIVE_HEIGHT))
            .overflow_hidden()
            .child(list)
            .into_any_element()
    } else {
        let toggle = session.clone();
        let head = div()
            .id(gpui_kit::ElementId::Name(format!("{key}-activity-{from}").into()))
            .debug_selector(|| "activity-summary".into())
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .text_size(TextSize::Sm.font_size())
            .text_color(theme.muted_foreground)
            .hover(|s| s.text_color(theme.foreground))
            .on_click(move |_, _, cx| toggle.update(cx, |s, cx| s.toggle_group(from, cx)))
            .child(Icon::new(if open { IconName::ChevronDown } else { IconName::ChevronRight }).size(px(14.)))
            .child(SharedString::from(words));
        div().flex().flex_col().gap(px(8.)).child(head).when(open, |d| d.child(list)).into_any_element()
    };
    div().px(px(16.)).pb(px(14.)).child(body).into_any_element()
}

/// One item of the conversation, without its row's padding; `None` for an item that draws nothing.
fn item_body(session: &Entity<AgentSession>, ix: usize, cx: &App) -> Option<AnyElement> {
    let s = session.read(cx);
    let theme = cx.theme().clone();
    let items = s.conversation.items();
    let item = items.get(ix)?;
    // One row per call: the approval stands in for it while it waits, and its row carries the answer.
    if !calls::shows(items, ix) {
        return None;
    }
    let root = s.root();
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
            // A `#N` the project's pull requests hold is a chip; pressing it opens that pull request.
            let chips = s.pr_chips.clone();
            let opener = session.downgrade();
            AgentText::new(id("text"), SharedString::from(text.clone()))
                .status(status)
                .fade_tail(true)
                .copy_text(text.clone())
                .pr_resolver(move |number| chips.iter().find(|c| c.number == number).cloned())
                .on_open_pr(move |chip, _, cx| drop(opener.update(cx, |_, cx| cx.emit(SessionEvent::OpenPull(chip.clone())))))
                .into_any_element()
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
        Item::Tool(call) => tool_row(id("tool"), call, &root, calls::mark_kept(items, &call.call.id, &s.reviews.approvals)).into_any_element(),
        Item::Subagent { subagent, status, activity, calls, summary } => {
            let name = subagent.kind.clone().unwrap_or_else(|| "Subagent".into());
            let mut card = SubagentCard::new(id("sub"), look, name, subagent.task.clone())
                .tool_calls(calls.len() as u64)
                .calls(calls.iter().enumerate().map(|(n, c)| tool_row(id(&format!("sub-call-{n}")), c, &root, None)).collect());
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
            let preview = preview::preview(&request.call, &root);
            // With a preview, the raw input stays behind View details; with none, it is all there is.
            let mut approval = ToolApproval::new(id("ask"), request.call.name.clone())
                .title(request.call.name.clone())
                .default_open(preview.is_none())
                .status(match answer {
                    Answer::Asking => ToolApprovalStatus::Pending,
                    Answer::Answered(ChoiceKind::Deny) | Answer::Withdrawn => ToolApprovalStatus::Denied,
                    Answer::Answered(_) => ToolApprovalStatus::Approved,
                });
            if let Some(reason) = &request.reason {
                approval = approval.description(reason.clone());
            }
            if let Some(preview) = preview {
                approval = approval.preview(preview);
            }
            if let Some(file) = &request.call.file {
                approval = approval.parameter("File", beui::tool_preview::relative_path(file, &root));
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
    Some(body)
}

/// The conversation's rows: a list that lays out only the rows on screen. Each row has a test name,
/// `row-<index>`.
pub fn rows(session: &Entity<AgentSession>, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let session = session.clone();
    list(s.list.clone(), move |ix, _, cx| div().debug_selector(move || format!("row-{ix}")).child(row(&session, ix, cx)).into_any_element())
        .size_full()
        .into_any_element()
}

/// The panel for `session`, rows and all.
#[cfg(test)]
pub fn session_view(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> AnyElement {
    session_view_with(session, None, window, cx)
}

/// The panel for `session`, with its rows drawn by the caller (a cached view of their own), or here when
/// `rows` is `None`.
pub fn session_view_with(session: &Entity<AgentSession>, rows: Option<AnyElement>, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let muted = theme.muted_foreground;
    // The "Show details" fold keeps its state across frames, by the session's key.
    let key = session.read(cx).key.clone();
    let open = window.use_keyed_state(gpui_kit::ElementId::Name(format!("{key}-details").into()), cx, |_, _| false);
    let shown = *open.read(cx);
    let s = session.read(cx);
    let empty = s.conversation.items().is_empty();
    let rows = rows.unwrap_or_else(|| self::rows(session, cx));
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
            .rounded(radius::lg())
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
    let pull_card = session.read(cx).pull_card.clone();
    let header = header(session, window, cx);
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
        .child(div().flex().flex_col().gap(px(8.)).px(px(12.)).pb(px(12.)).children(todos).child(strip).children(pull_card).child(composer))
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
/// A9: Stop shows while a turn goes on (working, or waiting for the reader), not while the agent idles.
fn shows_stop(running: bool, status: &SessionStatus) -> bool {
    running && matches!(status, SessionStatus::Working | SessionStatus::NeedsYou(_))
}

fn header(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let (key, renaming, shown_title, running, task, project, badge, reviewable, session_id) = {
        let s = session.read(cx);
        (
            s.key.clone(),
            s.renaming.clone(),
            s.shown_title(),
            shows_stop(s.running(), &s.status),
            s.task.clone(),
            s.project_name(),
            s.badge.clone(),
            !s.reviews.turns.turns().is_empty(),
            s.id.as_ref().map(|i| i.as_str().to_string()),
        )
    };
    let title = match &renaming {
        Some(input) => div()
            .flex_1()
            .min_w_0()
            .rounded(radius::md())
            .bg(theme.card_strong)
            .child(gpui_kit::component::input::Input::new(input).appearance(false).px(px(8.)).text_size(TextSize::Sm.font_size()))
            .into_any_element(),
        None => {
            let rename = session.clone();
            div()
                .id(gpui_kit::ElementId::Name(format!("{key}-title").into()))
                .debug_selector({
                    let title = shown_title.clone();
                    move || format!("panel-title:{title}")
                })
                .flex_1()
                .min_w_0()
                .truncate()
                .cursor_text()
                .text_size(TextSize::Sm.font_size())
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .tooltip(beui::tooltip::Tooltip::text("Rename"))
                .press_stop(gpui_kit::ElementId::Name(format!("{key}-title-focus").into()), radius::md(), window, cx)
                .on_click(move |_, window, cx| rename.update(cx, |s, cx| s.start_rename(window, cx)))
                .child(shown_title)
                .into_any_element()
        }
    };
    let stop = running.then(|| {
        let stop = session.clone();
        Button::new(gpui_kit::ElementId::Name(format!("{key}-stop").into()))
            .label("Stop")
            .variant(ButtonVariant::Ghost)
            .tooltip("Stop the agent. The session stays in the list and can be resumed.")
            .on_click(move |_, _, cx| stop.update(cx, |s, cx| s.stop(cx)))
    });
    // The task the session began from: a press opens it in the Tasks pane.
    let chip = task.map(|task| {
        let open = session.clone();
        Button::new(gpui_kit::ElementId::Name(format!("{key}-task").into()))
            .label(task.key)
            .variant(ButtonVariant::Ghost)
            .size(beui::ButtonSize::Sm)
            .tooltip("Open the task")
            .on_click(move |_, _, cx| open.update(cx, |_, cx| cx.emit(SessionEvent::OpenTask)))
    });
    // What the session has to show for itself: the changes its turns made, ready to review.
    let review = reviewable.then(|| {
        let open = session.clone();
        Button::new(gpui_kit::ElementId::Name(format!("{key}-review").into()))
            .debug_name("panel-review")
            .label("Review")
            .variant(ButtonVariant::Ghost)
            .tooltip("Review what this session changed")
            .on_click(move |_, _, cx| open.update(cx, |_, cx| cx.emit(SessionEvent::Review { turn: None, path: None })))
    });
    let more = panel_menu(session, &key, session_id, window, cx);
    let close = {
        let close = session.clone();
        Button::new(gpui_kit::ElementId::Name(format!("{key}-close").into()))
            .debug_name("panel-close")
            .icon(IconName::Close)
            .variant(ButtonVariant::Ghost)
            .size(beui::ButtonSize::IconSm)
            .tooltip("Close the panel")
            .on_click(move |_, _, cx| close.update(cx, |_, cx| cx.emit(SessionEvent::Close)))
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(40.))
        .pl(px(16.))
        .pr(px(8.))
        // The project the session works in, as its badge (its name stands in until the shell has set the badge), so a
        // panel always says where it is; the name is its tooltip.
        .child(match badge {
            Some(badge) => div()
                .id(gpui_kit::ElementId::Name(format!("{key}-project").into()))
                .debug_selector(|| "panel-project".into())
                .flex_none()
                .tooltip(beui::tooltip::Tooltip::text(project))
                .child(beui::project_badge::ProjectBadge::new(badge.label, badge.color).icon(badge.icon))
                .into_any_element(),
            None => div().debug_selector(|| "panel-project".into()).flex_none().max_w(px(120.)).truncate().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(project).into_any_element(),
        })
        .child(title)
        .children(chip)
        .children(review)
        .children(stop)
        .child(more)
        .child(close)
}

/// What a choice in the panel's menu does.
type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// The panel's ⋯ menu: what a reader does with a session besides talking to it. Rename it, begin another in the same
/// project, see the project's files, copy the id the agent knows it by (to resume it elsewhere), archive it.
fn panel_menu(session: &Entity<AgentSession>, key: &SharedString, session_id: Option<String>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    use beui::{
        menu::{self, Entry, Menu, MenuItem, MenuLook, Origin},
        popover::{Hang, Popover},
    };
    let open = window.use_keyed_state(gpui_kit::ElementId::Name(format!("{key}-more").into()), cx, |_, _| false);
    let is_open = *open.read(cx);
    let flip = {
        let open = open.clone();
        move |cx: &mut App| open.update(cx, |o, cx| {
            *o = !*o;
            cx.notify();
        })
    };
    let menu = is_open.then(|| {
        // A choice shuts the menu, then does its work.
        let item = |label: &'static str, name: &'static str, run: MenuAction| {
            let open = open.clone();
            Entry::from(MenuItem::new(label).debug_name(name).on_select(move |window, cx| {
                open.update(cx, |o, cx| {
                    *o = false;
                    cx.notify();
                });
                run(window, cx)
            }))
        };
        let (rename, fresh, files, archive) = (session.clone(), session.clone(), session.clone(), session.clone());
        let mut entries = vec![
            item("Rename", "panel-rename", Rc::new(move |window, cx| rename.update(cx, |s, cx| s.start_rename(window, cx)))),
            item("New session in this project", "panel-new-session", Rc::new(move |_, cx| fresh.update(cx, |_, cx| cx.emit(SessionEvent::NewSession)))),
            item("Open the project's files", "panel-files", Rc::new(move |_, cx| files.update(cx, |_, cx| cx.emit(SessionEvent::ShowFiles)))),
        ];
        if let Some(id) = session_id.clone() {
            entries.push(item("Copy session id", "panel-copy-id", Rc::new(move |_, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(id.clone())))));
        }
        entries.push(item("Archive", "panel-archive", Rc::new(move |_, cx| archive.update(cx, |_, cx| cx.emit(SessionEvent::Archive)))));
        let rows = entries.len();
        let close = open.clone();
        Popover::new(gpui_kit::ElementId::Name(format!("{key}-more-popover").into()))
            .open(true)
            .hang(Hang::Right(0., 30.))
            .keep_focus()
            .height(menu::height_in(MenuLook::PROJECT, rows))
            .on_close(move |_, cx| {
                close.update(cx, |o, cx| {
                    *o = false;
                    cx.notify();
                });
            })
            .child(Menu::new(gpui_kit::ElementId::Name(format!("{key}-more-menu").into()), entries).look(MenuLook::PROJECT).origin(Origin::TopRight))
    });
    div()
        .relative()
        .child(
            Button::new(gpui_kit::ElementId::Name(format!("{key}-more-button").into()))
                .debug_name("panel-more")
                .icon(IconName::MoreHoriz)
                .variant(ButtonVariant::Ghost)
                .size(beui::ButtonSize::IconSm)
                .tooltip("More")
                .open(is_open)
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    flip(cx)
                }),
        )
        .children(menu)
}

pub(crate) mod calls;
mod preview;
mod summary;
#[cfg(test)]
mod tests;
