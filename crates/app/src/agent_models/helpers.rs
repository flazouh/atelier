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

/// Every model of `agent` in the reader's order, with whether the reader hid it: what Settings lists.
pub fn all(agent: &Agent, cx: &App) -> Vec<(ModelChoice, bool)> {
    let built_in: Vec<(String, String)> = agent.backend.capabilities().models.into_iter().map(|m| (m.id, m.label)).collect();
    let prefs = prefs_of(agent.backend.name(), cx);
    let list = prefs.arranged(&built_in);
    let shown = prefs.visible(&list);
    list.into_iter().map(|(id, label)| { let hidden = !shown.iter().any(|(s, _)| *s == id); (ModelChoice { id, label }, hidden) }).collect()
}

/// The models to offer for `agent` in a picker: what it last reported else its built-in list, in the reader's order, without the hidden.
pub fn offered(agent: &Agent, cx: &App) -> Vec<ModelChoice> {
    all(agent, cx).into_iter().filter(|(_, hidden)| !hidden).map(|(model, _)| model).collect()
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
    // A model started on is not left out of the picker.
    update(backend, move |p| {
        p.default = Some(id.clone());
        p.hidden.retain(|h| *h != id);
    }, cx);
}

/// The reader pressed an eye: `id` leaves the picker, or comes back to it.
pub fn set_hidden(backend: &str, id: &str, hidden: bool, cx: &mut App) {
    let id = id.to_string();
    update(backend, move |p| {
        p.hidden.retain(|h| *h != id);
        if hidden {
            p.hidden.push(id.clone());
        }
    }, cx);
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
