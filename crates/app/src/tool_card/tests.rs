use atelier_agents::session::{Call, ToolCall, ToolId, ToolKind, ToolOutput, ToolStatus};
use atelier_capabilities::card::{ResolvedAction, from_json};
use atelier_ui::{ToolCardState, ToolNode};
use serde_json::{Value, json};

use super::{
    build,
    cards::{self, fits, shape, tool_of},
    fixtures::{self, Done},
    map,
    providers::{Place, place_of, provider},
    types::{PREFIX, Press, TOOLS},
};

fn card_for(
    c: &Call,
    waiting: bool,
    origin: Option<String>,
    now: i64,
) -> Option<super::types::Built> {
    build::card_for(&c.call, c.output.as_ref(), waiting, origin, now)
}

const NOW: i64 = 1_791_547_600_000;

fn call(name: &str, input: Value, status: ToolStatus, output: Option<(&str, bool)>) -> Call {
    Call {
        call: ToolCall {
            id: ToolId::new("t1"),
            name: name.into(),
            kind: ToolKind::Other,
            input,
            file: None,
            parent: None,
            status,
        },
        output: output.map(|(text, is_error)| ToolOutput {
            text: text.into(),
            is_error,
            truncated: false,
            full_at: None,
        }),
        edit: None,
    }
}

fn finished(done: &Done) -> Call {
    call(
        &format!("{PREFIX}{}", done.tool),
        done.args.clone(),
        ToolStatus::Done,
        Some((&done.text, false)),
    )
}

fn drawn(tool: &str) -> super::types::Built {
    let all = fixtures::all();
    let done = all
        .iter()
        .find(|d| d.tool == tool)
        .unwrap_or_else(|| panic!("no fixture for {tool}"));
    card_for(&finished(done), false, Some("Alex's agent".into()), NOW)
        .unwrap_or_else(|| panic!("{tool} draws no card"))
}

fn rows(node: &Option<ToolNode>) -> usize {
    match node {
        Some(ToolNode::List { rows, .. }) => rows.len(),
        _ => 0,
    }
}

#[test]
fn every_built_in_card_is_valid_against_the_limits() {
    assert_eq!(cards::sources().len(), TOOLS.len());
    for (tool, json) in cards::sources() {
        let card = from_json(json).unwrap_or_else(|e| panic!("{tool}: {e}"));
        assert_eq!(&card.tool, tool, "a card is named for its tool");
        assert!(cards::card(tool).is_some(), "{tool} is loaded");
        assert!(shape(tool).is_some(), "{tool} has a shape");
        assert!(
            card.running.is_some() && card.failed.is_some(),
            "{tool} has all three states"
        );
    }
    for tool in TOOLS {
        assert!(
            cards::sources().iter().any(|(t, _)| *t == tool),
            "{tool} has a card"
        );
    }
}

#[test]
fn no_built_in_card_calls_a_tool_or_opens_a_link() {
    for (tool, json) in cards::sources() {
        assert!(
            !json.contains("\"call\"") && !json.contains("\"open_url\""),
            "{tool} has an action that is not Open"
        );
    }
}

#[test]
fn a_card_never_gives_a_press_that_calls_or_opens_a_link() {
    let mut presses = Vec::new();
    let call = ResolvedAction::Call {
        label: "Close".into(),
        tool: "tasks_update".into(),
        args: Default::default(),
    };
    let link = ResolvedAction::OpenUrl {
        label: "Site".into(),
        url: "https://example.com".into(),
    };
    let open = ResolvedAction::Open {
        label: "Open".into(),
        reference: "tasks:linear:acme:ENG-1".into(),
    };
    let footer = map::footer(&[call, link, open], &mut presses);
    assert_eq!(footer.len(), 1, "only Open is kept");
    assert_eq!(footer[0].label, "Open");
    assert_eq!(presses.len(), 1);
    // A reference that is not one is no press either.
    let bad = ResolvedAction::Open {
        label: "Open".into(),
        reference: "nonsense".into(),
    };
    assert!(map::footer(&[bad], &mut presses).is_empty());
}

