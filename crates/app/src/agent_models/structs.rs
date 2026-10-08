use std::collections::BTreeMap;

use atelier_settings::AgentModels;
use gpui_kit::Global;

/// The reader's arrangement of each agent's models, by the agent's backend name.
#[derive(Clone, Debug, Default)]
pub struct ModelPrefs(pub BTreeMap<String, AgentModels>);

impl Global for ModelPrefs {}
