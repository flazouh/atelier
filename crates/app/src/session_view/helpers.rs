use std::{rc::Rc, time::Instant};

use atelier_ui::{
    AgentText,
    AgentTextStatus,
    FileDiff,
    FileDiffStatus,
    MessageBubble,
    MessageBubbleAlign,
    MessageBubbleVariant,
    PressStop,
    SubagentCard,
    SubagentRow,
    SubagentStrip,
    Thinking,
    ThinkingPhase,
    Todo as TodoRow,
    TodoList,
    TodoStatus as RowStatus,
    ToolApproval,
    ToolApprovalStatus,
    ToolCall as ToolRow,
    ToolStatus as RowToolStatus,
    button::{Button, ButtonVariant},
    icon::{Icon, IconName},
    message_rail::MessageRail,
    preview_clamp::PREVIEW_ROWS,
    select::{Select, SelectOption},
    session_status::SessionStatus,
    theme::{ActiveTheme, radius},
    typography::TextSize,
};
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, list, prelude::FluentBuilder,
};
use atelier_ui::scale::px;
use atelier_agents::session::{
    Answer, Call, ChoiceKind, Item, SubagentStatus, TodoStatus, ToolStatus,
};

use crate::{
    agent_session::{AgentSession, SessionEvent},
    list_diff::Row,
};
use super::types::{Block, MenuAction, READING_WIDTH};

fn row_status(status: ToolStatus) -> RowToolStatus {
    match status {
        ToolStatus::Pending | ToolStatus::Running => RowToolStatus::Running,
        ToolStatus::Done => RowToolStatus::Done,
        ToolStatus::Failed => RowToolStatus::Failed,
    }
}

/// Reading and searching show as flat rows, close together, not as cards.
pub(super) fn is_lookup(kind: atelier_agents::session::ToolKind) -> bool {
    matches!(kind, atelier_agents::session::ToolKind::Read | atelier_agents::session::ToolKind::Search)
}

/// A read's output is the file it read, which the agent and not the reader needs; only a failed one shows why.
pub(super) fn shows_output(kind: atelier_agents::session::ToolKind, status: ToolStatus) -> bool {
    kind != atelier_agents::session::ToolKind::Read || status == ToolStatus::Failed
}

fn block_of(item: &Item) -> Block {
    match item {
        Item::Tool(call) if is_lookup(super::summary::summary(call, |path| path.to_string()).kind) => Block::Flat,
        Item::Tool(_) | Item::Subagent { .. } | Item::Permission { .. } => Block::Card,
        _ => Block::Prose,
    }
}

/// The space between a block and the one after it. Two flat rows stack close, and so do cards, as the cards above the
/// composer do ([`atelier_ui::STACK_GAP`]); next to prose, or at the end, it is `default`.
pub(super) fn gap_between(above: Block, below: Option<Block>, default: f32) -> f32 {
    match (above, below) {
        (Block::Flat, Some(Block::Flat)) => 2.,
        (Block::Flat | Block::Card, Some(Block::Flat | Block::Card)) => atelier_ui::STACK_GAP,
        _ => default,
    }
}

/// The row of `call`: what it is about (the command, the file, the pattern, with an icon for its kind), its paths
/// relative to `root`, and the mark its answered approval left.
fn tool_row(id: impl Into<gpui_kit::ElementId>, call: &Call, root: &str, mark: Option<&'static str>, detailed: bool) -> ToolRow {
    let about = super::summary::summary(call, |path| atelier_ui::tool_preview::relative_path(path, root).to_string());
    let icon = match about.kind {
        atelier_agents::session::ToolKind::Shell => IconName::Terminal,
        atelier_agents::session::ToolKind::Read => IconName::Description,
        atelier_agents::session::ToolKind::Edit | atelier_agents::session::ToolKind::Write => IconName::Edit,
        atelier_agents::session::ToolKind::Search => IconName::Search,
        atelier_agents::session::ToolKind::Fetch => IconName::Public,
        atelier_agents::session::ToolKind::Other => IconName::Build,
    };
    let mut row = ToolRow::new(id, SharedString::from(about.title)).icon(icon).status(row_status(call.call.status));
    // Full detail keeps a call open on its output, also once it has finished.
    if detailed {
        row = row.default_open(true).collapse_on_complete(false);
    }
    if is_lookup(about.kind) {
        row = row.flat();
    }
    if let Some(detail) = about.detail {
        row = row.tool(detail);
    }
    if let Some(file) = about.file {
        row = row.file(file);
    }
    if let Some(mark) = mark {
        row = row.meta(mark);
    }
    if let Some(output) = call.output.as_ref().filter(|_| shows_output(about.kind, call.call.status)) {
        let note = match (&output.full_at, output.truncated) {
            (Some(path), _) => format!("\n… the whole output is at {path}"),
            (None, true) => "\n… cut; the agent keeps the rest".into(),
            (None, false) => String::new(),
        };
        row = row.output(format!("{}{note}", output.text));
    }
    row
}

