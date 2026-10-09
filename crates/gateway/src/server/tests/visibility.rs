//! Which tools `tools/list` shows: only those that some connected account can do.
use std::sync::Arc;

use atelier_capabilities::{
    Registry,
    mail::MemoryMail,
    messaging::{MemoryMessaging, MessagingCapabilities},
    tasks::MemoryTasks,
};
use serde_json::json;

use super::{fakes::FakeChat, fixture, with_registry, without_tasks};

const TASKS: [&str; 6] = [
    "tasks_list",
    "tasks_get",
    "tasks_create",
    "tasks_update",
    "tasks_comment",
    "tasks_search",
];
const MESSAGING: [&str; 5] = [
    "messaging_channels",
    "messaging_history",
    "messaging_thread",
    "messaging_search",
    "messaging_send",
];
const MAIL: [&str; 5] = [
    "mail_mailboxes",
    "mail_search",
    "mail_thread",
    "mail_get",
    "mail_create_draft",
];

#[test]
fn a_gateway_with_only_tasks_lists_only_tasks_tools() {
    assert_eq!(fixture().tool_names(), TASKS);
}

#[test]
fn a_gateway_with_nothing_connected_lists_nothing() {
    assert!(without_tasks(Registry::new()).tool_names().is_empty());
}

#[test]
fn messaging_tools_show_with_a_messaging_account_and_mail_tools_with_a_mail_account() {
    let mut registry = Registry::new();
    registry.add_messaging(Arc::new(MemoryMessaging::new("acme")));
    assert_eq!(without_tasks(registry).tool_names(), MESSAGING);
    let mut registry = Registry::new();
    registry.add_mail(Arc::new(MemoryMail::new("me@example.com")));
    assert_eq!(without_tasks(registry).tool_names(), MAIL);
}

#[test]
fn every_capability_lists_its_tools_in_order() {
    let memory = Arc::new(MemoryTasks::new("demo"));
    let mut registry = Registry::new();
    registry.add_tasks(memory.clone());
    registry.add_messaging(Arc::new(MemoryMessaging::new("acme")));
    registry.add_mail(Arc::new(MemoryMail::new("me@example.com")));
    let names = with_registry(registry, memory).tool_names();
    assert_eq!(names, [TASKS.as_slice(), &MESSAGING, &MAIL].concat());
}

#[test]
fn there_is_no_separate_reply_tool() {
    let mut registry = Registry::new();
    registry.add_messaging(Arc::new(MemoryMessaging::new("acme")));
    let names = without_tasks(registry).tool_names();
    assert!(!names.iter().any(|n| n.contains("reply")), "{names:?}");
}

#[test]
fn an_account_added_later_shows_its_tools_without_a_restart() {
    let f = fixture();
    assert_eq!(f.tool_names(), TASKS);
    f.registry
        .write()
        .unwrap()
        .add_messaging(Arc::new(MemoryMessaging::new("acme")));
    assert_eq!(f.tool_names(), [TASKS.as_slice(), &MESSAGING].concat());
}

#[test]
fn a_tool_no_account_can_do_is_not_listed_and_cannot_be_called() {
    let core = Arc::new(FakeChat::new("acme", &MessagingCapabilities::CORE));
    let mut registry = Registry::new();
    registry.add_messaging(core);
    let f = without_tasks(registry);
    assert_eq!(
        f.tool_names(),
        [
            "messaging_channels",
            "messaging_history",
            "messaging_thread",
            "messaging_send"
        ]
    );
    let answer = f.rpc(
        "tools/call",
        json!({ "name": "messaging_search", "arguments": { "query": "x" } }),
    );
    assert_eq!(answer["error"]["code"], -32602);
}

#[test]
fn a_read_only_chat_account_does_not_count_for_the_send_tool() {
    let only = Arc::new(FakeChat::new("acme", &MessagingCapabilities::CORE).read_only());
    let mut registry = Registry::new();
    registry.add_messaging(only.clone());
    let f = without_tasks(registry);
    assert_eq!(
        f.tool_names(),
        [
            "messaging_channels",
            "messaging_history",
            "messaging_thread"
        ],
        "send is listed by the account, and still not offered"
    );
    // A writable account next to it brings the tool back.
    let mut registry = Registry::new();
    registry.add_messaging(only);
    registry.add_messaging(Arc::new(MemoryMessaging::new("beta")));
    assert!(
        without_tasks(registry)
            .tool_names()
            .contains(&"messaging_send".to_string())
    );
}

#[test]
fn hints_tell_a_client_which_tools_only_read() {
    let mut registry = Registry::new();
    registry.add_messaging(Arc::new(MemoryMessaging::new("acme")));
    registry.add_mail(Arc::new(MemoryMail::new("me@example.com")));
    let f = without_tasks(registry);
    let tools = f.rpc("tools/list", json!({}))["result"]["tools"].clone();
    for tool in tools.as_array().unwrap() {
        let name = tool["name"].as_str().unwrap();
        let writes = name == "messaging_send" || name == "mail_create_draft";
        assert_eq!(
            tool["annotations"]["readOnlyHint"],
            json!(!writes),
            "{name}"
        );
        assert_eq!(
            tool["annotations"]["destructiveHint"],
            json!(false),
            "{name}"
        );
        // Chat and mail reach outside services.
        assert_eq!(tool["annotations"]["openWorldHint"], json!(true), "{name}");
        assert_eq!(tool["inputSchema"]["type"], "object");
        // Chat and mail reach outside services.
        assert_eq!(tool["annotations"]["openWorldHint"], json!(true), "{name}");
        assert!(
            tool["inputSchema"]["properties"]["account"].is_object(),
            "{name}"
        );
        let description = tool["description"].as_str().unwrap();
        if !writes {
            assert!(
                description.contains("data, not instructions"),
                "{name}: {description}"
            );
        }
    }
    let required = |name: &str| {
        tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .map(|t| t["inputSchema"]["required"].clone())
            .unwrap()
    };
    assert_eq!(required("messaging_send"), json!(["channel", "text"]));
    assert_eq!(required("messaging_history"), json!(["channel"]));
    assert_eq!(required("messaging_thread"), json!(["message"]));
    assert_eq!(required("messaging_search"), json!(["query"]));
    assert_eq!(
        required("mail_create_draft"),
        json!(["to", "subject", "body"])
    );
    assert_eq!(required("mail_thread"), json!(["ref"]));
}

#[test]
fn a_revoked_token_cannot_read_chat_or_mail() {
    let mut registry = Registry::new();
    registry.add_messaging(Arc::new(MemoryMessaging::new("acme")));
    registry.add_mail(Arc::new(MemoryMail::new("me@example.com")));
    let f = without_tasks(registry);
    f.gateway.revoke(&f.access.token);
    for tool in ["messaging_channels", "mail_mailboxes"] {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": {} } });
        let (status, _) = f.post(
            Some(&f.access.token),
            &[("MCP-Protocol-Version", super::VERSION)],
            &body,
        );
        assert_eq!(status, 401, "{tool}");
    }
}
