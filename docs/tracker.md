# The task back end
`crates/tracker` (`atelier-tracker`). It keeps the tasks that the task parts in atelier-ui show. It has no UI type
and no network code. The app maps a `Task` to atelier-ui's `TaskData`, and an agent's name to its `AgentLook`.
## The pieces
- **`Tracker`** is the neutral interface. Every call blocks, so none is made on the UI thread (the same
  rule as `Forge`). The app talks to this and to nothing else.
- **`LocalTracker`** is the implementation that is built: one SQLite database per project.
- **`RuleSet` and `handle`** are the automation. They are pure over the trait, so a Linear or GitHub tracker
  gets them for free.
## The data
`Task { id, key, title, description, status, priority, assignee, labels, project, parent, sessions, prs, created_at, updated_at }`.
- `id` is opaque (`TaskId`). `key` is the short id people say: "LAT-42".
- `Status`: Backlog, Todo, In Progress, In Review, Done, Canceled. `Priority`: None, Urgent, High, Medium,
  Low, numbered 0 to 4 as Linear numbers them.
- `Assignee` is `Person(name)` or `Agent(name)`. The name is the id.
- `parent` makes a sub-task. A task cannot become its own ancestor.
- `sessions` and `prs` are links: `SessionLink { session_id, title, agent }`, `PrLink { number, repo }`.
- Times are seconds since the Unix epoch.
### The calls
`list(Query)` (most recently changed first), `get`, `create`, `create_many` (one transaction), `update(Patch)`,
`record(Entry)`, `activity`, `tasks_of_session`, `tasks_of_pr`, `labels`, `subscribe`.
- `Query` sets any of: statuses, assignee, label, priority, text (title or key, any case), parent. Every
  part that is set must match.
- `Patch` leaves a field alone when it is `None`; `Some(None)` takes an assignee, a project or a parent off.
  A patch that changes nothing writes nothing, logs nothing, and does not move `updated_at`.
- `Entry` is what `record` appends: a comment, a session started, a pull request opened, a pull request
  merged. The last three also become links of the task, once each.
- `subscribe` gives a `Subscription` that reads as a channel of `Event`: `Created(Task)`, `Updated(Task)`,
  `Activity(Activity)`. Drop it to stop: a backend that polls a service checks its `StopFlag` at each tick.
### The activity log
Append-only. Each line has an id, a time, who did it (`by`), and a kind:
created, status changed (from and to), assigned (to whom, or nobody), edited (which field), commented,
session started, PR opened, PR merged. `by` is a person's name, an agent's name, or `rule:<id>` when the
automation did it. Nothing edits or deletes a line.
## The local tracker
- **Where.** In the project data folder, `<data>/atelier/projects/<folder>-<hash>/tracker.sqlite`, never in the
  repository. `Project::tracker()` opens it: `LocalProject` on this machine, and `RemoteTracker` for a project
  over SSH, which asks atelier-remote to open the host own file. So every machine that opens the project sees the
  same tasks. `ProjectKey` and `path_in` are the older way (a file per project in the app data folder) and
  nothing uses them now.
- **Short ids.** The prefix is the first three letters or digits of the project's name, in capitals ("atelier"
  gives LAT). The database keeps the prefix it was made with, so renaming a project keeps its ids. The number
  is one more than the highest so far.
- **Schema.** `PRAGMA user_version` counts migration steps. A step is never edited once released. A
  database from a newer build is refused with "update atelier" instead of being misread.
- **Speed.** WAL mode with `synchronous = NORMAL`: a write is one small append and does not wait for the
  disk to settle. A power cut can lose the last few writes; it cannot corrupt the file. Labels come in the
  task row, so a load is three queries. One connection behind a lock.
