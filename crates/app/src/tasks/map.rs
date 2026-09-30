//! Tasks between the tracker's types and what beui shows. The tracker knows no UI type and beui names no
//! tracker, so this is the one place that knows both.
use beui::{
    AgentLook, PrChipData,
    pr::PrState,
    session_status::SessionStatus,
    task_edit::Change,
    task_model::{Activity, Assignee, Label, Priority, SessionLink, TaskData, TaskStatus},
};
use gpui_kit::SharedString;
use lathe_tracker as tracker;

pub fn status_of(status: tracker::Status) -> TaskStatus {
    match status {
        tracker::Status::Backlog => TaskStatus::Backlog,
        tracker::Status::Todo => TaskStatus::Todo,
        tracker::Status::InProgress => TaskStatus::InProgress,
        tracker::Status::InReview => TaskStatus::InReview,
        tracker::Status::Done => TaskStatus::Done,
        tracker::Status::Canceled => TaskStatus::Canceled,
    }
}

pub fn status_to(status: TaskStatus) -> tracker::Status {
    match status {
        TaskStatus::Backlog => tracker::Status::Backlog,
        TaskStatus::Todo => tracker::Status::Todo,
        TaskStatus::InProgress => tracker::Status::InProgress,
        TaskStatus::InReview => tracker::Status::InReview,
        TaskStatus::Done => tracker::Status::Done,
        TaskStatus::Canceled => tracker::Status::Canceled,
    }
}

pub fn priority_of(priority: tracker::Priority) -> Priority {
    match priority {
        tracker::Priority::None => Priority::None,
        tracker::Priority::Urgent => Priority::Urgent,
        tracker::Priority::High => Priority::High,
        tracker::Priority::Medium => Priority::Medium,
        tracker::Priority::Low => Priority::Low,
    }
}

pub fn priority_to(priority: Priority) -> tracker::Priority {
    match priority {
        Priority::None => tracker::Priority::None,
        Priority::Urgent => tracker::Priority::Urgent,
        Priority::High => tracker::Priority::High,
        Priority::Medium => tracker::Priority::Medium,
        Priority::Low => tracker::Priority::Low,
    }
}

/// A label's tone from its name, so it wears the same tone on every machine and every run.
pub fn label_of(name: &str) -> Label {
    let tone = name.bytes().fold(0u8, |sum, byte| sum.wrapping_add(byte)) % 8;
    Label::new(name.to_string(), tone)
}

/// The look an agent wears, by its name. `neutral` is for an agent lathe has no look for.
pub type Looks<'a> = &'a dyn Fn(&str) -> AgentLook;

pub fn assignee_of(assignee: &tracker::Assignee, looks: Looks) -> Assignee {
    match assignee {
        tracker::Assignee::Person(name) => Assignee::Person { name: name.clone().into() },
        tracker::Assignee::Agent(name) => Assignee::agent(name.clone(), looks(name)),
    }
}

pub fn assignee_to(assignee: &Assignee) -> tracker::Assignee {
    match assignee {
        Assignee::Person { name } => tracker::Assignee::Person(name.to_string()),
        Assignee::Agent { name, .. } => tracker::Assignee::Agent(name.to_string()),
    }
}

fn pr_of(link: &tracker::PrLink) -> PrChipData {
    PrChipData {
        number: link.number,
        repo: link.repo.clone().into(),
        title: SharedString::default(),
        state: PrState::Open,
        url: format!("https://github.com/{}/pull/{}", link.repo, link.number).into(),
    }
}

fn activity_of(line: &tracker::Activity) -> Option<Activity> {
    let at = line.at.max(0) as u64;
    let by: SharedString = line.by.clone().into();
    Some(match &line.kind {
        tracker::ActivityKind::Created => Activity::Created { by, at },
        tracker::ActivityKind::StatusChanged { from, to } => {
            Activity::StatusChanged { by, from: status_of(*from), to: status_of(*to), at }
        }
        tracker::ActivityKind::Commented { text } => Activity::Comment { author: by, text: text.clone().into(), at },
        tracker::ActivityKind::SessionStarted { session } => Activity::SessionStarted { agent: session.agent.clone().into(), at },
        tracker::ActivityKind::PrOpened { pr } => Activity::PrOpened { number: pr.number, at },
        tracker::ActivityKind::PrMerged { pr } => Activity::PrMerged { number: pr.number, at },
        // Assignments and other edits have no line in the task view yet.
        tracker::ActivityKind::Assigned { .. } | tracker::ActivityKind::Edited { .. } => return None,
    })
}

