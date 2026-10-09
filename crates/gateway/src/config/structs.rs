use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use super::helpers::{config_json, create_private_dir, open_private_file};
use crate::{SessionAccess, session::random_hex};

/// A config file for one session. It holds the session's token, so it is private to the user (mode 0600, in a
/// folder of mode 0700) and goes away with the value.
#[derive(Debug)]
pub struct McpConfig {
    path: PathBuf,
}

impl McpConfig {
    /// Writes the config for `access` into `dir`, which is made when it is missing. Each call makes its own file.
    pub fn write(dir: &Path, access: &SessionAccess) -> io::Result<Self> {
        create_private_dir(dir)?;
        let path = dir.join(format!("mcp-{}.json", random_hex(8)?));
        // From here the file exists, so the value that removes it must exist too: a failed write removes it itself.
        let config = Self { path };
        let mut file = open_private_file(&config.path)?;
        file.write_all(config_json(access).as_bytes())?;
        file.flush()?;
        Ok(config)
    }

    /// The path to give `--mcp-config`.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for McpConfig {
    fn drop(&mut self) {
        // Already gone is fine. Anything else leaves a file only its owner can read.
        let _ = fs::remove_file(&self.path);
    }
}
