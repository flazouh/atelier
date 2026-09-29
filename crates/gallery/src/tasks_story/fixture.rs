//! Tasks for the "Tasks" story: the same list every time, so a screenshot or a run can be compared.
use beui::{
    AgentLook, PrChipData, PrState, SessionStatus,
    task_model::{Activity, Assignee, Label, Priority, SessionLink, TaskData, TaskStatus},
};
use gpui_kit::SharedString;

/// The time the fixture counts from, in seconds since the Unix epoch.
pub const BASE: u64 = 1_790_700_000;
/// Who the reader is.
pub const ME: &str = "Alex";

const VERBS: [&str; 12] = ["Add", "Fix", "Port", "Remove", "Speed up", "Rewrite", "Document", "Test", "Split", "Move", "Rename", "Check"];
const THINGS: [&str; 16] = [
    "the sidebar",
    "the diff layout",
    "the review queue",
    "the forge client",
    "the highlight cache",
    "session restore",
    "the task board",
    "the finder",
    "the theme picker",
    "the SSH reconnect",
    "the PR chip",
    "the composer toolbar",
    "the keyboard table",
    "the settings file",
    "the release build",
    "the markdown parser",
];
const TAILS: [&str; 6] = ["", " on the Mac", " for long lines", " after a resize", " with no network", " in dark mode"];

pub fn labels() -> Vec<Label> {
    ["bug", "ui", "perf", "docs", "infra", "agents", "review"].iter().enumerate().map(|(i, n)| Label::new(*n, i as u8)).collect()
}

pub fn people(claude: &AgentLook, other: &AgentLook) -> Vec<Assignee> {
    vec![
        Assignee::Person { name: ME.into() },
        Assignee::Person { name: "Sam".into() },
        Assignee::Person { name: "Mina".into() },
        Assignee::Person { name: "Ravi".into() },
        Assignee::Agent { name: "Claude".into(), look: claude.clone() },
        Assignee::Agent { name: "Codex".into(), look: other.clone() },
    ]
}

/// A small deterministic generator. Not random: the same seed gives the same run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % below
    }
}

fn pr(number: u64, state: PrState, title: &str) -> PrChipData {
    PrChipData {
        number,
        repo: "flazouh/lathe".into(),
        title: title.to_string().into(),
        state,
        url: format!("https://github.com/flazouh/lathe/pull/{number}").into(),
    }
}

/// `count` tasks, keys LAT-1 up. Every 25th starts a small family of sub-tasks.
pub fn tasks(count: usize, claude: &AgentLook, other: &AgentLook) -> Vec<TaskData> {
    let labels = labels();
    let people = people(claude, other);
    let mut rng = Lcg(7);
    let mut out: Vec<TaskData> = Vec::with_capacity(count);
    for i in 0..count {
        let status = match rng.next(20) {
            0..=4 => TaskStatus::Backlog,
            5..=9 => TaskStatus::Todo,
            10..=12 => TaskStatus::InProgress,
            13..=14 => TaskStatus::InReview,
            15..=18 => TaskStatus::Done,
            _ => TaskStatus::Canceled,
        };
        let title = format!("{} {}{}", VERBS[rng.next(VERBS.len())], THINGS[rng.next(THINGS.len())], TAILS[rng.next(TAILS.len())]);
        let mut task = TaskData::new(format!("t{i}"), format!("LAT-{}", i + 1), title, status);
        task.priority = [Priority::None, Priority::Low, Priority::Medium, Priority::High, Priority::Urgent][rng.next(5)];
        if rng.next(10) < 7 {
            task.assignee = Some(people[rng.next(people.len())].clone());
        }
        for _ in 0..rng.next(3) {
            let label = labels[rng.next(labels.len())].clone();
            if !task.labels.contains(&label) {
                task.labels.push(label);
            }
        }
        task.project = Some("lathe".into());
        let age = (rng.next(90) as u64 + 1) * 3600 * (1 + rng.next(4) as u64);
        task.created_at = BASE - age - 86_400;
        task.updated_at = BASE - age;
        task.activity.push(Activity::Created { by: ME.into(), at: task.created_at });
        if matches!(status, TaskStatus::InProgress | TaskStatus::InReview | TaskStatus::Done) && rng.next(3) == 0 {
            let number = 3000 + i as u64;
            let state = match status {
                TaskStatus::Done => PrState::Merged,
                TaskStatus::InReview => PrState::Open,
                _ => PrState::Draft,
            };
            task.prs.push(pr(number, state, &task.title.to_string()));
            task.activity.push(Activity::PrOpened { number, at: task.updated_at - 600 });
        }
        if status == TaskStatus::InProgress && rng.next(2) == 0 {
            task.sessions.push(SessionLink {
                id: format!("s{i}").into(),
                title: task.title.clone(),
                status: SessionStatus::Working,
                look: claude.clone(),
            });
        }
        if i % 25 > 0 && i % 25 <= 4 {
            task.parent = Some(format!("t{}", i - i % 25).into());
        }
        out.push(task);
    }
    if let Some(first) = out.first_mut() {
        feature(first, claude);
    }
    out
}

/// The first task is the one the "Task" tab opens on, so it has a bit of everything.
fn feature(task: &mut TaskData, claude: &AgentLook) {
    task.title = "Add the task list and the task board".into();
    task.status = TaskStatus::InProgress;
    task.priority = Priority::High;
    task.assignee = Some(Assignee::Agent { name: "Claude".into(), look: claude.clone() });
    task.labels = vec![Label::new("ui", 1), Label::new("agents", 5)];
    task.description = "## Goal\n\nShow the work as **tasks**, the way Linear does.\n\n- a list grouped by status\n- a board with drag between columns\n- a page for one task\n\nSee `docs/tasks.md` for the keys.".into();
    task.sessions = vec![SessionLink { id: "s0".into(), title: "Build the task parts".into(), status: SessionStatus::Working, look: claude.clone() }];
    task.prs = vec![pr(3344, PrState::Open, "Add the task parts")];
    let at = task.updated_at;
    task.activity.extend([
        Activity::StatusChanged { by: ME.into(), from: TaskStatus::Todo, to: TaskStatus::InProgress, at: at - 5400 },
        Activity::SessionStarted { agent: "Claude".into(), at: at - 5000 },
        Activity::Comment { author: ME.into(), text: SharedString::from("Keep the rows at 28 pixels."), at: at - 3000 },
        Activity::PrOpened { number: 3344, at: at - 600 },
    ]);
}
