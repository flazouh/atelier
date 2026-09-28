//! Which language server serves which file. This is the only place that names a language or a
//! server: everything else takes a [`ServerSpec`] and works the same for all of them. Adding a
//! language is adding a row to [`LANGUAGES`] and, if no server already covers it, one to [`SERVERS`].

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::install::{Download, Kind, Package, PlatformFile, Unpack};

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
    /// How to install it, for the status line when it is missing and lathe cannot download it.
    pub install: &'static str,
    /// The pinned copy lathe downloads when the user has none.
    pub download: Option<Download>,
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
        download: Some(RUST_ANALYZER),
        initialization_options: no_options,
    },
    ServerSpec {
        name: "typescript-language-server",
        program: "typescript-language-server",
        args: &["--stdio"],
        language_ids: &["typescript", "typescriptreact", "javascript", "javascriptreact"],
        root_markers: &["tsconfig.json", "jsconfig.json", "package.json"],
        install: "npm install -g typescript@5 typescript-language-server",
        download: Some(Download {
            version: "6.0.1",
            kind: Kind::Node {
                packages: &[
                    Package {
                        name: "typescript-language-server",
                        url: "https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-6.0.1.tgz",
                        sha256: "85eabb9251d85d3798b247b2bc0895ca111d866bfbf59533f64b2461b0d8649f",
                    },
                    Package {
                        name: "typescript",
                        url: "https://registry.npmjs.org/typescript/-/typescript-5.9.3.tgz",
                        sha256: "10e108c9cf7d5f2879053dff18515fb405abf2ccef63eaaf017d9c571687a1d3",
                    },
                ],
                script: "node_modules/typescript-language-server/lib/cli.mjs",
            },
        }),
        initialization_options: typescript_options,
    },
    ServerSpec {
        name: "pyright",
        program: "pyright-langserver",
        args: &["--stdio"],
        language_ids: &["python"],
        root_markers: &["pyproject.toml", "setup.py", "requirements.txt", "pyrightconfig.json"],
        install: "npm install -g pyright",
        download: Some(Download {
            version: "1.1.414",
            kind: Kind::Node {
                packages: &[Package {
                    name: "pyright",
                    url: "https://registry.npmjs.org/pyright/-/pyright-1.1.414.tgz",
                    sha256: "bf5f473f6167c0d14175492c3263d783b4489a6956e1c06c18e15228e3a3fa42",
                }],
                script: "node_modules/pyright/langserver.index.js",
            },
        }),
        initialization_options: no_options,
    },
    ServerSpec {
        name: "gopls",
        program: "gopls",
        args: &[],
        language_ids: &["go"],
        root_markers: &["go.mod"],
        install: "go install golang.org/x/tools/gopls@latest",
        download: Some(Download {
            version: "v0.23.0",
            kind: Kind::Go { module: "golang.org/x/tools/gopls" },
        }),
        initialization_options: no_options,
    },
    ServerSpec {
        name: "jdtls",
        program: "jdtls",
        args: &[],
        language_ids: &["java"],
        root_markers: &["pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle"],
        install: "brew install jdtls",
        // jdtls needs a Java runtime as well, which lathe does not download yet.
        download: None,
        initialization_options: no_options,
    },
];

/// rust-analyzer's own release builds, one gzipped binary per platform.
const RUST_ANALYZER: Download = Download {
    version: "2026-09-21",
    kind: Kind::Platform {
        files: &[
            PlatformFile {
                platform: "aarch64-apple-darwin",
                url: "https://github.com/rust-lang/rust-analyzer/releases/download/2026-09-21/rust-analyzer-aarch64-apple-darwin.gz",
                sha256: "eb474adfd12b6e66a6d0a7c25ed51210a64a0237f326f898e32e25164c9b11ad",
            },
            PlatformFile {
                platform: "x86_64-apple-darwin",
                url: "https://github.com/rust-lang/rust-analyzer/releases/download/2026-09-21/rust-analyzer-x86_64-apple-darwin.gz",
                sha256: "1e2f706ee97d9f931ea36ed4e83f839efd0ac09870e18ec8350877d7b98ad8a0",
            },
            PlatformFile {
                platform: "aarch64-unknown-linux-gnu",
                url: "https://github.com/rust-lang/rust-analyzer/releases/download/2026-09-21/rust-analyzer-aarch64-unknown-linux-gnu.gz",
                sha256: "32a75657041a7a2ebf635c1e3968753e2639778b68dd4fb13c4b2584255f2ba8",
            },
            PlatformFile {
                platform: "x86_64-unknown-linux-gnu",
                url: "https://github.com/rust-lang/rust-analyzer/releases/download/2026-09-21/rust-analyzer-x86_64-unknown-linux-gnu.gz",
                sha256: "b2d24ce2bda2ea05b1ad7c2917d205f8111775f703ccd908e6061803ae8257d0",
            },
        ],
        unpack: Unpack::Gunzip,
        program: "rust-analyzer",
    },
};

/// The Node.js that runs every npm server, so the user needs none. The checksums are the ones in
/// Node's own `SHASUMS256.txt`.
pub const NODE: Download = Download {
    version: "v24.21.0",
    kind: Kind::Platform {
        files: &[
            PlatformFile {
                platform: "aarch64-apple-darwin",
                url: "https://nodejs.org/dist/v24.21.0/node-v24.21.0-darwin-arm64.tar.gz",
                sha256: "bed7eea5325e1108f32ce5228ddd6a5f0f08a499ee42aa7442aea583702f6057",
            },
            PlatformFile {
                platform: "x86_64-apple-darwin",
                url: "https://nodejs.org/dist/v24.21.0/node-v24.21.0-darwin-x64.tar.gz",
                sha256: "1462cb3b3046b815cf8ea436d3da450ec1a9f11dac7e5a46b0ada5305d7e8097",
            },
            PlatformFile {
                platform: "aarch64-unknown-linux-gnu",
                url: "https://nodejs.org/dist/v24.21.0/node-v24.21.0-linux-arm64.tar.gz",
                sha256: "724282c3b43aec998aa9527380465b45d229e021b58035f5f4f63095eabfe5d5",
            },
            PlatformFile {
                platform: "x86_64-unknown-linux-gnu",
                url: "https://nodejs.org/dist/v24.21.0/node-v24.21.0-linux-x64.tar.gz",
                sha256: "6e1db87ef58b8819e5d5402eff1536491b18edd8eb7bee5ef7897876e88dc5ff",
            },
        ],
        unpack: Unpack::TarGz,
        program: "bin/node",
    },
};

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

/// The standard LSP language id of each language lathe knows, and the file extensions that are in it.
pub const LANGUAGES: &[(&str, &[&str])] = &[
    ("rust", &["rs"]),
    ("typescript", &["ts", "mts", "cts"]),
    ("typescriptreact", &["tsx"]),
    ("javascript", &["js", "mjs", "cjs"]),
    ("javascriptreact", &["jsx"]),
    ("python", &["py", "pyi"]),
    ("go", &["go"]),
    ("java", &["java"]),
];

/// The standard LSP language id for a file, by its extension.
pub fn language_id(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?;
    LANGUAGES.iter().find(|(_, extensions)| extensions.contains(&extension)).map(|(id, _)| *id)
}

/// The server for a language id, if lathe knows one.
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

#[cfg(test)]
mod tests;