/// The log of a task as beui shows it.
pub fn activity_data(lines: &[tracker::Activity]) -> Vec<Activity> {
    lines.iter().filter_map(activity_of).collect()
}

/// A task as beui shows it. `activity` is the task's log, empty for a list that does not need it.
pub fn task_data(task: &tracker::Task, activity: &[tracker::Activity], looks: Looks) -> TaskData {
    let mut data = TaskData::new(task.id.0.clone(), task.key.clone(), task.title.clone(), status_of(task.status));
    data.description = task.description.clone().into();
    data.priority = priority_of(task.priority);
    data.assignee = task.assignee.as_ref().map(|a| assignee_of(a, looks));
    data.labels = task.labels.iter().map(|l| label_of(l)).collect();
    data.project = task.project.clone().map(Into::into);
    data.parent = task.parent.as_ref().map(|p| p.0.clone().into());
    data.sessions = task
        .sessions
        .iter()
        .map(|s| SessionLink {
            id: s.session_id.clone().into(),
            title: s.title.clone().into(),
            status: SessionStatus::Idle,
            look: looks(&s.agent),
        })
        .collect();
    data.prs = task.prs.iter().map(pr_of).collect();
    data.activity = activity_data(activity);
    data.created_at = task.created_at.max(0) as u64;
    data.updated_at = task.updated_at.max(0) as u64;
    data
}

/// The patch a change makes to one task. A label toggles as `beui::task_edit::apply` does it: it comes
/// off when every task named has it, else it goes on. `all` says whether every task has it.
pub fn patch_of(change: &Change, all_have_label: bool) -> tracker::Patch {
    let mut patch = tracker::Patch::default();
    match change {
        Change::Status(status) => patch.status = Some(status_to(*status)),
        Change::Priority(priority) => patch.priority = Some(priority_to(*priority)),
        Change::Assignee(who) => patch.assignee = Some(who.as_ref().map(assignee_to)),
        Change::ToggleLabel(label) if all_have_label => patch.remove_labels.push(label.name.to_string()),
        Change::ToggleLabel(label) => patch.add_labels.push(label.name.to_string()),
    }
    patch
}

/// One patch for each task the change names, with the id it is for.
pub fn patches_of(tasks: &[TaskData], ids: &[SharedString], change: &Change) -> Vec<(tracker::TaskId, tracker::Patch)> {
    let named = || tasks.iter().filter(|t| ids.contains(&t.id));
    let all_have = match change {
        Change::ToggleLabel(label) => named().all(|t| t.labels.contains(label)),
        _ => false,
    };
    named().map(|t| (tracker::TaskId(t.id.to_string()), patch_of(change, all_have))).collect()
}

/// A task to make from what the create dialog collected.
pub fn new_task_of(draft: &beui::new_task_model::Draft) -> tracker::NewTask {
    tracker::NewTask {
        title: draft.title.trim().to_string(),
        description: draft.description.clone(),
        status: status_to(draft.status),
        priority: priority_to(draft.priority),
        assignee: draft.assignee.as_ref().map(assignee_to),
        labels: draft.labels.iter().map(|l| l.name.to_string()).collect(),
        project: draft.project.as_ref().map(|p| p.to_string()),
        parent: draft.parent.as_ref().map(|p| tracker::TaskId(p.to_string())),
    }
}

/// The first message of a session started from `task`: the key and title, the description, and the
/// name of the task, so the agent can say which task it works on.
pub fn first_message(task: &tracker::Task) -> String {
    let mut text = format!("{}: {}", task.key, task.title);
    if !task.description.trim().is_empty() {
        text.push_str("\n\n");
        text.push_str(task.description.trim());
    }
    text.push_str(&format!("\n\nWork on this task. The task is {}.", task.key));
    text
}
