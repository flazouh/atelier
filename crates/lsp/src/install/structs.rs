use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, atomic::{AtomicU64, Ordering}},
};

use crate::{
    ServerSpec,
    servers::{NODE, find_program_in, search_dirs},
};
use super::types::{Kind, Unavailable, Unpack};
use super::helpers::{
    default_dir, failed, fetch_checked, gunzip, platform, remove, run, settle, untar,
};

/// What to fetch for one server, at one version.
#[derive(Debug)]
pub struct Download {
    pub version: &'static str,
    pub kind: Kind,
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

/// atelier's folder of downloaded servers, and where it looks for ones the user installed.
pub struct Store {
    pub(super) dir: PathBuf,
    pub(super) search: Vec<PathBuf>,
    pub(super) offline: bool,
    /// The servers live on another host, whose `PATH` finds them; nothing is looked up or
    /// downloaded here.
    pub(super) on_host: bool,
    /// One download at a time, so two files never fetch the same server twice.
    pub(super) downloading: Mutex<()>,
}

impl Store {
    /// The app's folder, the usual search path, and `ATELIER_OFFLINE`.
    pub fn from_env() -> Self {
        let offline = std::env::var("ATELIER_OFFLINE").is_ok_and(|v| !v.is_empty() && v != "0");
        Self::new(default_dir(), search_dirs(), offline)
    }

    pub fn new(dir: PathBuf, search: Vec<PathBuf>, offline: bool) -> Self {
        Self { dir, search, offline, on_host: false, downloading: Mutex::new(()) }
    }

    /// For a project on another host: each server by its program's name, for the host's `PATH` to
    /// find when the project starts it there. A server the host does not have fails to start, and
    /// says so; atelier downloads nothing onto a host.
    pub fn on_host() -> Self {
        Self { on_host: true, ..Self::new(PathBuf::new(), Vec::new(), true) }
    }

    /// How to run `spec`: the copy the user installed, else atelier's own, downloaded now if need be.
    /// It can block for a download, so call it off the UI thread. `report` hears each download start.
    pub fn launch(&self, spec: &ServerSpec, report: &dyn Fn(String)) -> Result<Launch, Unavailable> {
        if self.on_host {
            return Ok(Launch::direct(PathBuf::from(spec.program)));
        }
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
    pub(super) fn ensure(&self, name: &str, download: &Download, report: &dyn Fn(String)) -> Result<PathBuf, Unavailable> {
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
    pub(super) fn fetch(&self, kind: &Kind, version: &str, into: &Path) -> Result<(), Unavailable> {
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
