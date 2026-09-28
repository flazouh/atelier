//! Servers that come with the app. A user installs lathe and nothing else: when a file's server is
//! missing, lathe downloads a pinned copy once and runs that copy from then on. The order is the
//! server the user installed, then lathe's own copy, then a download. See `docs/code-editor.md`,
//! Stage E.
//!
//! Every file is pinned by version and SHA-256 in the registry. A download is unpacked in a staging
//! folder and renamed into place only once it checked out, so a folder that exists is complete.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use sha2::{Digest, Sha256};

use crate::{
    ServerSpec,
    servers::{NODE, find_program_in, search_dirs},
};

/// What to fetch for one server, at one version.
#[derive(Debug)]
pub struct Download {
    pub version: &'static str,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    /// One archive per platform. `program` is the executable in it, relative to the server's folder.
    Platform { files: &'static [PlatformFile], unpack: Unpack, program: &'static str },
    /// npm tarballs unpacked side by side into `node_modules`, run with lathe's Node.js. `script` is
    /// the server's entry, relative to the server's folder.
    Node { packages: &'static [Package], script: &'static str },
    /// A Go module built by the user's Go into the server's folder, as the server's program. Go
    /// serves Go projects only, and a Go project has Go.
    Go { module: &'static str },
}

/// A file built for one platform, named by its Rust target triple.
#[derive(Debug)]
pub struct PlatformFile {
    pub platform: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
}

/// One npm package's tarball.
#[derive(Debug)]
pub struct Package {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
}

/// How a platform file becomes a folder.
#[derive(Clone, Copy, Debug)]
pub enum Unpack {
    /// A gzipped executable, written to `program`.
    Gunzip,
    /// A gzipped tarball with one top folder, which is dropped.
    TarGz,
}

/// This machine's Rust target triple, if lathe downloads servers for it.
pub fn platform() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => Some("aarch64-apple-darwin"),
        ("x86_64", "macos") => Some("x86_64-apple-darwin"),
        ("aarch64", "linux") => Some("aarch64-unknown-linux-gnu"),
        ("x86_64", "linux") => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

/// How to run a server: its program, and the script that program runs when the server is a Node one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    pub script: Option<PathBuf>,
}

impl Launch {
    /// A server that is its own program.
    pub fn direct(program: PathBuf) -> Self {
        Self { program, script: None }
    }

    /// The arguments to start `spec` with: the script, if any, then the server's own.
    pub fn args(&self, spec: &ServerSpec) -> Vec<String> {
        let script = self.script.iter().map(|s| s.to_string_lossy().into_owned());
        script.chain(spec.args.iter().map(|a| a.to_string())).collect()
    }

    /// The server's own file, which its options can look beside.
    pub fn server_file(&self) -> &Path {
        self.script.as_deref().unwrap_or(&self.program)
    }
}

/// Why a server cannot run here.
#[derive(Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// Nothing to run, and nothing lathe may download: no pin for it or this platform, downloads are
    /// off, or a tool it is built with is missing.
    NotInstalled,
    /// A download was tried and failed, for this reason.
    Failed(String),
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "not installed"),
            Self::Failed(reason) => write!(f, "{reason}"),
        }
    }
}

/// lathe's folder of downloaded servers, and where it looks for ones the user installed.
pub struct Store {
    dir: PathBuf,
    search: Vec<PathBuf>,
    offline: bool,
    /// One download at a time, so two files never fetch the same server twice.
    downloading: Mutex<()>,
}

impl Store {
    /// The app's folder, the usual search path, and `LATHE_OFFLINE`.
    pub fn from_env() -> Self {
        let offline = std::env::var("LATHE_OFFLINE").is_ok_and(|v| !v.is_empty() && v != "0");
        Self::new(default_dir(), search_dirs(), offline)
    }

    pub fn new(dir: PathBuf, search: Vec<PathBuf>, offline: bool) -> Self {
        Self { dir, search, offline, downloading: Mutex::new(()) }
    }

    /// How to run `spec`: the copy the user installed, else lathe's own, downloaded now if need be.
    /// It can block for a download, so call it off the UI thread. `report` hears each download start.
    pub fn launch(&self, spec: &ServerSpec, report: &dyn Fn(String)) -> Result<Launch, Unavailable> {
        if let Some(program) = find_program_in(spec.program, &self.search) {
            return Ok(Launch::direct(program));
        }
        let download = spec.download.as_ref().ok_or(Unavailable::NotInstalled)?;
        let folder = self.ensure(spec.name, download, report)?;
        match download.kind {
            Kind::Platform { program, .. } => Ok(Launch::direct(folder.join(program))),
            Kind::Go { .. } => Ok(Launch::direct(folder.join(spec.program))),
            Kind::Node { script, .. } => {
                let Kind::Platform { program: node, .. } = NODE.kind else { unreachable!("Node.js is a platform file") };
                let runtime = self.ensure("node", &NODE, report)?;
                Ok(Launch { program: runtime.join(node), script: Some(folder.join(script)) })
            }
        }
    }

