//! Which language server serves which file. This is the only place that names a language or a
//! server: everything else takes a [`ServerSpec`] and works the same for all of them. Adding a
//! language is adding a row to [`LANGUAGES`] and, if no server already covers it, one to [`SERVERS`].

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::install::{Download, Kind, PlatformFile, Unpack};

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
    /// How to install it, for the status line when it is missing and atelier cannot download it.
    pub install: &'static str,
    /// The pinned copy atelier downloads when the user has none.
    pub download: Option<Download>,
    /// The `initializationOptions` to send, from where the program is and the project root.
    pub initialization_options: fn(program: &Path, root: &Path) -> Option<Value>,
}

/// For a server that needs no options.
fn no_options(_: &Path, _: &Path) -> Option<Value> {
    None
}

/// The servers atelier knows how to run.
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
        name: "tsgo",
        // TypeScript 7's compiler is native Go and serves LSP itself, so there is no Node.js and no
        // tsserver. The binary is `tsc`, but a `tsc` on the PATH is most often TypeScript 5, which has
        // no `--lsp`; `tsgo` is the name the native preview installed it under.
        program: "tsgo",
        args: &["--lsp", "--stdio"],
        language_ids: &["typescript", "typescriptreact", "javascript", "javascriptreact"],
        root_markers: &["tsconfig.json", "jsconfig.json", "package.json"],
        install: "npm install -g @typescript/native-preview",
        download: Some(TSGO),
        initialization_options: no_options,
    },
    ServerSpec {
        name: "ty",
        program: "ty",
        args: &["server"],
        language_ids: &["python"],
        root_markers: &["pyproject.toml", "ty.toml", "setup.py", "requirements.txt"],
        install: "uv tool install ty",
        download: Some(TY),
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
        // jdtls needs a Java runtime as well, which atelier does not download yet.
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

/// TypeScript 7's native compiler, from its per-platform npm packages. The tarball's top folder is
/// `package`, which unpacking drops, so the binary lands at `lib/tsc` beside the `lib.*.d.ts` it reads.
const TSGO: Download = Download {
    version: "7.0.2",
    kind: Kind::Platform {
        files: &[
            PlatformFile {
                platform: "aarch64-apple-darwin",
                url: "https://registry.npmjs.org/@typescript/typescript-darwin-arm64/-/typescript-darwin-arm64-7.0.2.tgz",
                sha256: "902e2fe1cf0799198ef902c6b8c310a450fef629a6baba41d45641ef75c04ebd",
            },
            PlatformFile {
                platform: "x86_64-apple-darwin",
                url: "https://registry.npmjs.org/@typescript/typescript-darwin-x64/-/typescript-darwin-x64-7.0.2.tgz",
                sha256: "eba158cb54050f723d5ff781438f33de5640054440bb4f2bd170cfe9bc2eb551",
            },
            PlatformFile {
                platform: "aarch64-unknown-linux-gnu",
                url: "https://registry.npmjs.org/@typescript/typescript-linux-arm64/-/typescript-linux-arm64-7.0.2.tgz",
                sha256: "c83d931ac9dd7549cde6e71246aa9d6a9812843023df3e277fe3b5dcf41dd0ea",
            },
            PlatformFile {
                platform: "x86_64-unknown-linux-gnu",
                url: "https://registry.npmjs.org/@typescript/typescript-linux-x64/-/typescript-linux-x64-7.0.2.tgz",
                sha256: "7ecad6f67377e831856367ab062ef394f21506a611405bf8ac0ff039348637d3",
            },
        ],
        unpack: Unpack::TarGz,
        program: "lib/tsc",
    },
};

/// Astral's ty, a Python type checker in Rust. The checksums match the release's own `sha256.sum`.
const TY: Download = Download {
    version: "0.0.84",
    kind: Kind::Platform {
        files: &[
            PlatformFile {
                platform: "aarch64-apple-darwin",
                url: "https://github.com/astral-sh/ty/releases/download/0.0.84/ty-aarch64-apple-darwin.tar.gz",
                sha256: "c65c09f27bcef726c0b043dcee8d0f1e578bd1936799e5bc447235cbc3e19d91",
            },
            PlatformFile {
                platform: "x86_64-apple-darwin",
                url: "https://github.com/astral-sh/ty/releases/download/0.0.84/ty-x86_64-apple-darwin.tar.gz",
                sha256: "3891e5509d306721cee4dd4e69b94535dbd96371af7ec3b734ee65f25b167ae4",
            },
            PlatformFile {
                platform: "aarch64-unknown-linux-gnu",
                url: "https://github.com/astral-sh/ty/releases/download/0.0.84/ty-aarch64-unknown-linux-gnu.tar.gz",
                sha256: "d575243e0586742ae0e9186441358bd7e57e8160e319b3afb781430126762a39",
            },
            PlatformFile {
                platform: "x86_64-unknown-linux-gnu",
                url: "https://github.com/astral-sh/ty/releases/download/0.0.84/ty-x86_64-unknown-linux-gnu.tar.gz",
                sha256: "336bb36b7e917d844b8b16925d373b4614b326e452c881ff4ed6bc5904b65185",
            },
        ],
        unpack: Unpack::TarGz,
        program: "ty",
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

/// The standard LSP language id of each language atelier knows, and the file extensions that are in it.
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

#[cfg(test)]
mod tests;