/// The diff of an edit, as the agent writes it: open while it streams, then shut to its line or kept open as the density says.
/// Its rows are clipped and do not scroll; pressing them opens them wider and opens the review on this file.
fn edit_diff(session: &Entity<AgentSession>, id: impl Into<gpui_kit::ElementId>, view: super::edit::EditView) -> FileDiff {
    let path = view.preview.path().cloned().unwrap_or_default();
    let (review, file) = (session.clone(), path.to_string());
    FileDiff::new(id, path, view.preview.rows())
        .status(if view.streaming { FileDiffStatus::Streaming } else { FileDiffStatus::Complete })
        .default_open(view.open)
        .collapse_on_complete(view.fold_when_done)
        .preview_rows(PREVIEW_ROWS)
        .on_open(move |_, cx| review.update(cx, |_, cx| cx.emit(SessionEvent::Review { turn: None, path: Some(file.clone()) })))
}

/// One row of the list: a conversation item, or a turn's changed files.
pub(super) fn row(session: &Entity<AgentSession>, ix: usize, cx: &App) -> AnyElement {
    match session.read(cx).shown.get(ix).copied() {
        Some(Row::Item(item)) => item_row(session, item, ix, cx),
        Some(Row::Activity { from, to }) => activity_row(session, from, to, cx),
        Some(Row::Waiting) => waiting_row(session, cx),
        None => div().into_any_element(),
    }
}

/// The status line between a sent message and the agent's first word: the agent's own words for waiting, shimmering.
fn waiting_row(session: &Entity<AgentSession>, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let look = s.agent.look.clone();
    let loading = if look.mark.working == atelier_agents::claude::mark().working { atelier_agents::claude::loading_strips() } else { Vec::new() };
    let id = gpui_kit::ElementId::Name(format!("{}-waiting", s.key).into());
    div().px(px(16.)).pb(px(14.)).child(Thinking::new(id, look, ThinkingPhase::Waiting).loading(loading)).into_any_element()
}