#[test]
fn only_the_atelier_gateway_tools_are_cards() {
    assert_eq!(tool_of("mcp__atelier__tasks_list"), Some("tasks_list"));
    assert_eq!(
        tool_of("mcp__atelier__tasks_delete"),
        None,
        "a tool of the server that has no card"
    );
    assert_eq!(
        tool_of("mcp__other__tasks_list"),
        None,
        "another server's tool of the same name"
    );
    assert_eq!(tool_of("Bash"), None);
}

#[test]
fn every_tool_draws_the_result_the_gateway_gives() {
    let all = fixtures::all();
    assert_eq!(all.len(), TOOLS.len(), "a fixture for each tool");
    for done in &all {
        assert!(fits(done.tool, &done.value), "{} fits its shape", done.tool);
        let built = card_for(&finished(done), false, Some("Alex's agent".into()), NOW)
            .unwrap_or_else(|| panic!("{} has no card", done.tool));
        assert_eq!(built.data.state, ToolCardState::Done, "{}", done.tool);
        assert!(
            !built.data.title.is_empty() && !built.data.title.contains("{$"),
            "{}: {}",
            done.tool,
            built.data.title
        );
        assert!(built.data.body.is_some(), "{} has a body", done.tool);
        assert_eq!(built.data.origin.as_deref(), Some("Alex's agent"));
        assert!(built.data.footer.len() <= 3);
    }
}

#[test]
fn the_titles_say_what_the_call_did_and_where() {
    assert_eq!(drawn("tasks_list").data.title, "Listed 2 tasks in Memory");
    assert_eq!(drawn("tasks_search").data.title, "Searched Memory, 1 task");
    assert_eq!(drawn("tasks_get").data.title, "Read MEM-2 in Memory");
    assert_eq!(drawn("tasks_create").data.title, "Created MEM-3 in Memory");
    assert_eq!(drawn("tasks_update").data.title, "Updated MEM-2 in Memory");
    assert_eq!(
        drawn("messaging_channels").data.title,
        "Listed 1 channel in Memory"
    );
    assert_eq!(
        drawn("messaging_history").data.title,
        "Read 2 messages in Memory"
    );
    assert_eq!(
        drawn("messaging_send").data.title,
        "Sent a message in Memory"
    );
    assert_eq!(drawn("mail_search").data.title, "Searched Memory, 1 thread");
    assert_eq!(drawn("mail_thread").data.title, "Read “Q3 plan” in Memory");
}

#[test]
fn a_list_has_a_row_for_each_item_and_a_press_that_opens_it() {
    let built = drawn("tasks_list");
    assert_eq!(rows(&built.data.body), 2);
    let Some(ToolNode::List { rows, .. }) = &built.data.body else {
        panic!("a list")
    };
    assert!(
        rows.iter().all(|r| r.action.is_some()),
        "each row opens its task"
    );
    assert!(
        built
            .presses
            .contains(&Press::Open("tasks:memory:smoke:MEM-2".parse().unwrap()))
    );
    let key = rows[0].action.as_ref().unwrap();
    assert_eq!(
        built.presses[key.parse::<usize>().unwrap()],
        Press::Open("tasks:memory:smoke:MEM-2".parse().unwrap())
    );
}

#[test]
fn a_task_card_opens_its_task_and_a_send_opens_its_channel() {
    let created = drawn("tasks_create");
    assert_eq!(created.data.footer.len(), 1);
    assert_eq!(
        created.presses,
        [Press::Open("tasks:memory:smoke:MEM-3".parse().unwrap())]
    );
    let sent = drawn("messaging_send");
    assert_eq!(sent.data.footer[0].label, "Open channel");
    assert_eq!(
        sent.presses,
        [Press::Open("messaging:memory:smoke:C1".parse().unwrap())]
    );
    let thread = drawn("mail_thread");
    assert_eq!(
        thread.presses.last(),
        Some(&Press::Open(
            "mail:memory:me@example.com:t:1".parse().unwrap()
        ))
    );
}

