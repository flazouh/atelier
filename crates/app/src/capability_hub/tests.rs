use std::sync::Arc;

use atelier_capabilities::Actor;
use atelier_project::{LocalProject, Project};
use serde_json::{Value, json};

use super::CapabilityHub;

fn hub(dir: &std::path::Path, enabled: bool) -> CapabilityHub {
    CapabilityHub::new(
        Actor::person("alex", "Alex"),
        Some(dir.join("run")),
        enabled,
    )
}

/// A project in a folder named `name` under `base`, with its data under `base/data`.
fn project(base: &std::path::Path, name: &str) -> LocalProject {
    let root = base.join(name);
    std::fs::create_dir_all(&root).unwrap();
    LocalProject::open(&root)
        .unwrap()
        .with_data_dir(&base.join("data"))
}

fn call(url: &str, token: &str, tool: &str, arguments: Value) -> Value {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
    let mut response = agent
        .post(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .send(body.to_string())
        .unwrap();
    serde_json::from_str(&response.body_mut().read_to_string().unwrap()).unwrap()
}

#[test]
fn a_session_of_a_project_gets_a_grant_that_creates_tasks_in_its_tracker() {
    let base = tempfile::tempdir().unwrap();
    let project = project(base.path(), "atelier");
    // The hub owns the gateway, so it must outlive the calls, as the app-wide one does.
    let hub = hub(base.path(), true);
    let grant = hub.grant(&project).expect("a grant");
    let access = grant.access().clone();
    let answer = call(
        &access.url,
        &access.token,
        "tasks_create",
        json!({ "title": "From the agent" }),
    );
    assert_eq!(answer["result"]["isError"], json!(false), "{answer}");
    assert!(
        answer["result"]["structuredContent"]["task"]["ref"]
            .as_str()
            .unwrap()
            .starts_with("tasks:local:atelier:")
    );
    let tracker = project.tracker().unwrap();
    let tasks = tracker.list(&Default::default()).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "From the agent");
}

#[test]
fn the_agent_that_wrote_is_named_and_the_grant_ends_with_the_session() {
    let base = tempfile::tempdir().unwrap();
    let project = project(base.path(), "atelier");
    let hub = hub(base.path(), true);
    let grant = hub.grant(&project).unwrap();
    let access = grant.access().clone();
    let made = call(
        &access.url,
        &access.token,
        "tasks_create",
        json!({ "title": "Mine" }),
    );
    let reference = made["result"]["structuredContent"]["task"]["ref"]
        .as_str()
        .unwrap()
        .to_string();
    let comment = call(
        &access.url,
        &access.token,
        "tasks_comment",
        json!({ "ref": reference, "body": "Hello" }),
    );
    assert_eq!(
        comment["result"]["structuredContent"]["comment"]["author"]["name"],
        "Claude Code"
    );
    let path = grant.config_path().to_path_buf();
    assert!(path.exists());
    drop(grant);
    assert!(!path.exists());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let refused = agent
        .post(&access.url)
        .header("Authorization", format!("Bearer {}", access.token))
        .send("{}")
        .unwrap();
    assert_eq!(refused.status().as_u16(), 401);
}

#[test]
fn the_setting_turns_it_off() {
    let base = tempfile::tempdir().unwrap();
    let project = project(base.path(), "atelier");
    assert!(hub(base.path(), false).grant(&project).is_none());
    assert!(!base.path().join("run").exists(), "nothing was written");
}

#[test]
fn without_a_folder_for_the_file_the_session_goes_on_without() {
    let base = tempfile::tempdir().unwrap();
    let project = project(base.path(), "atelier");
    let hub = CapabilityHub::new(Actor::person("alex", "Alex"), None, true);
    assert!(hub.grant(&project).is_none());
}

