use std::{
    sync::{Arc, mpsc::channel},
    thread,
    time::Duration,
};

use atelier_capabilities::{
    Actor, AuthKind, CapError, CapResult, Capabilities, Feature, Limits, Operation, Ref,
    Subscription,
    tasks::{
        Activity, Change, Comment, Event, Label, NewTask, Page, Patch, Project, Query, Status,
        Task, TasksProvider,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    helpers,
    types::{self, ENDPOINT, PAGE_DEFAULT, PAGE_MAX},
};
use crate::client::Client;

/// What a [`LinearTasks`] needs. The caller reads the key from wherever it keeps it and hands it over here.
#[derive(Clone)]
pub struct Config {
    pub api_key: String,
    /// Linear's address. Only a test changes it.
    pub endpoint: String,
    /// The key of the team new tasks go to and lists stay in, for example `ENG`. Without it a list covers the
    /// workspace, and `create` works only in a workspace with one team.
    pub team: Option<String>,
    /// How often `subscribe` asks Linear what changed.
    pub poll_every: Duration,
}

impl Config {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            endpoint: ENDPOINT.into(),
            team: None,
            poll_every: types::POLL_EVERY,
        }
    }

    pub fn with_team(mut self, team: impl Into<String>) -> Self {
        self.team = Some(team.into());
        self
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn with_poll_every(mut self, every: Duration) -> Self {
        self.poll_every = every;
        self
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("api_key", &"<hidden>")
            .field("endpoint", &self.endpoint)
            .field("team", &self.team)
            .field("poll_every", &self.poll_every)
            .finish()
    }
}

/// A Linear workspace as a tasks provider.
#[derive(Debug)]
pub struct LinearTasks {
    client: Arc<Client>,
    /// The workspace's url key: the account part of every reference.
    account: String,
    team: Option<String>,
    poll_every: Duration,
}

impl LinearTasks {
    /// Signs in with the key and a default team: reads who the key belongs to and which workspace it opens.
    pub fn connect(api_key: &str) -> CapResult<Self> {
        Self::connect_with(Config::new(api_key))
    }

    pub fn connect_with(config: Config) -> CapResult<Self> {
        let client = Client::new(&config.endpoint, &config.api_key);
        let data = client.run(types::VIEWER, json!({}))?;
        let viewer: Viewer = helpers::read(&data["viewer"], "viewer")?;
        Ok(Self {
            client: Arc::new(client),
            account: viewer.organization.url_key,
            team: config.team,
            poll_every: config.poll_every,
        })
    }

    fn run_issue(&self, query: &str, variables: Value) -> CapResult<Value> {
        self.client
            .run(&helpers::with_issue_fields(query), variables)
    }

    /// The issue as Linear has it, by key or uuid.
    fn read_issue(&self, key: &str) -> CapResult<Value> {
        let data = self.run_issue(types::GET, json!({ "id": key }))?;
        match data.get("issue") {
            Some(issue) if !issue.is_null() => Ok(issue.clone()),
            _ => Err(CapError::not_found(key)),
        }
    }

    fn task_key(&self, task: &Ref) -> CapResult<String> {
        helpers::task_key(task, &self.account)
    }

    fn team_id(&self) -> CapResult<String> {
        let data = self.client.run(types::TEAMS, json!({}))?;
        let teams: Vec<TeamNode> = helpers::read(&data["teams"]["nodes"], "teams")?;
        match (&self.team, teams.as_slice()) {
            (Some(key), _) => teams
                .iter()
                .find(|t| &t.key == key)
                .map(|t| t.id.clone())
                .ok_or_else(|| CapError::invalid("team")),
            (None, [only]) => Ok(only.id.clone()),
            // Several teams and none chosen: guessing would put the task in the wrong team.
            (None, _) => Err(CapError::invalid("team")),
        }
    }

    /// The uuid of a task that a patch or a new task names as a parent.
    fn issue_uuid(&self, task: &Ref) -> CapResult<String> {
        let issue = self.read_issue(&self.task_key(task)?)?;
        Ok(helpers::text(&issue["id"]))
    }
}

impl TasksProvider for LinearTasks {
    fn provider(&self) -> &str {
        "linear"
    }

    fn account(&self) -> &str {
        &self.account
    }

