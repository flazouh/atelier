use atelier_capabilities::{
    Actor, ActorKind, CapError, CapResult, Ref,
    tasks::{self as v1, ActivityKind, Category},
};
use atelier_ui::{
    PrChipData,
    pr::PrState,
    session_status::SessionStatus,
    task_edit::Change,
    task_model::{Activity, Assignee, Label, Priority, SessionLink, TaskData, TaskStatus},
};
use gpui_kit::SharedString;
use atelier_tracker as tracker;

use super::super::source::Vocabulary;
use super::types::Looks;

/// The prefix of an agent's actor id in the local provider: a person's id is the name. A provider that lists its
/// own actors will replace this rule.
const AGENT_PREFIX: &str = "agent:";

pub fn status_of(category: Category) -> TaskStatus {
    match category {
        Category::Backlog => TaskStatus::Backlog,
        Category::Todo => TaskStatus::Todo,
        Category::InProgress => TaskStatus::InProgress,
        Category::InReview => TaskStatus::InReview,
        Category::Done => TaskStatus::Done,
        Category::Canceled => TaskStatus::Canceled,
    }
}

pub fn status_to(status: TaskStatus) -> Category {
    match status {
        TaskStatus::Backlog => Category::Backlog,
        TaskStatus::Todo => Category::Todo,
        TaskStatus::InProgress => Category::InProgress,
        TaskStatus::InReview => Category::InReview,
        TaskStatus::Done => Category::Done,
        TaskStatus::Canceled => Category::Canceled,
    }
}

pub fn priority_of(priority: v1::Priority) -> Priority {
    match priority {
        v1::Priority::None => Priority::None,
        v1::Priority::Urgent => Priority::Urgent,
        v1::Priority::High => Priority::High,
        v1::Priority::Medium => Priority::Medium,
        v1::Priority::Low => Priority::Low,
    }
}

pub fn priority_to(priority: Priority) -> v1::Priority {
    match priority {
        Priority::None => v1::Priority::None,
        Priority::Urgent => v1::Priority::Urgent,
        Priority::High => v1::Priority::High,
        Priority::Medium => v1::Priority::Medium,
        Priority::Low => v1::Priority::Low,
    }
}

/// A label's tone from its name, so it wears the same tone on every machine and every run.
pub fn label_of(name: &str) -> Label {
    let tone = name.bytes().fold(0u8, |sum, byte| sum.wrapping_add(byte)) % 8;
    Label::new(name.to_string(), tone)
}

pub fn assignee_of(actor: &Actor, looks: Looks) -> Assignee {
    match actor.kind {
        ActorKind::Person => Assignee::Person { name: actor.name.clone().into() },
        ActorKind::Agent => Assignee::agent(actor.name.clone(), looks(&actor.name)),
    }
}

/// The id of the actor an assignee stands for, as the local provider names it.
pub fn actor_id_of(assignee: &Assignee) -> String {
    match assignee {
        Assignee::Person { name } => name.to_string(),
        Assignee::Agent { name, .. } => agent_id(name),
    }
}

/// The actor id of the agent named `name`.
pub fn agent_id(name: &str) -> String {
    format!("{AGENT_PREFIX}{name}")
}

/// Seconds, as atelier-ui counts them, from the milliseconds of v1.
fn seconds(ms: i64) -> u64 {
    (ms / 1000).max(0) as u64
}

fn pr_of(link: &tracker::PrLink) -> PrChipData {
    PrChipData {
        number: link.number,
        repo: link.repo.clone().into(),
        title: SharedString::default(),
        state: PrState::Open,
        url: format!("https://github.com/{}/pull/{}", link.repo, link.number).into(),
        facts: None,
    }
}

fn text_of(detail: &Option<serde_json::Value>, field: &str) -> SharedString {
    detail.as_ref().and_then(|d| d.get(field)).and_then(|v| v.as_str()).unwrap_or_default().to_string().into()
}

fn category_in(detail: &Option<serde_json::Value>, field: &str) -> Option<TaskStatus> {
    let word = detail.as_ref()?.get(field)?.clone();
    serde_json::from_value::<Category>(word).ok().map(status_of)
}

fn number_in(detail: &Option<serde_json::Value>) -> Option<u64> {
    detail.as_ref()?.get("number")?.as_u64()
}

fn activity_of(line: &v1::Activity) -> Option<Activity> {
    let at = seconds(line.at);
    let by: SharedString = line.by.name.clone().into();
    let detail = &line.detail;
    Some(match line.kind {
        ActivityKind::Created => Activity::Created { by, at },
        ActivityKind::StatusChanged => {
            Activity::StatusChanged { by, from: category_in(detail, "from")?, to: category_in(detail, "to")?, at }
        }
        ActivityKind::Commented => Activity::Comment { author: by, text: text_of(detail, "text"), at },
        ActivityKind::SessionStarted => Activity::SessionStarted { agent: text_of(detail, "agent"), at },
        ActivityKind::PrOpened => Activity::PrOpened { number: number_in(detail)?, at },
        ActivityKind::PrMerged => Activity::PrMerged { number: number_in(detail)?, at },
        ActivityKind::Commit => Activity::Committed {
            by,
            sha: text_of(detail, "sha").chars().take(7).collect::<String>().into(),
            subject: text_of(detail, "subject"),
            at,
        },
        // Assignments and other edits have no line in the task view yet.
        ActivityKind::Assigned | ActivityKind::Edited => return None,
    })
}

