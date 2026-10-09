use std::sync::Arc;

use atelier_capabilities::{
    Actor, CapError, Ref, Registry,
    tasks::{NewTask, Patch, Query, TasksProvider},
};
use serde_json::{Value, json};

use super::render::{NOTICE, comment_detail, page, task_detail, task_line, untrusted};
use crate::{
    ToolResult,
    shared::{account, neutral, pick, required},
};

/// How many entries of a task's activity `tasks_get` shows: the latest.
const ACTIVITY_SHOWN: usize = 20;

/// A tool body gives its result, or the sentence the model reads when the call failed.
type Done = Result<ToolResult, String>;

pub(super) fn list(registry: &Registry, args: &Value) -> Done {
    let provider = provider_for(registry, account(args)?, None)?;
    let query: Query = pick(
        args,
        &[
            "status", "assignee", "project", "labels", "text", "sort", "limit", "cursor",
        ],
        &[],
    )?;
    let found = provider.list(&query).map_err(failed("list the tasks"))?;
    let (text, data) = page(&found, &place(provider.as_ref()));
    Ok(ToolResult::ok(text, data))
}

pub(super) fn search(registry: &Registry, args: &Value) -> Done {
    let provider = provider_for(registry, account(args)?, None)?;
    let words = required(args, "query")?;
    let query: Query = pick(
        args,
        &["status", "limit", "cursor"],
        &[("text", json!(words))],
    )?;
    let found = provider.list(&query).map_err(failed("search the tasks"))?;
    let (text, data) = page(&found, &place(provider.as_ref()));
    Ok(ToolResult::ok(text, data))
}

pub(super) fn get(registry: &Registry, args: &Value) -> Done {
    let (provider, reference) = task_of(registry, args)?;
    let task = provider.get(&reference).map_err(failed("read the task"))?;
    // The activity is a bonus. A provider without it, or one that fails here, still gives the task.
    let mut activity = provider
        .activity(&reference, None)
        .map(|page| page.items)
        .unwrap_or_default();
    let skip = activity.len().saturating_sub(ACTIVITY_SHOWN);
    activity.drain(..skip);
    let text = format!(
        "Task {}, version {}.\n{}",
        task.reference,
        task.version,
        untrusted(&task_detail(&task, &activity))
    );
    let data = json!({
        "task": neutral(&task),
        "activity": activity.iter().map(neutral).collect::<Vec<_>>(),
        "notice": NOTICE,
    });
    Ok(ToolResult::ok(text, data))
}

pub(super) fn create(registry: &Registry, args: &Value, by: &Actor) -> Done {
    let provider = provider_for(registry, account(args)?, None)?;
    let new: NewTask = pick(
        args,
        &[
            "title",
            "description",
            "status",
            "priority",
            "project",
            "labels",
            "assignees",
            "parent",
        ],
        &[],
    )?;
    let task = provider
        .create(&new, by)
        .map_err(failed("create the task"))?;
    let text = format!(
        "Created {}.\n{}",
        task.reference,
        untrusted(&task_line(&task))
    );
    Ok(ToolResult::ok(
        text,
        json!({ "task": neutral(&task), "notice": NOTICE }),
    ))
}

pub(super) fn update(registry: &Registry, args: &Value, by: &Actor) -> Done {
    let (provider, reference) = task_of(registry, args)?;
    let patch: Patch = pick(
        args,
        &[
            "title",
            "description",
            "status",
            "priority",
            "project",
            "labels",
            "assignees",
            "parent",
        ],
        &[],
    )?;
    if patch == Patch::default() {
        return Err("Send at least one field to change, such as title, status or priority.".into());
    }
    let version = match args["version"].as_str().filter(|v| !v.is_empty()) {
        Some(version) => version.to_string(),
        // The agent chose not to say which version it saw. Its change then goes on top of what is there now.
        None => {
            provider
                .get(&reference)
                .map_err(failed("read the task"))?
                .version
        }
    };
    let task = provider
        .update(&reference, &patch, &version, by)
        .map_err(failed("update the task"))?;
    let text = format!(
        "Updated {}, now at version {}.\n{}",
        task.reference,
        task.version,
        untrusted(&task_line(&task))
    );
    Ok(ToolResult::ok(
        text,
        json!({ "task": neutral(&task), "notice": NOTICE }),
    ))
}

pub(super) fn comment(registry: &Registry, args: &Value, by: &Actor) -> Done {
    let (provider, reference) = task_of(registry, args)?;
    let body = required(args, "body")?;
    let made = provider
        .comment(&reference, &body, by)
        .map_err(failed("comment on the task"))?;
    let text = format!(
        "Added a comment to {reference}.\n{}",
        untrusted(&comment_detail(&made))
    );
    Ok(ToolResult::ok(
        text,
        json!({ "comment": neutral(&made), "notice": NOTICE }),
    ))
}

fn place(provider: &dyn TasksProvider) -> String {
    format!("{}/{}", provider.provider(), provider.account())
}

/// The provider a call goes to: the account when it names one, else the one the ref belongs to, else the only one
/// there is. With several and no hint, the sentence lists them, so the model can choose.
fn provider_for(
    registry: &Registry,
    named: Option<(String, String)>,
    reference: Option<&Ref>,
) -> Result<Arc<dyn TasksProvider>, String> {
    let from_ref = reference.map(|r| (r.provider.clone(), r.account.clone()));
    if let (Some(named), Some(from_ref)) = (&named, &from_ref)
        && named != from_ref
    {
        return Err(format!(
            "The ref belongs to {}/{} but the account says {}/{}. Leave the account out, or make them agree.",
            from_ref.0, from_ref.1, named.0, named.1
        ));
    }
    let all = registry.all_tasks();
    let choices = || {
        all.iter()
            .map(|p| place(p.as_ref()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match named.or(from_ref) {
        Some((provider, account)) => registry.tasks(&provider, &account).ok_or_else(|| {
            format!(
                "No tasks are connected at {provider}/{account}. Choose one of: {}.",
                choices()
            )
        }),
        None => match all.as_slice() {
            [] => Err("No task provider is connected in Atelier.".into()),
            [only] => Ok(only.clone()),
            _ => Err(format!(
                "More than one place keeps tasks. Pass account as one of: {}.",
                choices()
            )),
        },
    }
}

/// The provider and the task a call is about. `ref` is a full reference; a bare id such as `LAT-42` works when the
/// account is clear from the call.
fn task_of(registry: &Registry, args: &Value) -> Result<(Arc<dyn TasksProvider>, Ref), String> {
    let text = required(args, "ref")?;
    let named = account(args)?;
    if let Ok(reference) = text.parse::<Ref>() {
        if reference.capability != "tasks" {
            return Err(format!("{text} is not a task reference."));
        }
        let provider = provider_for(registry, named, Some(&reference))?;
        return Ok((provider, reference));
    }
    let provider = provider_for(registry, named, None)?;
    let reference =
        Ref::new("tasks", provider.provider(), provider.account(), &text).map_err(|_| {
            format!("{text} is not a task reference. Use the ref a list or search gave you.")
        })?;
    Ok((provider, reference))
}

/// The sentence for a failed call. A stale version also says what to do about it.
fn failed(what: &'static str) -> impl Fn(CapError) -> String {
    move |error| match error {
        CapError::Conflict { current } => {
            let version = current["version"].as_str().unwrap_or("unknown");
            format!(
                "Could not {what}: the task changed since you read it. Its version is now {version}. Call tasks_get \
                 to read it again, check that your change still fits, and send it with that version."
            )
        }
        other => format!("Could not {what}: {other}."),
    }
}