#[test]
fn a_message_from_an_agent_shows_where_it_came_from() {
    fn has_origin(node: &ToolNode) -> bool {
        match node {
            ToolNode::Badge { value, .. } => value.contains("agent"),
            ToolNode::Stack { children, .. } => children.iter().any(has_origin),
            ToolNode::List { rows, .. } => rows.iter().any(|r| has_origin(&r.node)),
            _ => false,
        }
    }
    assert!(has_origin(
        drawn("messaging_send").data.body.as_ref().unwrap()
    ));
    assert!(
        !has_origin(drawn("messaging_history").data.body.as_ref().unwrap()),
        "a person's message has no origin"
    );
}

#[test]
fn a_result_of_the_wrong_shape_keeps_the_plain_row() {
    for tool in TOOLS {
        let name = format!("{PREFIX}{tool}");
        for text in [
            "",
            "not json",
            "[]",
            "{}",
            "{\"items\":\"x\"}",
            "{\"items\":[1,2]}",
            "{\"task\":3}",
            "{\"other\":{}}",
            "null",
        ] {
            let c = call(&name, json!({}), ToolStatus::Done, Some((text, false)));
            assert!(
                card_for(&c, false, None, NOW).is_none(),
                "{tool} with {text:?}"
            );
        }
        // A result cut short is not JSON any more.
        let mut cut = call(
            &name,
            json!({}),
            ToolStatus::Done,
            Some(("{\"items\":[{\"ref\":\"tasks:a:b:", false)),
        );
        cut.output.as_mut().unwrap().truncated = true;
        assert!(card_for(&cut, false, None, NOW).is_none(), "{tool} cut");
    }
}

#[test]
fn a_call_with_no_card_keeps_the_plain_row() {
    let c = call(
        "Bash",
        json!({"command": "ls"}),
        ToolStatus::Done,
        Some(("a b", false)),
    );
    assert!(card_for(&c, false, None, NOW).is_none());
    let c = call(
        "mcp__atelier__tasks_delete",
        json!({}),
        ToolStatus::Running,
        None,
    );
    assert!(card_for(&c, false, None, NOW).is_none());
    let done_without_output = call(
        "mcp__atelier__tasks_list",
        json!({}),
        ToolStatus::Done,
        None,
    );
    assert!(
        card_for(&done_without_output, false, None, NOW).is_none(),
        "nothing came back, so there is nothing to draw"
    );
}

#[test]
fn a_running_call_shows_its_title_and_state() {
    let c = call(
        "mcp__atelier__tasks_create",
        json!({"title": "X", "account": "linear/acme"}),
        ToolStatus::Running,
        None,
    );
    let built = card_for(&c, false, Some("Alex's agent".into()), NOW).unwrap();
    assert_eq!(built.data.state, ToolCardState::Running);
    assert_eq!(built.data.title, "Creating a task…");
    assert_eq!(
        built.data.provider.name, "Linear",
        "the account argument says where it goes"
    );
    assert_eq!(built.data.provider.account.as_deref(), Some("acme"));
    assert!(built.data.body.is_none());
}

#[test]
fn a_call_that_waits_for_approval_shows_its_title_and_state() {
    let c = call(
        "mcp__atelier__messaging_send",
        json!({"channel": "messaging:slack:acme:C1", "text": "hi"}),
        ToolStatus::Pending,
        None,
    );
    let built = card_for(&c, true, None, NOW).unwrap();
    assert_eq!(built.data.state, ToolCardState::Waiting);
    assert_eq!(built.data.title, "Sending a message…");
    assert_eq!(
        built.data.provider.name, "Slack",
        "a reference in the arguments says where it goes"
    );
}

