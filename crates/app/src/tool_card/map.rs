//! From the tree the card resolver gives to the tree the card draws. A press that opens something becomes a number the card gives
//! back; a press that would call a tool, or open a link, is dropped, so a card never does more than open what its result names.
use atelier_capabilities::{
    Ref,
    card::{Direction, Gap, Resolved, ResolvedAction, Style, Tone},
};
use atelier_ui::{
    IconName, TaskStatus, ToolAction, ToolGap, ToolIcon, ToolNode, ToolRow, ToolText, ToolTone,
};

use super::types::Press;

/// The tree a card draws, and the presses it can give back, in the order of their numbers.
pub fn node(resolved: &Resolved, presses: &mut Vec<Press>) -> Option<ToolNode> {
    Some(match resolved {
        Resolved::Stack {
            direction,
            gap,
            children,
        } => ToolNode::Stack {
            row: *direction == Direction::Row,
            gap: match gap {
                Gap::Xs => ToolGap::Xs,
                Gap::Sm => ToolGap::Sm,
                Gap::Md => ToolGap::Md,
            },
            children: children.iter().filter_map(|c| node(c, presses)).collect(),
        },
        Resolved::Text {
            value,
            style,
            max_lines,
        } => ToolNode::Text {
            value: value.clone().into(),
            style: match style {
                Style::Body => ToolText::Body,
                Style::Title => ToolText::Title,
                Style::Muted => ToolText::Muted,
                Style::Code => ToolText::Code,
            },
            max_lines: *max_lines,
        },
        Resolved::Badge { value, tone } => ToolNode::Badge {
            value: value.clone().into(),
            tone: tone_of(*tone),
        },
        Resolved::Metric { label, value } => ToolNode::Metric {
            label: label.clone().into(),
            value: value.clone().into(),
        },
        Resolved::Icon { name, tone } => ToolNode::Icon {
            icon: icon_of(name),
            tone: tone_of(*tone),
        },
        // The picture of an avatar is not fetched: the card draws the letter, as the Messages screen does.
        Resolved::Avatar { name, .. } => ToolNode::Avatar {
            name: name.clone().into(),
        },
        Resolved::Code {
            value, max_lines, ..
        } => ToolNode::Code {
            value: value.clone().into(),
            max_lines: *max_lines,
        },
        Resolved::List {
            rows,
            hidden,
            empty,
        } => ToolNode::List {
            rows: rows
                .iter()
                .filter_map(|row| {
                    Some(ToolRow {
                        node: node(&row.node, presses)?,
                        action: row.on_click.as_ref().and_then(|a| press(a, presses)),
                    })
                })
                .collect(),
            hidden: *hidden,
            empty: empty.clone().map(Into::into),
        },
        Resolved::Button { action } => {
            let (label, key) = action_of(action, presses)?;
            ToolNode::Button {
                action: ToolAction { label, key },
            }
        }
        Resolved::Divider => ToolNode::Divider,
    })
}

/// The footer of a card: its buttons, at most three.
pub fn footer(actions: &[ResolvedAction], presses: &mut Vec<Press>) -> Vec<ToolAction> {
    actions
        .iter()
        .filter_map(|a| action_of(a, presses))
        .take(atelier_ui::tool_card::FOOTER_MAX)
        .map(|(label, key)| ToolAction { label, key })
        .collect()
}

fn action_of(
    action: &ResolvedAction,
    presses: &mut Vec<Press>,
) -> Option<(gpui_kit::SharedString, gpui_kit::SharedString)> {
    let ResolvedAction::Open { label, .. } = action else {
        return None;
    };
    Some((label.clone().into(), press(action, presses)?))
}

/// The number of the press for `action`, or `None` when the action is not one a card may do.
fn press(action: &ResolvedAction, presses: &mut Vec<Press>) -> Option<gpui_kit::SharedString> {
    let ResolvedAction::Open { reference, .. } = action else {
        return None;
    };
    let reference: Ref = reference.parse().ok()?;
    presses.push(Press::Open(reference));
    Some((presses.len() - 1).to_string().into())
}

fn tone_of(tone: Tone) -> ToolTone {
    match tone {
        Tone::Neutral => ToolTone::Neutral,
        Tone::Info => ToolTone::Info,
        Tone::Success => ToolTone::Success,
        Tone::Warning => ToolTone::Warning,
        Tone::Danger => ToolTone::Danger,
    }
}

/// The icon a card names. The marks of a task's status are `status_todo` and its kin; any other name is a name of the icon set
/// or one of a few short names, and a name that is none of them shows a plain circle.
fn icon_of(name: &str) -> ToolIcon {
    let status = match name {
        "status_backlog" => Some(TaskStatus::Backlog),
        "status_todo" => Some(TaskStatus::Todo),
        "status_in_progress" => Some(TaskStatus::InProgress),
        "status_in_review" => Some(TaskStatus::InReview),
        "status_done" => Some(TaskStatus::Done),
        "status_canceled" => Some(TaskStatus::Canceled),
        _ => None,
    };
    if let Some(status) = status {
        return ToolIcon::Status(status);
    }
    let short = match name {
        "bug" => Some(IconName::Bug),
        "warning" => Some(IconName::Warning),
        "error" => Some(IconName::Error),
        "info" => Some(IconName::Info),
        "mail" => Some(IconName::Mail),
        "chat" => Some(IconName::ChatBubble),
        "forum" => Some(IconName::Forum),
        "lock" => Some(IconName::Lock),
        "check" => Some(IconName::CheckCircle),
        "folder" => Some(IconName::Folder),
        "file" => Some(IconName::File),
        "link" => Some(IconName::Link),
        "attachment" => Some(IconName::AttachFile),
        "clock" => Some(IconName::Schedule),
        _ => None,
    };
    ToolIcon::Named(
        short
            .or_else(|| IconName::ALL.iter().copied().find(|i| i.name() == name))
            .unwrap_or(IconName::Circle),
    )
}
