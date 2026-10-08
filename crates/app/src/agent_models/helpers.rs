use atelier_agents::{registry::Agent, session::ModelChoice};
use atelier_settings::AgentModels;
use gpui_kit::{App, AppContext};

use super::structs::ModelPrefs;

fn prefs_of(backend: &str, cx: &App) -> AgentModels {
    cx.try_global::<ModelPrefs>().and_then(|p| p.0.get(backend).cloned()).unwrap_or_default()
}

/// Keeps `change` on the backend's arrangement, in the global and in the settings file.
fn update(backend: &str, change: impl Fn(&mut AgentModels) + Send + 'static, cx: &mut App) {
    let mut all = cx.try_global::<ModelPrefs>().cloned().unwrap_or_default();
    change(all.0.entry(backend.to_string()).or_default());
    cx.set_global(all);
    let backend = backend.to_string();
    crate::settings_pane::save(cx, move |s| change(s.agent_models.entry(backend).or_default()));
}

/// The models to offer for `agent`: what it last reported else its built-in list, in the reader's order.
pub fn offered(agent: &Agent, cx: &App) -> Vec<ModelChoice> {
    let built_in: Vec<(String, String)> = agent.backend.capabilities().models.into_iter().map(|m| (m.id, m.label)).collect();
    prefs_of(agent.backend.name(), cx).arranged(&built_in).into_iter().map(|(id, label)| ModelChoice { id, label }).collect()
}

/// The model a new session of `agent` starts on, when the app names one: the reader's default, else the first of a list the agent
/// reported. A built-in list names none: the agent's own default stands, and its picker shows the first.
pub fn start_model(agent: &Agent, cx: &App) -> Option<String> {
    let prefs = prefs_of(agent.backend.name(), cx);
    let list = offered(agent, cx);
    let pairs: Vec<(String, String)> = list.iter().map(|m| (m.id.clone(), m.label.clone())).collect();
    prefs.default_in(&pairs).or_else(|| (!prefs.known.is_empty()).then(|| list.first().map(|m| m.id.clone())).flatten())
}

/// The reader starred `id`: new sessions of the agent start on it.
pub fn choose_default(backend: &str, id: &str, cx: &mut App) {
    let id = id.to_string();
    update(backend, move |p| p.default = Some(id.clone()), cx);
}

/// The reader sorted the list: every id, first to last.
pub fn reorder(backend: &str, order: Vec<String>, cx: &mut App) {
    update(backend, move |p| p.order = order.clone(), cx);
}

/// Asks each agent that has a service of its own for its models, off the main thread, and keeps what comes. A failure keeps what was there.
pub fn refresh(cx: &mut App) {
    for agent in atelier_agents::registry::agents() {
        let backend = agent.backend.clone();
        let name = backend.name().to_string();
        let asking = cx.background_spawn(async move {
            let dir = std::env::temp_dir();
            let project = atelier_project::LocalProject::open(&dir).ok()?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
            backend.list_models(&project, now)?.ok()
        });
        cx.spawn(async move |cx| {
            let Some(models) = asking.await else { return };
            let known: Vec<(String, String)> = models.into_iter().map(|m| (m.id, m.label)).collect();
            cx.update(|cx| update(&name, move |p| p.known = known.clone(), cx));
        })
        .detach();
    }
}