/// The log of a task as atelier-ui shows it.
pub fn activity_data(lines: &[v1::Activity]) -> Vec<Activity> {
    lines.iter().filter_map(activity_of).collect()
}

/// The session and pull request links of a task. They live in the local tracker, which the local provider keeps
/// in `raw`; a task of another provider has none here until the links move to the capability.
fn local_links(task: &v1::Task, looks: Looks) -> (Vec<SessionLink>, Vec<PrChipData>) {
    let Some(local) = task.raw.as_ref().and_then(|raw| serde_json::from_value::<tracker::Task>(raw.clone()).ok()) else {
        return (Vec::new(), Vec::new());
    };
    let sessions = local
        .sessions
        .iter()
        .map(|s| SessionLink {
            id: s.session_id.clone().into(),
            title: s.title.clone().into(),
            status: SessionStatus::Idle,
            look: looks(&s.agent),
        })
        .collect();
    (sessions, local.prs.iter().map(pr_of).collect())
}

/// A task as atelier-ui shows it. `activity` is the task's log, empty for a list that does not need it. Its id is
/// the task's reference, so it is the same in every provider and every part.
pub fn task_data(task: &v1::Task, activity: &[v1::Activity], vocab: &Vocabulary, looks: Looks) -> TaskData {
    let mut data = TaskData::new(task.reference.to_string(), task.key.clone(), task.title.clone(), status_of(task.status.category));
    data.description = task.description.clone().into();
    data.priority = priority_of(task.priority);
    data.assignee = task.assignees.first().map(|a| assignee_of(a, looks));
    data.labels = task.labels.iter().map(|l| label_of(&vocab.name_of(l))).collect();
    data.project = task.project.as_ref().map(|p| vocab.name_of(p));
    data.parent = task.parent.as_ref().map(|p| p.to_string().into());
    (data.sessions, data.prs) = local_links(task, looks);
    data.activity = activity_data(activity);
    data.created_at = seconds(task.created_at);
    data.updated_at = seconds(task.updated_at);
    data
}

/// The patch a change makes to one task. A label toggles as `atelier_ui::task_edit::apply` does it: it comes
/// off when every task named has it, else it goes on. `all_have_label` says whether every task has it; `labels` are
/// the names the task carries now.
pub fn patch_of(change: &Change, labels: &[Label], all_have_label: bool, vocab: &Vocabulary) -> v1::Patch {
    let mut patch = v1::Patch::default();
    match change {
        Change::Status(status) => patch.status = vocab.status_id(*status),
        Change::Priority(priority) => patch.priority = Some(priority_to(*priority)),
        Change::Assignee(who) => patch.assignees = Some(who.iter().map(actor_id_of).collect()),
        Change::ToggleLabel(label) => {
            let mut names: Vec<&SharedString> = labels.iter().map(|l| &l.name).filter(|name| **name != label.name).collect();
            if !all_have_label {
                names.push(&label.name);
            }
            patch.labels = Some(names.into_iter().map(|name| vocab.label_ref(name)).collect());
        }
    }
    patch
}

/// One patch for each task the change names, with the reference it is for. A row whose id is not a reference
/// is left out: it is not a task of any provider.
pub fn patches_of(tasks: &[TaskData], ids: &[SharedString], change: &Change, vocab: &Vocabulary) -> Vec<(Ref, v1::Patch)> {
    let named = || tasks.iter().filter(|t| ids.contains(&t.id));
    let all_have = match change {
        Change::ToggleLabel(label) => named().all(|t| t.labels.contains(label)),
        _ => false,
    };
    named().filter_map(|t| Some((t.id.parse::<Ref>().ok()?, patch_of(change, &t.labels, all_have, vocab)))).collect()
}

/// A task to make from what the create dialog collected. A status the provider lacks is refused.
pub fn new_task_of(draft: &atelier_ui::new_task_model::Draft, vocab: &Vocabulary) -> CapResult<v1::NewTask> {
    let status = vocab.status_id(draft.status).ok_or_else(|| CapError::unsupported(format!("make a task {}", draft.status.words())))?;
    Ok(v1::NewTask {
        title: draft.title.trim().to_string(),
        description: draft.description.clone(),
        status: Some(status),
        priority: priority_to(draft.priority),
        project: draft.project.as_ref().map(|p| vocab.project_ref(p)),
        labels: draft.labels.iter().map(|l| vocab.label_ref(&l.name)).collect(),
        assignees: draft.assignee.iter().map(actor_id_of).collect(),
        parent: draft.parent.as_ref().and_then(|p| p.parse().ok()),
    })
}

/// The first message of a session started from `task`: the key and title, the description, and the
/// name of the task, so the agent can say which task it works on.
pub fn first_message(task: &v1::Task) -> String {
    let mut text = format!("{}: {}", task.key, task.title);
    if !task.description.trim().is_empty() {
        text.push_str("\n\n");
        text.push_str(task.description.trim());
    }
    text.push_str(&format!("\n\nWork on this task. The task is {}.", task.key));
    text
}

/// The local tracker's id of the task with `key`, for the rules and links, which are local-only for now.
pub fn local_id(tracker: &dyn tracker::Tracker, key: &str) -> Option<tracker::TaskId> {
    let query = tracker::Query { text: Some(key.to_string()), ..tracker::Query::default() };
    tracker.list(&query).ok()?.into_iter().find(|t| t.key.eq_ignore_ascii_case(key)).map(|t| t.id)
}