    fn capabilities(&self) -> Capabilities {
        let mut operations = Capabilities::CORE.to_vec();
        operations.push(Operation::Statuses);
        Capabilities {
            operations,
            features: vec![
                Feature::CustomStates,
                Feature::Subtasks,
                Feature::Estimates,
                Feature::DueDates,
            ],
            limits: Limits {
                page_max: Some(PAGE_MAX),
                per_minute: None,
            },
            auth: vec![AuthKind::Token],
        }
    }

    fn whoami(&self) -> CapResult<Actor> {
        let data = self.client.run(types::VIEWER, json!({}))?;
        let viewer: Viewer = helpers::read(&data["viewer"], "viewer")?;
        Ok(Actor::person(viewer.id, viewer.name))
    }

    fn list(&self, query: &Query) -> CapResult<Page<Task>> {
        let filter = helpers::issue_filter(query, self.team.as_deref(), &self.account)?;
        let variables = json!({
            "first": query.limit.unwrap_or(PAGE_DEFAULT).clamp(1, PAGE_MAX),
            "after": query.cursor,
            "filter": filter,
            "orderBy": helpers::order_by(query)?,
        });
        let data = self.run_issue(types::LIST, variables)?;
        helpers::task_page(&self.account, &data["issues"])
    }

    fn get(&self, task: &Ref) -> CapResult<Task> {
        helpers::task_from(&self.account, &self.read_issue(&self.task_key(task)?)?)
    }

    fn create(&self, new: &NewTask, _by: &Actor) -> CapResult<Task> {
        // Linear writes as the owner of the key, so `_by` has nowhere to go.
        if new.title.trim().is_empty() {
            return Err(CapError::invalid("title"));
        }
        let parent = match &new.parent {
            Some(parent) => Some(self.issue_uuid(parent)?),
            None => None,
        };
        let input = helpers::create_input(new, &self.team_id()?, parent, &self.account)?;
        let data = self.run_issue(types::CREATE, json!({ "input": input }))?;
        helpers::task_from(
            &self.account,
            &helpers::mutated(&data["issueCreate"], "issue")?,
        )
    }

    fn update(&self, task: &Ref, patch: &Patch, version: &str, _by: &Actor) -> CapResult<Task> {
        let key = self.task_key(task)?;
        let current = helpers::task_from(&self.account, &self.read_issue(&key)?)?;
        // Linear has no "only if unchanged", so the check is a read before the write. A change that lands in
        // between is not caught; the window is one round trip.
        if current.version != version {
            return Err(CapError::Conflict {
                current: serde_json::to_value(&current).unwrap_or(Value::Null),
            });
        }
        let parent = match &patch.parent {
            Change::Set(parent) => Change::Set(self.issue_uuid(parent)?),
            Change::Clear => Change::Clear,
            Change::Keep => Change::Keep,
        };
        let input = helpers::update_input(patch, parent, &self.account)?;
        if input.as_object().is_none_or(|o| o.is_empty()) {
            return Ok(current);
        }
        let data = self.run_issue(types::UPDATE, json!({ "id": key, "input": input }))?;
        helpers::task_from(
            &self.account,
            &helpers::mutated(&data["issueUpdate"], "issue")?,
        )
    }

    fn comment(&self, task: &Ref, body: &str, by: &Actor) -> CapResult<Comment> {
        if body.trim().is_empty() {
            return Err(CapError::invalid("body"));
        }
        let issue_id = self.issue_uuid(task)?;
        let input = json!({ "issueId": issue_id, "body": helpers::comment_body(body, by) });
        let data = self
            .client
            .run(types::COMMENT_CREATE, json!({ "input": input }))?;
        let comment = helpers::mutated(&data["commentCreate"], "comment")?;
        helpers::comment_from(&self.account, task, &comment)
    }

    fn activity(&self, task: &Ref, cursor: Option<&str>) -> CapResult<Page<Activity>> {
        let key = self.task_key(task)?;
        let issue = self.read_issue(&key)?;
        let history = helpers::collect(
            &self.client,
            types::HISTORY_OF,
            json!({ "id": key }),
            "/issue/history",
        )?;
        let comments = helpers::collect(
            &self.client,
            types::COMMENTS,
            json!({ "id": key }),
            "/issue/comments",
        )?;
        helpers::activity_page(&self.account, task, &issue, &history, &comments, cursor)
    }