    /// The folder holding `name` at `download`'s version, downloaded first if it is not there.
    fn ensure(&self, name: &str, download: &Download, report: &dyn Fn(String)) -> Result<PathBuf, Unavailable> {
        let folder = self.dir.join(name).join(download.version);
        if folder.exists() {
            return Ok(folder);
        }
        if self.offline {
            return Err(Unavailable::NotInstalled);
        }
        let _one = self.downloading.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // Another thread may have finished it while this one waited.
        if folder.exists() {
            return Ok(folder);
        }
        report(format!("downloading {name} {}", download.version));
        // Unique per process and per download, since several stores can share one folder.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let staging = self.dir.join(format!(".{name}-{}-{unique}", download.version));
        _ = fs::remove_dir_all(&staging);
        let fetched = fs::create_dir_all(&staging)
            .map_err(|e| Unavailable::Failed(format!("cannot write {}: {e}", staging.display())))
            .and_then(|()| self.fetch(&download.kind, download.version, &staging))
            .and_then(|()| settle(&staging, &folder));
        if fetched.is_err() {
            _ = fs::remove_dir_all(&staging);
        }
        fetched.map(|()| folder)
    }

    /// Fills `into` with the server.
    fn fetch(&self, kind: &Kind, version: &str, into: &Path) -> Result<(), Unavailable> {
        match kind {
            Kind::Platform { files, unpack, program } => {
                let here = platform().ok_or(Unavailable::NotInstalled)?;
                let file = files.iter().find(|f| f.platform == here).ok_or(Unavailable::NotInstalled)?;
                let archive = into.join(".download");
                fetch_checked(file.url, file.sha256, &archive)?;
                match unpack {
                    Unpack::Gunzip => gunzip(&archive, &into.join(program))?,
                    Unpack::TarGz => untar(&archive, into)?,
                }
                remove(&archive)
            }
            Kind::Node { packages, .. } => packages.iter().try_for_each(|package| {
                let archive = into.join(".download");
                fetch_checked(package.url, package.sha256, &archive)?;
                let folder = into.join("node_modules").join(package.name);
                fs::create_dir_all(&folder).map_err(|e| failed("cannot write", &folder, e))?;
                untar(&archive, &folder)?;
                remove(&archive)
            }),
            Kind::Go { module } => {
                let go = find_program_in("go", &self.search).ok_or(Unavailable::NotInstalled)?;
                let mut install = Command::new(go);
                install.arg("install").arg(format!("{module}@{version}")).env("GOBIN", into);
                run(&mut install, "go install")
            }
        }
    }
}

/// Where lathe keeps its servers: `LATHE_SERVERS_DIR`, else the platform's app data folder.
fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LATHE_SERVERS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let data = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"))
    };
    data.join("lathe/servers")
}

/// Moves a finished staging folder into place. When another lathe got there first, its copy is as
/// good as this one, so this one is dropped.
fn settle(staging: &Path, folder: &Path) -> Result<(), Unavailable> {
    let parent = folder.parent().expect("a server folder has a parent");
    fs::create_dir_all(parent).map_err(|e| failed("cannot write", parent, e))?;
    match fs::rename(staging, folder) {
        Ok(()) => Ok(()),
        Err(_) if folder.exists() => {
            _ = fs::remove_dir_all(staging);
            Ok(())
        }
        Err(e) => Err(failed("cannot move into", folder, e)),
    }
}

/// Downloads `url` to `to` and keeps it only if its SHA-256 is `sha256`.
fn fetch_checked(url: &str, sha256: &str, to: &Path) -> Result<(), Unavailable> {
    let mut curl = Command::new("curl");
    curl.args(["--fail", "--silent", "--show-error", "--location", "--retry", "2", "--output"]).arg(to).arg(url);
    run(&mut curl, &format!("downloading {url}"))?;
    let actual = sha256_of(to).map_err(|e| failed("cannot read", to, e))?;
    if actual != sha256 {
        _ = fs::remove_file(to);
        return Err(Unavailable::Failed(format!("{url} is not the pinned file: its SHA-256 is {actual}")));
    }
    Ok(())
}

/// The file's SHA-256, in lower-case hex.
pub fn sha256_of(path: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut fs::File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Unzips a gzipped executable to `to` and lets it run.
fn gunzip(archive: &Path, to: &Path) -> Result<(), Unavailable> {
    let out = fs::File::create(to).map_err(|e| failed("cannot write", to, e))?;
    run(Command::new("gzip").arg("-dc").arg(archive).stdout(out), "gzip")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(to, fs::Permissions::from_mode(0o755)).map_err(|e| failed("cannot make runnable", to, e))?;
    }
    Ok(())
}

/// Unpacks a gzipped tarball into `into`, dropping its top folder.
fn untar(archive: &Path, into: &Path) -> Result<(), Unavailable> {
    run(Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(into).arg("--strip-components=1"), "tar")
}

fn remove(file: &Path) -> Result<(), Unavailable> {
    fs::remove_file(file).map_err(|e| failed("cannot remove", file, e))
}

/// Runs a tool to the end; a failure carries what it printed.
fn run(command: &mut Command, what: &str) -> Result<(), Unavailable> {
    let output = command.stdin(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).output();
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let said = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(Unavailable::Failed(format!("{what} failed: {said}")))
        }
        Err(e) => Err(Unavailable::Failed(format!("{what} did not run: {e}"))),
    }
}

fn failed(what: &str, path: &Path, error: io::Error) -> Unavailable {
    Unavailable::Failed(format!("{what} {}: {error}", path.display()))
}

#[cfg(test)]
mod tests;