#[test]
fn a_failed_call_says_why() {
    let c = call(
        "mcp__atelier__tasks_create",
        json!({}),
        ToolStatus::Failed,
        Some(("Could not create the task: Linear is offline.", true)),
    );
    let built = card_for(&c, false, None, NOW).unwrap();
    assert_eq!(built.data.state, ToolCardState::Failed);
    assert_eq!(built.data.title, "Could not create the task");
    let Some(ToolNode::Text { value, .. }) = &built.data.body else {
        panic!("the reason")
    };
    assert_eq!(value, "Could not create the task: Linear is offline.");
    // Failed with nothing said still draws a card.
    let c = call(
        "mcp__atelier__tasks_create",
        json!({}),
        ToolStatus::Failed,
        None,
    );
    assert_eq!(
        card_for(&c, false, None, NOW).unwrap().data.state,
        ToolCardState::Failed
    );
}

#[test]
fn a_reference_names_its_provider() {
    let named = |reference: &str| {
        let place = place_of(Some(&json!({"items": [{"ref": reference}]})), &json!({}));
        provider("tasks", place.as_ref())
    };
    let (linear, known) = named("tasks:linear:acme:ENG-1");
    assert!(known);
    assert_eq!(
        (
            linear.name.as_ref(),
            linear.account.as_deref(),
            linear.letter.as_ref()
        ),
        ("Linear", Some("acme"), "L")
    );
    assert!(linear.color.is_some());
    for (reference, name) in [
        ("tasks:local:atelier:LAT-1", "Atelier"),
        ("tasks:github:flazouh.atelier:12", "GitHub"),
        ("messaging:slack:acme:C1", "Slack"),
        ("messaging:discord:guild:C1", "Discord"),
        ("mail:gmail:me@example.com:t:1", "Gmail"),
    ] {
        assert_eq!(named(reference).0.name.as_ref(), name, "{reference}");
    }
}

#[test]
fn a_provider_that_is_not_known_has_a_neutral_tile() {
    let (unknown, known) = provider(
        "tasks",
        Some(&Place {
            provider: "jira".into(),
            account: "acme".into(),
        }),
    );
    assert!(known, "the call said where it went");
    assert_eq!(
        (
            unknown.name.as_ref(),
            unknown.letter.as_ref(),
            unknown.color
        ),
        ("Jira", "J", None)
    );
    let (none, known) = provider("messaging", None);
    assert!(!known);
    assert_eq!((none.name.as_ref(), none.color), ("Messages", None));
}

#[test]
fn a_ref_that_is_not_one_names_no_place() {
    assert_eq!(
        place_of(
            Some(&json!({"items": [{"ref": "nonsense"}]})),
            &json!({"ref": "x"})
        ),
        None
    );
    assert_eq!(
        place_of(None, &json!({"account": "linear/acme"})),
        Some(Place {
            provider: "linear".into(),
            account: "acme".into()
        })
    );
}

#[test]
fn text_from_a_task_is_drawn_as_text() {
    // A title that looks like markup, or a template hole, is data and stays what it is.
    let mut result = fixtures::all()
        .into_iter()
        .find(|d| d.tool == "tasks_create")
        .unwrap();
    result.value["task"]["title"] = json!("<b>{$._provider.name}</b> [x](http://e.com)");
    let text = result.value.to_string();
    let c = call(
        "mcp__atelier__tasks_create",
        json!({}),
        ToolStatus::Done,
        Some((&text, false)),
    );
    let built = card_for(&c, false, None, NOW).unwrap();
    let ToolNode::Stack { children, .. } = built.data.body.as_ref().unwrap() else {
        panic!("a row")
    };
    let shown: Vec<String> = children
        .iter()
        .filter_map(|c| match c {
            ToolNode::Text { value, .. } => Some(value.to_string()),
            _ => None,
        })
        .collect();
    assert!(
        shown.contains(&"<b>{$._provider.name}</b> [x](http://e.com)".to_string()),
        "{shown:?}"
    );
}