#[test]
fn a_folder_that_cannot_be_made_is_a_session_without_tools() {
    let base = tempfile::tempdir().unwrap();
    let project = project(base.path(), "atelier");
    // A file where the folder should be.
    std::fs::write(base.path().join("run"), "in the way").unwrap();
    assert!(hub(base.path(), true).grant(&project).is_none());
}

#[test]
fn two_sessions_of_one_project_share_one_provider_and_one_gateway() {
    let base = tempfile::tempdir().unwrap();
    let project = project(base.path(), "atelier");
    let hub = hub(base.path(), true);
    let (one, two) = (hub.grant(&project).unwrap(), hub.grant(&project).unwrap());
    assert_eq!(one.access().url, two.access().url);
    assert_ne!(one.access().token, two.access().token);
    let listed = call(
        &one.access().url,
        &one.access().token,
        "tasks_list",
        json!({}),
    );
    assert_eq!(
        listed["result"]["isError"],
        json!(false),
        "one provider, so no account is needed: {listed}"
    );
}

#[test]
fn two_projects_in_folders_of_one_name_get_two_accounts() {
    let base = tempfile::tempdir().unwrap();
    let (first, second) = (
        project(&base.path().join("a"), "app"),
        project(&base.path().join("b"), "app"),
    );
    let hub = hub(base.path(), true);
    let grant = hub.grant(&first).unwrap();
    hub.grant(&second).unwrap();
    let missing = call(
        &grant.access().url,
        &grant.access().token,
        "tasks_list",
        json!({}),
    );
    let text = missing["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("local/app") && text.contains("local/app-2"),
        "{text}"
    );
}

/// A project that says it lives on another host, and is the local one in every other way.
struct Remote(LocalProject);

impl Project for Remote {
    fn root(&self) -> &std::path::Path {
        self.0.root()
    }
    fn list(&self) -> std::io::Result<Vec<atelier_project::Entry>> {
        self.0.list()
    }
    fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
        self.0.read(path)
    }
    fn write(&self, path: &str, bytes: &[u8]) -> std::io::Result<()> {
        self.0.write(path, bytes)
    }
    fn watch(&self, sink: atelier_project::ChangeSink) -> std::io::Result<atelier_project::Watch> {
        self.0.watch(sink)
    }
    fn search(
        &self,
        query: &atelier_project::Query,
    ) -> std::io::Result<Vec<atelier_project::Match>> {
        self.0.search(query)
    }
    fn spawn(
        &self,
        command: &atelier_project::Command,
    ) -> std::io::Result<atelier_project::Process> {
        self.0.spawn(command)
    }
    fn git(&self, args: &[&str]) -> std::io::Result<atelier_project::GitOutput> {
        self.0.git(args)
    }
    fn host(&self) -> Option<&str> {
        Some("far-away")
    }
    fn tracker(&self) -> atelier_tracker::TrackerResult<Arc<dyn atelier_tracker::Tracker>> {
        self.0.tracker()
    }
}

#[test]
fn a_project_on_another_host_gets_nothing() {
    let base = tempfile::tempdir().unwrap();
    let remote = Remote(project(base.path(), "atelier"));
    assert!(hub(base.path(), true).grant(&remote).is_none());
    assert!(!base.path().join("run").exists(), "no port, no file");
}

fn memory(account: &str) -> Arc<dyn atelier_capabilities::tasks::TasksProvider> {
    Arc::new(atelier_capabilities::tasks::MemoryTasks::new(account))
}

/// What a refresh found: these providers work, and the rows say so.
fn built(providers: Vec<Arc<dyn atelier_capabilities::tasks::TasksProvider>>) -> crate::accounts::Built {
    use crate::accounts::{Row, Rows};
    let row = |on: bool| if on { Row::Connected("Ada".into()) } else { Row::Off };
    crate::accounts::Built { rows: Rows { linear: row(!providers.is_empty()), github: Row::Off }, providers }
}

