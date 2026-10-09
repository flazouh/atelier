use std::sync::{Arc, RwLock};

use atelier_capabilities::{Actor, Registry};
use serde_json::Value;

use super::{
    helpers::{channels, history, search, send, thread},
    schemas::definitions,
};
use crate::{ToolDef, ToolResult, ToolSet};

/// The messaging tools. They read the registry at each call, so an account the app adds later is there without a
/// restart, and the list of tools follows what the connected accounts can do.
pub struct MessagingTools {
    registry: Arc<RwLock<Registry>>,
}

impl MessagingTools {
    pub fn new(registry: Arc<RwLock<Registry>>) -> Self {
        Self { registry }
    }
}

impl ToolSet for MessagingTools {
    fn tools(&self) -> Vec<ToolDef> {
        let registry = self.registry.read().unwrap_or_else(|p| p.into_inner());
        definitions(&registry)
    }

    fn call(&self, tool: &str, arguments: &Value, actor: &Actor) -> Option<ToolResult> {
        let registry = self.registry.read().unwrap_or_else(|p| p.into_inner());
        // A tool that is not listed does not exist for the model, so a call to it is "not a tool of this server".
        if !definitions(&registry).iter().any(|def| def.name == tool) {
            return None;
        }
        let result = match tool {
            "messaging_channels" => channels(&registry, arguments),
            "messaging_history" => history(&registry, arguments),
            "messaging_thread" => thread(&registry, arguments),
            "messaging_search" => search(&registry, arguments),
            "messaging_send" => send(&registry, arguments, actor),
            _ => return None,
        };
        Some(result.unwrap_or_else(ToolResult::error))
    }
}
