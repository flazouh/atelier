//! The "Tool cards" story: a call of an agent to a gateway tool as a card, in each state and for each kind of result: a list that
//! shows ten rows and "Show more", one task, a chat and a mail result, a provider the app does not know, a call that failed,
//! one that runs, and one that waits for approval.
use atelier_ui::{
    IconName, TaskStatus, TextSize, ToolAction, ToolCard, ToolCardData, ToolCardState, ToolGap,
    ToolIcon, ToolNode, ToolProvider, ToolRow, ToolText, ToolTone,
};
use gpui_kit::{IntoElement, ParentElement, SharedString, Styled, div, px};

fn provider(name: &str, account: &str, letter: &str, color: Option<usize>) -> ToolProvider {
    ToolProvider {
        name: name.to_string().into(),
        account: Some(account.to_string().into()),
        letter: letter.to_string().into(),
        color,
    }
}

fn text(value: &str, style: ToolText) -> ToolNode {
    ToolNode::Text {
        value: value.to_string().into(),
        style,
        max_lines: Some(1),
    }
}

fn row(children: Vec<ToolNode>) -> ToolNode {
    ToolNode::Stack {
        row: true,
        gap: ToolGap::Sm,
        children,
    }
}

fn task_row(
    key: &str,
    title: &str,
    status: TaskStatus,
    priority: Option<(&str, ToolTone)>,
) -> ToolRow {
    let mut children = vec![
        ToolNode::Icon {
            icon: ToolIcon::Status(status),
            tone: ToolTone::Neutral,
        },
        text(key, ToolText::Muted),
        text(title, ToolText::Title),
    ];
    children.extend(priority.map(|(label, tone)| ToolNode::Badge {
        value: label.to_string().into(),
        tone,
    }));
    ToolRow {
        node: row(children),
        action: Some("open".into()),
    }
}

fn message_row(name: &str, origin: Option<&str>, when: &str, body: &str) -> ToolRow {
    let mut head = vec![ToolNode::Text { value: name.to_string().into(), style: ToolText::Title, max_lines: None }];
    head.extend(origin.map(|o| ToolNode::Badge {
        value: o.to_string().into(),
        tone: ToolTone::Neutral,
    }));
    head.push(text(when, ToolText::Muted));
    ToolRow {
        node: ToolNode::Stack {
            row: true,
            gap: ToolGap::Md,
            children: vec![
                ToolNode::Avatar {
                    name: name.to_string().into(),
                },
                ToolNode::Stack {
                    row: false,
                    gap: ToolGap::Xs,
                    children: vec![
                        row(head),
                        ToolNode::Text {
                            value: body.to_string().into(),
                            style: ToolText::Body,
                            max_lines: Some(3),
                        },
                    ],
                },
            ],
        },
        action: Some("open".into()),
    }
}

fn card(id: &'static str, data: ToolCardData) -> impl IntoElement {
    ToolCard::new(id, data)
}

fn open(label: &str) -> ToolAction {
    ToolAction {
        label: label.to_string().into(),
        key: "open".into(),
    }
}

fn data(
    provider: ToolProvider,
    title: &str,
    state: ToolCardState,
    body: Option<ToolNode>,
    footer: Vec<ToolAction>,
) -> ToolCardData {
    ToolCardData {
        provider,
        title: title.to_string().into(),
        state,
        origin: Some("Alex's agent".into()),
        body,
        footer,
    }
}

fn section(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .pb(px(24.))
        .child(
            div()
                .text_size(TextSize::Xs.font_size())
                .opacity(0.6)
                .child(SharedString::from(title)),
        )
        .child(div().max_w(px(560.)).child(content))
}