- **Threads.** The type is `Send + Sync`. Call it from a background task.
## The rules
Data, not code: `Rule` has an id (kept in the settings), a sentence for the settings screen, and a switch
in a `RuleSet`. All are on until a team turns one off (`RuleSet::from_disabled(ids)`, `disabled()`).
| Rule (id) | When | Moves |
| --- | --- | --- |
| session-start | A session is started from a task | Backlog or Todo to In Progress |
| agent-finish | The session ends well | In Progress to In Review |
| merge | The pull request is merged | Backlog, Todo, In Progress or In Review to Done |
A failed session moves nothing. Opening a pull request moves nothing. A closed task (Done, Canceled) is
never moved, and a task already past a step stays where it is. `RuleSet::decide(status, signal)` is pure.
`handle(tracker, rules, signal)` is the whole step: it logs the event on the tasks it concerns (a session
started, a PR opened, a PR merged), then asks the rules and updates the status with `by = rule:<id>`. The
signals are `SessionStarted { task, session }`, `SessionFinished { session_id, ok }`,
`PrOpened { task, pr, by }` and `PrMerged { number, by }`. A finished session and a merged pull request find
their tasks by the links (`tasks_of_session`, `tasks_of_pr`), so the app does not need to know the task.
The app builds the signals from the neutral session events (`atelier-agents`) and the forge (`atelier-forge`).
A merge moves a task at once even when it has a second open pull request. A stricter rule ("every linked
pull request is merged") is a new `Rule` and needs the link to remember its state.
## Numbers
Release, on the HP, 10,000 tasks on a file (2 labels each, some assignees), 20 runs:
| Case | Target | Median | p95 |
| --- | --- | --- | --- |
| Load all 10,000 | under 50 ms | 21 ms | 33 ms |
| Filter (label, priority and text) | under 50 ms | 6.4 ms | 11 ms |
| One status write | under 5 ms | 0.045 ms | 0.07 ms (max 1.4 ms) |
| Import of 10,000 in one transaction | | 260 ms | |
Run it: `cargo test --release -p atelier-tracker -- --ignored --nocapture`.
## Other backends (not built)
A backend implements `Tracker` and returns `TrackerError::Unsupported` for what its service has no place
for. `subscribe` carries what the service pushes (a webhook, a poll). The rules and the views do not change.
### Linear
Needs an API key from Alex. Not started.
- **Ids.** `id` is the issue UUID. `key` is the identifier (`LAT-42`), which Linear already numbers per team.
  `create` posts `issueCreate`, so the key comes from Linear, not from us.
- **Status.** Linear workflow states have a `type`: backlog, unstarted, started, completed, canceled.
  Backlog, Todo, Done and Canceled map by type. Both In Progress and In Review are `started`; map by state
  name, and let a team choose the two states in the settings. A team with one started state has no In Review.
- **Priority.** The numbers already match: 0 none, 1 urgent, 2 high, 3 medium, 4 low.
- **Assignee.** A Linear user is a `Person`. An agent is a Linear "app user" or a delegate; if the workspace
  has none, an agent assignee is `Unsupported` and lives in a label (`agent:claude`).
- **Labels, parent, project.** Labels map one to one. `parent` is `issue.parent`. `project` is `issue.project`.
- **Activity.** Comments are comments. Status, assignee and other edits come from `IssueHistory`. Session
  and pull request lines are comments or attachments that atelier writes (`attachmentCreate` with the pull
  request URL, which Linear also shows on the issue).
- **Links.** A pull request is an attachment. `tasks_of_pr(n)` searches attachments by URL. A session is a
  atelier attachment with a `atelier://session/<id>` URL.
- **Events.** A webhook on Issue and Comment, or a poll of `issues(filter: {updatedAt: {gt: ...}})`.
- **Rate limits.** Cache the list; `list` answers from the cache and refreshes in the background.
### GitHub Issues
Uses the token atelier already holds for the forge.
- **Ids.** `id` is the issue node id. `key` is `#42`; there is no prefix. The short id is the issue number.
- **Status.** An issue is open or closed. `closed` with `state_reason` `completed` is Done, `not_planned` is
  Canceled. Backlog, Todo, In Progress and In Review need more: a Projects (v2) single-select "Status"
  field is the closest match, or four `status:*` labels when the repository has no project. The
  settings choose which.
- **Priority.** A Projects field, or `priority:*` labels. Otherwise None.
- **Assignee.** GitHub assignees are users, so `Person`. An agent is a Copilot-style bot user, or a
  `agent:<name>` label.
- **Labels.** Map one to one, minus the ones used for status and priority.
- **Parent.** Sub-issues (`parent_issue`), where the repository has them.
- **Activity.** The issue timeline: `commented`, `assigned`, `labeled`, `closed`, `cross-referenced` (a pull
  request that names the issue), `referenced`. Session lines are comments that atelier writes, with a marker.
- **Links.** A pull request that closes the issue (`Closes #42`) is in the timeline. `tasks_of_pr(n)` reads
  the pull request's `closingIssuesReferences`.
- **Events.** A webhook or ETag polling of `/issues?since=`.
- **Limits.** No native Backlog and no priorities: the status and priority fields are the settings' problem.
  Sub-issue and Projects APIs differ on GitHub Enterprise Server.
## What the app still does
Choose the data folder, open a `LocalTracker` for each project on a background thread, map `Task` to
`TaskData`, save the changes the task parts report (`TaskListEvent::Changed`), turn agent and forge events
into `Signal`s and call `handle`, and keep the `RuleSet` in the settings.
