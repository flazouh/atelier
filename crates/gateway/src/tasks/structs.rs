use std::sync::{Arc, RwLock};

use atelier_capabilities::{Actor, Registry};
use serde_json::Value;

use super::{
    helpers::{comment, create, get, list, search, update},
    schemas::definitions,
};
use crate::{ToolDef, ToolResult, ToolSet};

/// The tasks tools. They read the registry at each call, so a provider the app adds later is there without a restart.
pub struct TasksTools {
    registry: Arc<RwLock<Registry>>,
}

impl TasksTools {
    pub fn new(registry: Arc<RwLock<Registry>>) -> Self {
        Self { registry }
    }
}

impl ToolSet for TasksTools {
    fn tools(&self) -> Vec<ToolDef> {
        definitions()
    }

    fn call(&self, tool: &str, arguments: &Value, actor: &Actor) -> Option<ToolResult> {
        let registry = self.registry.read().unwrap_or_else(|p| p.into_inner());
        let result = match tool {
            "tasks_list" => list(&registry, arguments),
            "tasks_search" => search(&registry, arguments),
            "tasks_get" => get(&registry, arguments),
            "tasks_create" => create(&registry, arguments, actor),
            "tasks_update" => update(&registry, arguments, actor),
            "tasks_comment" => comment(&registry, arguments, actor),
            _ => return None,
        };
        Some(result.unwrap_or_else(ToolResult::error))
    }
}