/// One item of the conversation, in its row's padding. `row` is its place in the list: what follows it sets the room below.
fn item_row(session: &Entity<AgentSession>, ix: usize, row: usize, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let next = match s.shown.get(row + 1).copied() {
        Some(Row::Item(next)) => Some(block_of(&s.conversation.items()[next])),
        _ => None,
    };
    let below = gap_between(block_of(&s.conversation.items()[ix]), next, 14.);
    match item_body(session, ix, cx) {
        Some(body) => div().px(px(16.)).pb(px(below)).child(body).into_any_element(),
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
    let visible = |ix: usize| super::calls::shows(items, ix);
    let (live, open) = (s.group_is_live(to), s.group_is_open(from, to));
    let words = crate::activity::summary(items, from, to, &visible);
    let mut previous = None;
    let bodies: Vec<AnyElement> = if open {
        (from..to)
            .filter(|&ix| visible(ix))
            .filter_map(|ix| {
                let body = item_body(session, ix, cx)?;
                let here = block_of(&items[ix]);
                let above = previous.replace(here).map_or(0., |above| gap_between(above, Some(here), 8.));
                Some(div().mt(px(above)).child(body).into_any_element())
            })
            .collect()
    } else {
        Vec::new()
    };
    let key = s.key.clone();
    let list = div().flex().flex_col().flex_none().w_full().children(bodies);
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
    if !super::calls::shows(items, ix) {
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
        // In the dark theme a light bubble outshouts the reply; the card tone sets it apart from the panel instead.
        Item::User { text } => MessageBubble::text(id("user"), text.clone())
            .variant(if cx.theme().appearance == atelier_ui::theme::Appearance::Dark { MessageBubbleVariant::Borderless } else { MessageBubbleVariant::Solid })
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
            // Claude's spark loads with a different animation each run; another agent's mark is its own.
            let loading = if look.mark.working == atelier_agents::claude::mark().working { atelier_agents::claude::loading_strips() } else { Vec::new() };
            Thinking::new(id("think"), look, phase).loading(loading).subagents(running).into_any_element()
        }
        // The agent's question is a card of its own: streaming as it is written, answered once the reader has said.
        Item::Tool(call) if call.call.name == atelier_agents::claude_code::ASK_QUESTION => {
            let approval = super::calls::approval(items, &call.call.id);
            let (status, answers) = super::question::call_state(call, approval, s.conversation.answers_of(&call.call.id));
            atelier_ui::QuestionCard::new(id("question"), super::question::views(&call.call.input)).status(status).answers(answers).into_any_element()
        }
        Item::Tool(call) => {
            let mark = super::calls::mark_kept(items, &call.call.id, &s.reviews.approvals);
            let density = crate::tool_density::tool_density(cx);
            match super::edit::edit_view(call, &root, density, mark) {
                Some(view) => edit_diff(session, id("tool"), view).into_any_element(),
                // A command's log is clipped like a diff, and does not scroll; pressing it opens it wider.
                None => tool_row(id("tool"), call, &root, mark, density == crate::tool_density::ToolDensity::Detailed)
                    .preview_rows(PREVIEW_ROWS)
                    .into_any_element(),
            }
        }
        Item::Subagent { subagent, status, activity, calls, summary } => {
            let name = subagent.kind.clone().unwrap_or_else(|| "Subagent".into());
            let mut card = SubagentCard::new(id("sub"), look, name, subagent.task.clone())
                .tool_calls(calls.len() as u64)
                .calls(calls.iter().enumerate().map(|(n, c)| tool_row(id(&format!("sub-call-{n}")), c, &root, None, false)).collect());
            card = card.tint(super::tint::colour(tint_of(s, subagent.id.as_str())));
            if let Some(model) = &subagent.model {
                card = card.model(model.clone());
                if let Some(mark) = atelier_agents::registry::model_mark(model) {
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
            // A question with choices is answered on its own card, not allowed or denied.
            let questions = super::question::views(&request.call.input);
            if request.call.name == atelier_agents::claude_code::ASK_QUESTION && !questions.is_empty() {
                let submit = {
                    let (session, request) = (session.clone(), request.id.clone());
                    move |answers: Vec<(SharedString, SharedString)>, _: &mut Window, cx: &mut App| {
                        let answers = answers.into_iter().map(|(question, answer)| (question.to_string(), answer.to_string())).collect();
                        session.update(cx, |s, cx| s.answer_questions(&request, answers, cx))
                    }
                };
                return Some(atelier_ui::QuestionCard::new(id("question"), questions).status(atelier_ui::QuestionStatus::Pending).on_submit(submit).into_any_element());
            }
            let preview = super::preview::preview(&request.call, &root);
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
                approval = approval.parameter("File", atelier_ui::tool_preview::relative_path(file, &root));
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

/// The conversation's rows: a list that lays out only the rows on screen. Each row has a test name, `row-<index>`.
/// Beside it, as beui's message scroller has: a rail of ticks, one for each message the reader sent, when the
/// conversation is longer than the panel; and a "Latest" button while the list has let go of the end.
pub fn rows(session: &Entity<AgentSession>, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let list_state = s.list.clone();
    let glide = s.glide.clone();
    let key = s.key.clone();
    let reply = s.reply.clone();
    let overflowing = f32::from(list_state.max_offset_for_scrollbar().y) > 1.;
    let following = glide.following();
    let entries = super::rail::entries(s.conversation.items(), &s.shown);
    let session = session.clone();
    let list = list(list_state.clone(), {
        let session = session.clone();
        move |ix, window, cx| div().debug_selector(move || format!("row-{ix}")).child(reading_width(entering(&session, ix, window, cx))).into_any_element()
    })
    .size_full();
    let rail = (overflowing && entries.len() >= 2).then(|| {
        let rows: Vec<usize> = entries.iter().map(|e| e.1).collect();
        let active = super::rail::active(&rows, list_state.logical_scroll_top().item_ix, following);
        let (jump, told) = (glide.clone(), session.clone());
        MessageRail::new(gpui_kit::ElementId::Name(format!("{key}-rail").into()), entries.into_iter().map(|e| e.0).collect(), active).on_select(
            move |i, _, cx| {
                // The reader chose a message: the list glides to it and stops following the output, unless it is the last.
                if i + 1 == rows.len() {
                    jump.follow();
                } else {
                    jump.go_to(rows[i]);
                }
                told.update(cx, |_, cx| cx.notify());
            },
        )
    });
    let latest = (overflowing && !following).then(|| {
        let (state, told) = (glide.clone(), session.clone());
        div().absolute().bottom(px(8.)).left_0().right_0().flex().justify_center().child(
            Button::new(gpui_kit::ElementId::Name(format!("{key}-latest").into()))
                .debug_name("latest")
                .icon(IconName::ArrowDownward)
                .label("Latest")
                .variant(ButtonVariant::Secondary)
                .size(atelier_ui::ButtonSize::Sm)
                .on_click(move |_, _, cx| {
                    state.follow();
                    told.update(cx, |_, cx| cx.notify());
                }),
        )
    });
    // Runs after the list has laid out, so the glide sees this frame's heights.
    let tick = gpui_kit::canvas(move |_, window, cx| glide.tick(&list_state, cx.reduce_motion(), window), |_, _, _, _| {}).absolute().size_0();
    // The reply to selected words is over the list: a selection that ends in the conversation offers it.
    div().relative().size_full().child(list).child(tick).children(rail).children(latest).child(reply).into_any_element()
}

/// Row `ix`, rising and fading in as beui's messages do when it came in live a moment ago.
fn entering(session: &Entity<AgentSession>, ix: usize, window: &mut Window, cx: &App) -> AnyElement {
    let s = session.read(cx);
    let arrived = s.shown.get(ix).and_then(|&row| s.arrived.get(&crate::list_diff::Arrival::of(row))).copied();
    let body = row(session, ix, cx);
    let Some(at) = arrived else { return body };
    let f = atelier_ui::message_pop::frame(at.elapsed().as_secs_f32(), cx.reduce_motion());
    if f.settled {
        // The same name as while it moves, so a reader of the screen finds the row whenever it looks.
        return div().debug_selector(move || format!("entering-{ix}")).child(body).into_any_element();
    }
    window.request_animation_frame();
    div().debug_selector(move || format!("entering-{ix}")).relative().top(px(f.y)).opacity(f.opacity).child(body).into_any_element()
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
    let rows = rows.unwrap_or_else(|| super::rows(session, cx));
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
                    div().font_family(atelier_ui::typography::MONO_FONT_FAMILY).text_color(muted).whitespace_normal().child(t)
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
    let continues = s.continues().map(|source| format!("Continues “{}”", short_title(&source.title)));
    let (heading, words): (SharedString, SharedString) = match (continues, starting) {
        (Some(heading), _) if s.continues_natively() => (heading.into(), format!("{agent_name} picks it up where it stopped, on this account.").into()),
        (Some(heading), _) => (heading.into(), format!("Its conversation goes to {agent_name} with your first message.").into()),
        (None, true) => ("Starting…".into(), format!("Ask {agent_name} anything about this project.").into()),
        (None, false) => ("A new session".into(), format!("Ask {agent_name} anything about this project.").into()),
    };
    let body = if empty {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .px(px(24.))
            .text_center()
            .child(div().debug_selector(|| "session-heading".into()).text_size(TextSize::Sm.font_size()).child(heading))
            .child(div().text_size(TextSize::Xs.font_size()).text_color(muted).child(words))
            .children(s.can_choose_agent().then(|| agent_picker(session, cx)).flatten())
            .into_any_element()
    } else {
        div().flex_1().min_h_0().pt(px(12.)).child(rows).into_any_element()
    };
    let composer = s.composer.clone();
    let changed = changed_files(session, cx);
    let pull_card = session.read(cx).pull_card.clone();
    let sign_in = super::sign_in::sign_in_notice(session, window, cx);
    let limit = super::limit::limit_notice(session, window, cx);
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
        .child(reading_width(div().flex().flex_col().children(failure).children(sign_in).children(limit).into_any_element()))
        .child(reading_width(
            div()
                .debug_selector(|| "session-foot".into())
                .flex()
                .flex_col()
                .gap(px(atelier_ui::STACK_GAP))
                .px(px(12.))
                .pb(px(12.))
                .children(todos)
                .child(strip)
                .children(pull_card)
                .children(changed)
                .child(composer)
                .into_any_element(),
        ))
        .into_any_element()
}

/// `content` at most [`READING_WIDTH`] wide, centred in the width it has.
fn reading_width(content: AnyElement) -> AnyElement {
    div().w_full().flex().justify_center().child(div().w_full().max_w(px(READING_WIDTH)).child(content)).into_any_element()
}

/// The files the whole session changed, folded to a header with Review, which opens the session's review.
fn changed_files(session: &Entity<AgentSession>, cx: &App) -> Option<AnyElement> {
    let s = session.read(cx);
    let files = s.changed_files();
    if files.is_empty() {
        return None;
    }
    let (review, open) = (session.clone(), session.clone());
    Some(
        atelier_ui::ChangedFiles::new(gpui_kit::ElementId::Name(format!("{}-changed", s.key).into()), files.to_vec())
            .collapsible()
            .running(s.conversation.working())
            .on_review(move |path, _, cx| review.update(cx, |_, cx| cx.emit(SessionEvent::Review { turn: None, path: Some(path.to_string()) })))
            .on_open_file(move |path, _, cx| open.update(cx, |_, cx| cx.emit(SessionEvent::Review { turn: None, path: Some(path.to_string()) })))
            .into_any_element(),
    )
}

/// The agents this build can start, as a picker a new session shows until its first message.
fn agent_picker(session: &Entity<AgentSession>, cx: &App) -> Option<AnyElement> {
    let agents = atelier_agents::registry::agents();
    let s = session.read(cx);
    let provider = provider_picker(session, cx);
    let fill = cx.theme().card_strong;
    // One control in two parts: the agent, and where it runs, joined as a button group is.
    let agent_corners = if provider.is_some() { atelier_ui::button_group::segment_corners(0, 2, gpui_kit::Axis::Horizontal) } else { segment_corners_all() };
    if agents.len() < 2 && provider.is_none() {
        return None;
    }
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
            .flex()
            .items_center()
            .gap(px(atelier_ui::button_group::SEAM))
            .child(
                div().w(px(200.)).child(
                    Select::new(gpui_kit::ElementId::Name(format!("{}-agent", s.key).into()), options)
                        .corners(agent_corners)
                        .fill(fill)
                        .selected(current)
                        .on_change(move |ix, _, cx| {
                            let Some(backend) = backends.get(ix).cloned() else { return };
                            pick.update(cx, |_, cx| cx.emit(SessionEvent::ChooseAgent(backend)));
                        }),
                ),
            )
            .children(provider)
            .into_any_element(),
    )
}

fn segment_corners_all() -> gpui_kit::Corners<bool> {
    gpui_kit::Corners { top_left: true, top_right: true, bottom_left: true, bottom_right: true }
}

/// Where the agent runs, for an agent with a choice of provider: the session's accounts and OpenRouter.
fn provider_picker(session: &Entity<AgentSession>, cx: &App) -> Option<AnyElement> {
    let s = session.read(cx);
    let current = s.provider.clone()?;
    let choices = s.provider_choices();
    let words: Vec<SelectOption> = choices
        .iter()
        .map(|c| {
            let option = SelectOption::from(crate::providers::label(c, &s.provider_accounts));
            match crate::providers::mark(c) {
                Some(mark) => option.mark(mark),
                None => option,
            }
        })
        .collect();
    let pick = session.clone();
    Some(
        div()
            .flex_none()
            .debug_selector(|| "provider-picker".into())
            .child(
                Select::new(gpui_kit::ElementId::Name(format!("{}-provider", s.key).into()), words)
                    .corners(atelier_ui::button_group::segment_corners(1, 2, gpui_kit::Axis::Horizontal))
                    .fill(cx.theme().card_strong)
                    // As wide as the account's name, with no chevron: the menu still opens at a readable width.
                    .chevron(false)
                    .panel_width(px(180.))
                    .selected(choices.iter().position(|c| *c == current))
                    .on_change(move |ix, _, cx| {
                        let Some(choice) = choices.get(ix).cloned() else { return };
                        pick.update(cx, |s, cx| s.set_provider(choice, cx));
                    }),
            )
            .into_any_element(),
    )
}

/// The panel's top line: the title (a press renames it), what the session is doing, and Stop while
/// its agent runs.
/// A9: Stop shows while a turn goes on (working, or waiting for the reader), not while the agent idles.
pub(super) fn shows_stop(running: bool, status: &SessionStatus) -> bool {
    running && matches!(status, SessionStatus::Working | SessionStatus::NeedsYou(_))
}

pub(super) fn header(session: &Entity<AgentSession>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let (key, renaming, shown_title, running, task, project, badge, session_id, agent) = {
        let s = session.read(cx);
        let agent = atelier_ui::session_row::agent_icon(gpui_kit::ElementId::Name(format!("{}-agent-mark", s.key).into()), &s.agent.look, &s.status, &theme, true);
        (
            s.key.clone(),
            s.renaming.clone(),
            s.shown_title(),
            shows_stop(s.running(), &s.status),
            s.task.clone(),
            s.project_name(),
            s.badge.clone(),
            s.id.as_ref().map(|i| i.as_str().to_string()),
            agent,
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
                .tooltip(atelier_ui::tooltip::Tooltip::text("Rename"))
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
            .size(atelier_ui::ButtonSize::Sm)
            .tooltip("Open the task")
            .on_click(move |_, _, cx| open.update(cx, |_, cx| cx.emit(SessionEvent::OpenTask)))
    });
    // Where the agent runs, as the design system's model badge in a pill: the lab's mark and the account or OpenRouter. The
    // sidebar's rows carry nothing of it, so the panel is the one place that says it.
    let provider = {
        let s = session.read(cx);
        s.provider.as_ref().map(|choice| {
            let label = crate::providers::label(choice, &s.provider_accounts);
            let id = gpui_kit::ElementId::Name(format!("{key}-provider-mark").into());
            let badge = atelier_ui::model_badge::ModelBadge::new(label.clone());
            let badge = match crate::providers::mark(choice) {
                Some(mark) => badge.mark(id, mark),
                None => badge.monogram(id),
            };
            div()
                .id(gpui_kit::ElementId::Name(format!("{key}-provider").into()))
                .debug_selector(|| "panel-provider".into())
                .flex()
                .flex_none()
                .items_center()
                .h(px(22.))
                .px(px(8.))
                .rounded_full()
                .bg(theme.card_strong)
                .tooltip(atelier_ui::tooltip::Tooltip::text(format!("Runs on {label}")))
                .child(badge)
        })
    };
    let more = panel_menu(session, &key, session_id, window, cx);
    let close = {
        let close = session.clone();
        Button::new(gpui_kit::ElementId::Name(format!("{key}-close").into()))
            .debug_name("panel-close")
            .icon(IconName::Close)
            .variant(ButtonVariant::Ghost)
            .size(atelier_ui::ButtonSize::IconSm)
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
        .child(div().debug_selector(|| "panel-agent".into()).flex_none().child(agent))
        // The project the session works in, as its badge (its name stands in until the shell has set the badge), so a
        // panel always says where it is; the name is its tooltip.
        .child(match badge {
            Some(badge) => div()
                .id(gpui_kit::ElementId::Name(format!("{key}-project").into()))
                .debug_selector(|| "panel-project".into())
                .flex_none()
                .tooltip(atelier_ui::tooltip::Tooltip::text(project))
                .child(atelier_ui::project_badge::ProjectBadge::new(badge.label, badge.color).icon(badge.icon))
                .into_any_element(),
            None => div().debug_selector(|| "panel-project".into()).flex_none().max_w(px(120.)).truncate().text_size(TextSize::Xs.font_size()).text_color(theme.muted_foreground).child(project).into_any_element(),
        })
        .child(title)
        .children(provider)
        .children(chip)
        .children(stop)
        .child(more)
        .child(close)
}

/// The panel's ⋯ menu: what a reader does with a session besides talking to it. Rename it, begin another in the same
/// project, see the project's files, copy the id the agent knows it by (to resume it elsewhere), archive it.
fn panel_menu(session: &Entity<AgentSession>, key: &SharedString, session_id: Option<String>, window: &mut Window, cx: &mut App) -> impl IntoElement {
    use atelier_ui::{
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
        let item = |label: &'static str, icon: IconName, name: &'static str, run: MenuAction| {
            let open = open.clone();
            Entry::from(MenuItem::new(label).icon(icon).debug_name(name).on_select(move |window, cx| {
                open.update(cx, |o, cx| {
                    *o = false;
                    cx.notify();
                });
                run(window, cx)
            }))
        };
        let (rename, fresh, files, archive) = (session.clone(), session.clone(), session.clone(), session.clone());
        let mut entries = vec![
            item("Rename", IconName::Edit, "panel-rename", Rc::new(move |window, cx| rename.update(cx, |s, cx| s.start_rename(window, cx)))),
            item("New session in this project", IconName::Add, "panel-new-session", Rc::new(move |_, cx| fresh.update(cx, |_, cx| cx.emit(SessionEvent::NewSession)))),
            item("Open the project's files", IconName::Folder, "panel-files", Rc::new(move |_, cx| files.update(cx, |_, cx| cx.emit(SessionEvent::ShowFiles)))),
        ];
        if let Some(id) = session_id.clone() {
            entries.push(item("Copy session id", IconName::Copy, "panel-copy-id", Rc::new(move |_, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(id.clone())))));
        }
        entries.push(item("Archive", IconName::Archive, "panel-archive", Rc::new(move |_, cx| archive.update(cx, |_, cx| cx.emit(SessionEvent::Archive)))));
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
                .size(atelier_ui::ButtonSize::IconSm)
                .tooltip("More")
                .open(is_open)
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    flip(cx)
                }),
        )
        .children(menu)
}

/// How much of a session's title the heading of the session that continues it shows.
const HEADING_TITLE_MAX: usize = 60;

fn short_title(title: &str) -> String {
    match title.char_indices().nth(HEADING_TITLE_MAX) {
        Some((cut, _)) => format!("{}…", title[..cut].trim_end()),
        None => title.to_string(),
    }
}

/// The pool index the subagent `call` holds: the one it took when it first showed, else the next free one. Its colour stays with it
/// when it is done, but only the running ones hold a colour against a new subagent.
fn tint_of(s: &AgentSession, call: &str) -> usize {
    let running: Vec<&str> = s
        .conversation
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Subagent { subagent, status: SubagentStatus::Running, .. } => Some(subagent.id.as_str()),
            _ => None,
        })
        .collect();
    let mut held = s.subagent_tints.borrow_mut();
    if let Some((_, index)) = held.iter().find(|(id, _)| id == call) {
        return *index;
    }
    let taken: Vec<usize> = held.iter().filter(|(id, _)| running.contains(&id.as_str())).map(|(_, i)| *i).collect();
    let index = super::tint::next(&taken);
    held.push((call.to_string(), index));
    index
}
