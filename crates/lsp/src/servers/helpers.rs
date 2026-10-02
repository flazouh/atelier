use std::path::{Path, PathBuf};

use serde_json::Value;

use super::structs::ServerSpec;
use super::types::{LANGUAGES, SERVERS};

/// For a server that needs no options.
pub(super) fn no_options(_: &Path, _: &Path) -> Option<Value> {
    None
}

/// The standard LSP language id for a file, by its extension.
pub fn language_id(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?;
    LANGUAGES.iter().find(|(_, extensions)| extensions.contains(&extension)).map(|(id, _)| *id)
}

/// The server for a language id, if atelier knows one.
pub fn server_for(language_id: &str) -> Option<&'static ServerSpec> {
    SERVERS.iter().find(|spec| spec.language_ids.contains(&language_id))
}

/// Where a program is: on `PATH`, then in the places installers put language servers. An app opened
/// from the Finder has almost no `PATH`, so a server found in a terminal must still be found here.
pub fn find_program(program: &str) -> Option<PathBuf> {
    find_program_in(program, &search_dirs())
}

/// The directories [`find_program`] looks in, in order.
pub fn search_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(
        [".cargo/bin", ".local/bin", "go/bin"].iter().map(|d| home.join(d)).chain(
            ["/opt/homebrew/bin", "/usr/local/bin"].iter().map(PathBuf::from),
        ),
    );
    dirs
}

/// [`find_program`] over an explicit list of directories, in order.
pub fn find_program_in(program: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().map(|dir| dir.join(program)).find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// The project root for `file`: the nearest directory above it that holds one of `markers`, or the
/// file's own directory when none does.
pub fn find_root(file: &Path, markers: &[&str]) -> PathBuf {
    let start = file.parent().unwrap_or(file);
    start
        .ancestors()
        .find(|dir| markers.iter().any(|marker| dir.join(marker).exists()))
        .unwrap_or(start)
        .to_path_buf()
}
