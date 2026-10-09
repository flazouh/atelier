//! Results as the agent gets them: each tool of the gateway run over the memory providers, so the cards are tried on the shape
//! the gateway really gives and not on one written by hand.
use std::sync::{Arc, RwLock};

use atelier_capabilities::{
    Actor, Registry,
    mail::{Incoming, MemoryMail},
    messaging::{ChannelKind, MemoryMessaging, MessagingProvider, NewMessage},
    tasks::{MemoryTasks, NewTask, Priority, TasksProvider},
};
use atelier_gateway::{MailTools, MessagingTools, TasksTools, ToolSet};
use serde_json::{Value, json};

/// One call and what came back: the arguments, and the text Claude Code hands the agent, which is the JSON of the result.
pub(super) struct Done {
    pub tool: &'static str,
    pub args: Value,
    pub text: String,
    pub value: Value,
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    let mut at = value;
    for step in path {
        at = match step.parse::<usize>() {
            Ok(n) => &at[n],
            Err(_) => &at[*step],
        };
    }
    at.as_str()
        .unwrap_or_else(|| panic!("no text at {path:?} in {value}"))
}

/// Every tool with a card, called once, in an order in which the later calls can name what the earlier ones gave.
pub(super) fn all() -> Vec<Done> {
    let me = Actor::person("alex", "Alex");
    let agent = Actor::agent("claude-code", "Claude Code", "alex");
    let memory = Arc::new(MemoryTasks::new("smoke"));
    for (title, priority) in [
        ("Write the release notes", Priority::None),
        ("Fix the login on Safari", Priority::High),
    ] {
        memory
            .create(
                &NewTask {
                    priority,
                    ..NewTask::titled(title)
                },
                &me,
            )
            .expect("a seeded task");
    }
    let chat = Arc::new(MemoryMessaging::new("smoke"));
    let general = chat.add_channel("general", ChannelKind::Public);
    for (who, text) in [
        ("Sam", "The deploy is at noon. Ping me if the build fails."),
        ("Ana", "The build is green on main."),
    ] {
        chat.send(
            &NewMessage::to(&general, text),
            &Actor::person(who.to_lowercase(), who),
        )
        .expect("a seeded message");
    }
    let post = Arc::new(MemoryMail::new("me@example.com"));
    let first = post
        .receive(&Incoming::new(
            "ana@example.com",
            "me@example.com",
            "Q3 plan",
            "Here is the Q3 plan. Can you review the budget section by Friday?",
            1_760_000_000_000,
        ))
        .expect("a seeded mail");
    post.receive(&Incoming {
        thread: Some(first.thread),
        ..Incoming::new(
            "sam@example.com",
            "me@example.com",
            "Re: Q3 plan",
            "The budget looks fine to me.",
            1_760_000_100_000,
        )
    })
    .expect("a seeded mail");
    let mut registry = Registry::new();
    registry.add_tasks(memory);
    registry.add_messaging(chat);
    registry.add_mail(post);
    let registry = Arc::new(RwLock::new(registry));
    let sets: Vec<Arc<dyn ToolSet>> = vec![
        Arc::new(TasksTools::new(registry.clone())),
        Arc::new(MessagingTools::new(registry.clone())),
        Arc::new(MailTools::new(registry)),
    ];
    let mut done: Vec<Done> = Vec::new();
    let mut call = |tool: &'static str, args: Value| -> Value {
        let result = sets
            .iter()
            .find_map(|set| set.call(tool, &args, &agent))
            .unwrap_or_else(|| panic!("no tool {tool}"));
        assert!(!result.is_error, "{tool} failed: {}", result.text);
        let value = result.structured.expect("a result with data");
        done.push(Done {
            tool,
            args,
            text: value.to_string(),
            value: value.clone(),
        });
        value
    };

    let listed = call("tasks_list", json!({}));
    let task = at(&listed, &["items", "0", "ref"]).to_string();
    call("tasks_get", json!({ "ref": task }));
    call("tasks_search", json!({ "query": "login" }));
    call(
        "tasks_create",
        json!({ "title": "Check the card", "priority": "high" }),
    );
    call("tasks_update", json!({ "ref": task, "priority": "urgent" }));
    call(
        "tasks_comment",
        json!({ "ref": task, "body": "Started on it." }),
    );

    let channels = call("messaging_channels", json!({}));
    let channel = at(&channels, &["items", "0", "ref"]).to_string();
    let history = call("messaging_history", json!({ "channel": channel }));
    let message = at(&history, &["items", "0", "ref"]).to_string();
    call("messaging_thread", json!({ "message": message }));
    call("messaging_search", json!({ "query": "deploy" }));
    call(
        "messaging_send",
        json!({ "channel": channel, "text": "Thanks, I will look." }),
    );

    call("mail_mailboxes", json!({}));
    let found = call("mail_search", json!({ "query": "plan" }));
    let thread = at(&found, &["items", "0", "ref"]).to_string();
    let read = call("mail_thread", json!({ "ref": thread }));
    let mail = at(&read, &["thread", "messages", "0", "ref"]).to_string();
    call("mail_get", json!({ "ref": mail }));
    call(
        "mail_create_draft",
        json!({ "to": ["ana@example.com"], "subject": "Re: Q3 plan", "body": "I will review it today.", "in_reply_to": mail }),
    );
    done
}