    fn labels(&self) -> CapResult<Vec<Label>> {
        let nodes = helpers::collect(&self.client, types::LABELS, json!({}), "/issueLabels")?;
        nodes
            .iter()
            .map(|n| helpers::label_from(&self.account, n))
            .collect()
    }

    fn projects(&self) -> CapResult<Vec<Project>> {
        let nodes = helpers::collect(&self.client, types::PROJECTS, json!({}), "/projects")?;
        nodes
            .iter()
            .map(|n| helpers::project_from(&self.account, n))
            .collect()
    }

    fn statuses(&self) -> CapResult<Vec<Status>> {
        let filter = self.team.as_ref().map_or(
            Value::Null,
            |key| json!({ "team": { "key": { "eq": key } } }),
        );
        let nodes = helpers::collect(
            &self.client,
            types::STATES,
            json!({ "filter": filter }),
            "/workflowStates",
        )?;
        nodes.iter().map(helpers::status_from).collect()
    }

    fn subscribe(&self) -> CapResult<Subscription<Event>> {
        let mut watermark = helpers::latest_update(&self.client, self.team.as_deref())?;
        let (tx, rx) = channel();
        let (subscription, stop) = Subscription::new(rx);
        let client = self.client.clone();
        let (account, team, every) = (self.account.clone(), self.team.clone(), self.poll_every);
        thread::spawn(move || {
            loop {
                // Sleep in short steps, so a dropped subscription ends the thread at once and not after a tick.
                let mut slept = Duration::ZERO;
                while slept < every {
                    if stop.is_stopped() {
                        return;
                    }
                    let step = types::STOP_CHECK.min(every - slept);
                    thread::sleep(step);
                    slept += step;
                }
                match helpers::changes_since(&client, &account, team.as_deref(), &watermark) {
                    Ok((events, newest)) => {
                        for event in events {
                            if tx.send(event).is_err() {
                                return;
                            }
                        }
                        watermark = newest.unwrap_or(watermark);
                    }
                    // A key that stopped working will not work at the next tick either.
                    Err(CapError::NotSignedIn) => return,
                    // Offline, rate limited, a hiccup: try again at the next tick.
                    Err(_) => {}
                }
            }
        });
        Ok(subscription)
    }
}

// What Linear sends, as far as the provider reads it. The whole node is kept as `raw` beside the neutral view.

#[derive(Deserialize)]
pub struct Viewer {
    pub id: String,
    pub name: String,
    pub organization: Organization,
}

#[derive(Deserialize)]
pub struct Organization {
    #[serde(rename = "urlKey")]
    pub url_key: String,
}

#[derive(Deserialize)]
pub struct TeamNode {
    pub id: String,
    pub key: String,
}

#[derive(Deserialize)]
pub struct UserNode {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct BotNode {
    pub name: String,
}

#[derive(Deserialize)]
pub struct StateNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Deserialize)]
pub struct LabelNode {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Deserialize)]
pub struct ProjectNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "slugId")]
    pub slug_id: String,
}

#[derive(Deserialize)]
pub struct ParentNode {
    pub identifier: String,
}

#[derive(Deserialize)]
pub struct Nodes<T> {
    pub nodes: Vec<T>,
}

#[derive(Deserialize)]
pub struct IssueNode {
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: u8,
    pub estimate: Option<f64>,
    pub creator: Option<UserNode>,
    #[serde(rename = "dueDate")]
    pub due_date: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub state: StateNode,
    pub assignee: Option<UserNode>,
    pub labels: Option<Nodes<LabelNode>>,
    pub project: Option<ProjectNode>,
    pub parent: Option<ParentNode>,
}

#[derive(Deserialize)]
pub struct CommentNode {
    pub id: String,
    pub body: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    pub user: Option<UserNode>,
    #[serde(rename = "botActor")]
    pub bot_actor: Option<BotNode>,
}

#[derive(Deserialize)]
pub struct HistoryNode {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub actor: Option<UserNode>,
    #[serde(rename = "botActor")]
    pub bot_actor: Option<BotNode>,
    #[serde(rename = "toState")]
    pub to_state: Option<StateNode>,
    #[serde(rename = "fromAssignee")]
    pub from_assignee: Option<UserNode>,
    #[serde(rename = "toAssignee")]
    pub to_assignee: Option<UserNode>,
}
