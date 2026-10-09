use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

use serde_json::json;

use crate::SessionAccess;

/// The name Claude Code shows the server under: its tools are `mcp__atelier__<tool>`.
pub(super) const SERVER_NAME: &str = "atelier";

pub(super) fn config_json(access: &SessionAccess) -> String {
    json!({
        "mcpServers": {
            SERVER_NAME: {
                "type": "http",
                "url": access.url,
                "headers": { "Authorization": format!("Bearer {}", access.token) },
            }
        }
    })
    .to_string()
}

/// The folder, with every missing parent, readable by the user alone. A folder that exists keeps its mode.
pub(super) fn create_private_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    // The mask of the process may have cut the mode; the folder is ours, so set it plainly.
    fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))
}

/// A new file of mode 0600. `create_new` refuses a file that is there already, so nothing else's mode is touched.
pub(super) fn open_private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}