pub fn tool_cards_story() -> impl IntoElement {
    let linear = || provider("Linear", "acme", "L", Some(9));
    let statuses = [
        TaskStatus::InProgress,
        TaskStatus::Todo,
        TaskStatus::InReview,
        TaskStatus::Backlog,
        TaskStatus::Done,
    ];
    let tasks: Vec<ToolRow> = (1..=14)
        .map(|n| {
            let priority = match n % 4 {
                0 => Some(("Urgent", ToolTone::Danger)),
                1 => Some(("High", ToolTone::Warning)),
                _ => None,
            };
            task_row(
                &format!("ENG-{n}"),
                &format!(
                    "A task number {n} with a title that can be long enough to run out of room"
                ),
                statuses[n % 5],
                priority,
            )
        })
        .collect();
    let list = ToolNode::List {
        rows: tasks,
        hidden: 0,
        empty: Some("No tasks match.".into()),
    };
    let one = ToolNode::Stack {
        row: true,
        gap: ToolGap::Sm,
        children: vec![
            ToolNode::Icon {
                icon: ToolIcon::Status(TaskStatus::Todo),
                tone: ToolTone::Neutral,
            },
            text("ENG-142", ToolText::Muted),
            text("The login shows a blank page on Safari", ToolText::Title),
            ToolNode::Badge {
                value: "High".into(),
                tone: ToolTone::Warning,
            },
        ],
    };
    let chat = ToolNode::List {
        rows: vec![
            message_row(
                "Claude Code",
                Some("Alex's agent"),
                "5 min ago",
                "I ran the checks on the release branch. All of them pass.",
            ),
            message_row(
                "Ana",
                None,
                "4 hours ago",
                "Here is the report from the last run.",
            ),
            message_row(
                "Ben",
                None,
                "4 hours ago",
                "Does anyone know why the gateway port changes on every start? It was fixed last week and now it moves again.",
            ),
        ],
        hidden: 0,
        empty: None,
    };
    let mail = ToolNode::List {
        rows: vec![
            ToolRow {
                node: ToolNode::Stack {
                    row: true,
                    gap: ToolGap::Md,
                    children: vec![
                        ToolNode::Icon {
                            icon: ToolIcon::Named(IconName::Mail),
                            tone: ToolTone::Neutral,
                        },
                        ToolNode::Stack {
                            row: false,
                            gap: ToolGap::Xs,
                            children: vec![
                                text("Quarterly planning", ToolText::Title),
                                text(
                                    "dan@example.com · Never mind, I decided. Option two.",
                                    ToolText::Muted,
                                ),
                            ],
                        },
                        text("9 hours ago", ToolText::Muted),
                    ],
                },
                action: Some("open".into()),
            },
            ToolRow {
                node: ToolNode::Stack {
                    row: true,
                    gap: ToolGap::Md,
                    children: vec![
                        ToolNode::Icon {
                            icon: ToolIcon::Named(IconName::Mail),
                            tone: ToolTone::Neutral,
                        },
                        ToolNode::Stack {
                            row: false,
                            gap: ToolGap::Xs,
                            children: vec![
                                text("Invoice 2041", ToolText::Title),
                                text(
                                    "billing@example.com · The invoice for October is attached.",
                                    ToolText::Muted,
                                ),
                            ],
                        },
                        text("1 day ago", ToolText::Muted),
                    ],
                },
                action: Some("open".into()),
            },
        ],
        hidden: 0,
        empty: None,
    };
    div()
        .flex()
        .flex_col()
        .child(section(
            "A list: ten rows, then Show more",
            card(
                "tc-list",
                data(
                    linear(),
                    "Searched Linear, 14 tasks",
                    ToolCardState::Done,
                    Some(list),
                    Vec::new(),
                ),
            ),
        ))
        .child(section(
            "One task, with an action",
            card(
                "tc-one",
                data(
                    linear(),
                    "Created ENG-142 in Linear",
                    ToolCardState::Done,
                    Some(one),
                    vec![open("Open task")],
                ),
            ),
        ))
        .child(section(
            "Chat",
            card(
                "tc-chat",
                data(
                    provider("Slack", "acme", "S", Some(11)),
                    "Read 3 messages in Slack",
                    ToolCardState::Done,
                    Some(chat),
                    vec![open("Open channel")],
                ),
            ),
        ))
        .child(section(
            "Mail",
            card(
                "tc-mail",
                data(
                    provider("Gmail", "me@example.com", "G", Some(0)),
                    "Searched Gmail, 2 threads",
                    ToolCardState::Done,
                    Some(mail),
                    Vec::new(),
                ),
            ),
        ))
        .child(section(
            "A provider the app does not know: a neutral tile",
            card(
                "tc-unknown",
                data(
                    provider("Jira", "acme", "J", None),
                    "Listed no tasks in Jira",
                    ToolCardState::Done,
                    Some(ToolNode::List {
                        rows: Vec::new(),
                        hidden: 0,
                        empty: Some("No tasks match.".into()),
                    }),
                    Vec::new(),
                ),
            ),
        ))
        .child(section(
            "Running",
            card(
                "tc-running",
                data(
                    linear(),
                    "Listing tasks…",
                    ToolCardState::Running,
                    None,
                    Vec::new(),
                ),
            ),
        ))
        .child(section(
            "Waiting for approval",
            card(
                "tc-waiting",
                data(
                    linear(),
                    "Creating a task…",
                    ToolCardState::Waiting,
                    None,
                    Vec::new(),
                ),
            ),
        ))
        .child(section(
            "Failed",
            card(
                "tc-failed",
                data(
                    linear(),
                    "Could not create the task",
                    ToolCardState::Failed,
                    Some(ToolNode::Text {
                        value: "Could not create the task: Linear is offline.".into(),
                        style: ToolText::Muted,
                        max_lines: Some(3),
                    }),
                    Vec::new(),
                ),
            ),
        ))
}
