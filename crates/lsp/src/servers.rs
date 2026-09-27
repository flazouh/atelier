//! Which language server serves which file. This is the only place that names a language or a
//! server: everything else takes a [`ServerSpec`] and works the same for all of them. Adding a
//! language is adding a row to [`SERVERS`].

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// How to run one language server, and for which files.
#[derive(Debug)]
pub struct ServerSpec {
    /// What the status line calls it.
    pub name: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    /// The LSP language ids it serves, as `language_id` returns them.
    pub language_ids: &'static [&'static str],
    /// Files whose directory is a project root for this server.
    pub root_markers: &'static [&'static str],
    /// How to install it, for the status line when it is missing.
    pub install: &'static str,
    /// The `initializationOptions` to send, from where the program is and the project root.
    pub initialization_options: fn(program: &Path, root: &Path) -> Option<Value>,
}

/// For a server that needs no options.
fn no_options(_: &Path, _: &Path) -> Option<Value> {
    None
}

/// The servers lathe knows how to run.
pub const SERVERS: &[ServerSpec] = &[
    ServerSpec {
        name: "rust-analyzer",
        program: "rust-analyzer",
        args: &[],
        language_ids: &["rust"],
        root_markers: &["Cargo.toml"],
        install: "rustup component add rust-analyzer",
        initialization_options: no_options,
    },
    ServerSpec {
        name: "typescript-language-server",
        program: "typescript-language-server",
        args: &["--stdio"],
        language_ids: &["typescript", "typescriptreact", "javascript", "javascriptreact"],
        root_markers: &["tsconfig.json", "jsconfig.json", "package.json"],
        install: "npm install -g typescript@5 typescript-language-server",
        initialization_options: typescript_options,
    },
    ServerSpec {
        name: "pyright",
        program: "pyright-langserver",
        args: &["--stdio"],
        language_ids: &["python"],
        root_markers: &["pyproject.toml", "setup.py", "requirements.txt", "pyrightconfig.json"],
        install: "npm install -g pyright",
        initialization_options: no_options,
    },
    ServerSpec {
        name: "gopls",
        program: "gopls",
        args: &[],
        language_ids: &["go"],
        root_markers: &["go.mod"],
        install: "go install golang.org/x/tools/gopls@latest",
        initialization_options: no_options,
    },
    ServerSpec {
        name: "jdtls",
        program: "jdtls",
        args: &[],
        language_ids: &["java"],
        root_markers: &["pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle"],
        install: "brew install jdtls",
        initialization_options: no_options,
    },
];

/// typescript-language-server runs the project's own TypeScript, from `node_modules`. A project
/// without one, such as a single file, would make it refuse to start, so it is pointed at the
/// TypeScript installed beside the server itself, as editors do.
fn typescript_options(program: &Path, root: &Path) -> Option<Value> {
    if root.join("node_modules/typescript/lib/tsserver.js").exists() {
        return None;
    }
    let installed = std::fs::canonicalize(program).ok()?;
    let modules = installed.ancestors().find(|dir| dir.file_name().is_some_and(|n| n == "node_modules"))?;
    let lib = modules.join("typescript/lib");
    lib.join("tsserver.js").exists().then(|| json!({ "tsserver": { "path": lib } }))
}

/// The standard LSP language id for a file, by its extension.
pub fn language_id(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?;
    Some(match extension {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "py" | "pyi" => "python",
        "go" => "go",
        "java" => "java",
        _ => return None,
    })
}

/// The server for a language id, if lathe knows one.
pub fn server_for(language_id: &str) -> Option<&'static ServerSpec> {
    SERVERS.iter().find(|spec| spec.language_ids.contains(&language_id))
}

/// Where a program is: on `PATH`, then in the places installers put language servers. An app opened
/// from the Finder has almost no `PATH`, so a server found in a terminal must still be found here.
pub fn find_program(program: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(
        [".cargo/bin", ".local/bin", "go/bin"].iter().map(|d| home.join(d)).chain(
            ["/opt/homebrew/bin", "/usr/local/bin"].iter().map(PathBuf::from),
        ),
    );
    find_program_in(program, &dirs)
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

#[cfg(test)]
mod tests;
