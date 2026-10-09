use std::sync::Arc;

use crate::{McpConfig, SessionAccess, server::Shared};

/// A session token and the config file that carries it. Dropping the grant revokes the token and removes the file, so
/// a session that ends leaves nothing that opens the gateway.
pub struct Grant {
    pub(crate) shared: Arc<Shared>,
    pub(crate) access: SessionAccess,
    pub(crate) config: McpConfig,
}

impl Grant {
    /// The file to give `claude --mcp-config`.
    pub fn config_path(&self) -> &std::path::Path {
        self.config.path()
    }

    pub fn access(&self) -> &SessionAccess {
        &self.access
    }
}

impl Drop for Grant {
    fn drop(&mut self) {
        self.shared.revoke(&self.access.token);
    }
}