fn same(a: &Arc<dyn atelier_capabilities::tasks::TasksProvider>, b: &Arc<dyn atelier_capabilities::tasks::TasksProvider>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

fn saved_linear() -> atelier_settings::AccountsSaved {
    atelier_settings::AccountsSaved { linear: Some(Default::default()), ..Default::default() }
}

#[test]
fn a_connected_account_is_in_the_registry_until_it_is_taken_out() {
    let base = tempfile::tempdir().unwrap();
    let hub = hub(base.path(), true);
    let acme = memory("acme");

    let turn = hub.checking(&saved_linear());
    assert!(hub.install(built(vec![acme.clone()]), turn));
    let held = hub.registry().read().unwrap().tasks("memory", "acme");
    assert!(held.is_some_and(|held| same(&held, &acme)), "the gateway's registry holds the very provider");
    assert_eq!(hub.accounts().len(), 1);

    let turn = hub.checking(&Default::default());
    assert!(hub.install(built(Vec::new()), turn));
    assert!(hub.registry().read().unwrap().tasks("memory", "acme").is_none(), "gone from the registry");
    assert!(hub.accounts().is_empty());
}

#[test]
fn the_answer_of_an_older_refresh_is_dropped() {
    let base = tempfile::tempdir().unwrap();
    let hub = hub(base.path(), true);
    let old = hub.checking(&saved_linear());
    let new = hub.checking(&Default::default());

    assert!(!hub.install(built(vec![memory("acme")]), old), "a newer refresh began");
    assert!(hub.accounts().is_empty());
    assert!(hub.install(built(Vec::new()), new));
}

#[test]
fn a_saved_account_reads_checking_until_its_answer_comes() {
    use crate::accounts::Row;
    let base = tempfile::tempdir().unwrap();
    let hub = hub(base.path(), true);
    hub.checking(&saved_linear());
    assert_eq!((hub.rows().linear, hub.rows().github), (Row::Checking, Row::Off));
}

#[test]
fn the_screen_and_the_gateway_read_the_same_local_provider() {
    let base = tempfile::tempdir().unwrap();
    let project = project(base.path(), "atelier");
    let hub = hub(base.path(), true);

    let for_the_screen = hub.local_tasks(&project).unwrap();
    hub.grant(&project).unwrap();
    let for_the_gateway = hub.registry().read().unwrap().tasks("local", "atelier").unwrap();

    assert!(same(&for_the_screen, &for_the_gateway), "one provider for the project, not one each");
    let opened = crate::tasks::source::TasksSource::open(&(Arc::new(project) as Arc<dyn Project>), &hub).unwrap();
    assert!(same(&opened.provider().unwrap(), &for_the_gateway), "and the screen's source holds it");
    assert_eq!(hub.registry().read().unwrap().all_tasks().len(), 1, "registered once");
}

#[test]
fn the_screen_and_an_agent_see_the_same_connected_account() {
    let base = tempfile::tempdir().unwrap();
    let project = Arc::new(project(base.path(), "atelier"));
    let hub = hub(base.path(), true);
    let acme = memory("acme");
    acme.create(&atelier_capabilities::tasks::NewTask::titled("From Linear"), &Actor::person("ada", "Ada")).unwrap();
    let turn = hub.checking(&saved_linear());
    hub.install(built(vec![acme.clone()]), turn);

    let mut source = crate::tasks::source::TasksSource::open(&(project.clone() as Arc<dyn Project>), &hub).unwrap();
    source.set_accounts(hub.accounts());
    let words: Vec<_> = source.choices().iter().map(|c| c.words().to_string()).collect();
    assert_eq!(words, ["Local · atelier", "Memory · acme"], "the switcher lists both");

    let grant = hub.grant(project.as_ref()).unwrap();
    let listed = call(&grant.access().url, &grant.access().token, "tasks_list", json!({ "account": "memory/acme" }));
    assert_eq!(listed["result"]["isError"], json!(false), "{listed}");
    assert!(listed.to_string().contains("From Linear"), "the agent reads the account: {listed}");
}
