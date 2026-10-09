use serde_json::{Value, json};

use crate::{Permission, ToolDef};

const CATEGORIES: [&str; 6] = [
    "backlog",
    "todo",
    "in_progress",
    "in_review",
    "done",
    "canceled",
];
const PRIORITIES: [&str; 5] = ["none", "urgent", "high", "medium", "low"];

pub(super) fn definitions() -> Vec<ToolDef> {
    vec![
        def(
            "tasks_list",
            "List tasks",
            "List tasks, most recently changed first. Filter by status, assignee, project or labels. Returns a page; \
             pass next_cursor as cursor to read the next one.",
            Permission::Read,
            list_properties(),
            &[],
        ),
        def(
            "tasks_get",
            "Get a task",
            "Read one task in full, with its recent activity. Use the ref a list or search gave you.",
            Permission::Read,
            with_account(json!({ "ref": ref_property() })),
            &["ref"],
        ),
        def(
            "tasks_create",
            "Create a task",
            "Create a task. The change is recorded as made by you, for the person you work for.",
            Permission::Write,
            with_account(json!({
                "title": { "type": "string", "description": "One line that says what to do." },
                "description": { "type": "string", "description": "Markdown." },
                "status": { "type": "string", "description": "A status id of the provider. Leave out for the first todo or backlog state." },
                "priority": { "type": "string", "enum": PRIORITIES },
                "project": { "type": "string", "description": "A project ref." },
                "labels": { "type": "array", "items": { "type": "string" }, "description": "Label refs." },
                "assignees": { "type": "array", "items": { "type": "string" }, "description": "Actor ids." },
                "parent": { "type": "string", "description": "The ref of the parent task." },
            })),
            &["title"],
        ),
        def(
            "tasks_update",
            "Update a task",
            "Change fields of a task. Send only the fields to change; null clears a field that can be cleared. Send \
             the version from tasks_get, so a change someone made in the meantime is not lost; without it the task \
             is read first and your change goes on top of what is there.",
            Permission::Write,
            with_account(json!({
                "ref": ref_property(),
                "version": { "type": "string", "description": "The version of the task you read." },
                "title": { "type": "string" },
                "description": { "type": ["string", "null"], "description": "Markdown. null clears it." },
                "status": { "type": "string", "description": "A status id of the provider." },
                "priority": { "type": "string", "enum": PRIORITIES },
                "project": { "type": ["string", "null"], "description": "A project ref. null clears it." },
                "labels": { "type": "array", "items": { "type": "string" }, "description": "The whole list of label refs." },
                "assignees": { "type": "array", "items": { "type": "string" }, "description": "The whole list of actor ids. [] clears it." },
                "parent": { "type": ["string", "null"], "description": "The ref of the parent task. null clears it." },
            })),
            &["ref"],
        ),
        def(
            "tasks_comment",
            "Comment on a task",
            "Add a comment to a task. It shows as yours, for the person you work for.",
            Permission::Write,
            with_account(json!({
                "ref": ref_property(),
                "body": { "type": "string", "description": "The comment, in markdown." },
            })),
            &["ref", "body"],
        ),
        def(
            "tasks_search",
            "Search tasks",
            "Find tasks whose title or text matches the query.",
            Permission::Read,
            with_account(json!({
                "query": { "type": "string", "description": "Words to look for." },
                "status": status_property(),
                "limit": limit_property(),
                "cursor": { "type": "string" },
            })),
            &["query"],
        ),
    ]
}

fn def(
    name: &str,
    title: &str,
    description: &str,
    permission: Permission,
    properties: Value,
    required: &[&str],
) -> ToolDef {
    ToolDef {
        name: name.into(),
        title: title.into(),
        description: description.into(),
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        }),
        permission,
    }
}

/// `account` is in every tool: the provider and the account, as `provider/account`.
fn with_account(mut properties: Value) -> Value {
    properties["account"] = json!({
        "type": "string",
        "description": "Where the tasks live, as provider/account, for example local/atelier. Leave out when only one \
                        is connected, or when ref says it already.",
    });
    properties
}

fn ref_property() -> Value {
    json!({
        "type": "string",
        "description": "The task reference, for example tasks:local:atelier:LAT-42.",
    })
}

fn status_property() -> Value {
    json!({
        "type": "array",
        "items": { "type": "string", "enum": CATEGORIES },
        "description": "Only tasks in these status categories.",
    })
}

fn limit_property() -> Value {
    json!({ "type": "integer", "minimum": 1, "description": "Page size." })
}

fn list_properties() -> Value {
    with_account(json!({
        "status": status_property(),
        "assignee": { "type": "string", "description": "An actor id." },
        "project": { "type": "string", "description": "A project ref." },
        "labels": { "type": "array", "items": { "type": "string" }, "description": "Label refs." },
        "text": { "type": "string", "description": "Words in the title or text." },
        "sort": { "type": "string", "enum": ["updated", "created", "priority"] },
        "limit": limit_property(),
        "cursor": { "type": "string", "description": "The next_cursor of the page before." },
    }))
}
