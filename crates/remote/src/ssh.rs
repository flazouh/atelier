//! Reaching a host over the user's own `ssh`: their config, their keys, their agent. lathe asks for
//! no credentials; `BatchMode` makes a host that wants a password fail at once and say so, instead
//! of waiting on a prompt nobody sees.
//!
//! Connecting is three steps:
//!
//! 1. Probe: `uname -sm` names the host's system and architecture, and `$HOME` its home.
//! 2. Deploy: `~/.cache/lathe/remote/<version>-<hash>/lathe-remote` must answer `--version` with
//!    this app's version; the hash is of the copy this app would upload, so a new build of the same
//!    version goes up once instead of an old one staying. If the host has not got it, the copy built
//!    for that platform goes up over the same `ssh`
//!    (`cat` into a temporary file, `chmod +x`, then a rename, so a half copy never runs). This needs
//!    no `scp` on either side.
//! 3. Dial: `ssh <host> <that path> --stdio`, whose stdin and stdout carry the frames.
//!
//! The copy to upload comes from `$LATHE_REMOTE_DIR/<system>-<architecture>/lathe-remote`, or, for a
//! host like this machine, the `lathe-remote` beside the app. In development, build it on a machine
//! of the host's kind (the HP builds linux-x86_64) and point `LATHE_REMOTE_DIR` at a folder of them.

use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, Stdio},
    sync::{Arc, Mutex},
};

use lathe_project::Tail;

use crate::client::{Connection, Dial, RemoteProject, Timeouts};

/// This app's version: the host's copy must say the same.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The host's system and architecture, as lathe names builds: `linux-x86_64`, `darwin-aarch64`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Platform {
    pub system: String,
    pub arch: String,
}

impl Platform {
    /// From `uname -sm`: `Linux x86_64`, `Darwin arm64`.
    pub fn from_uname(line: &str) -> Option<Self> {
        let mut parts = line.split_whitespace();
        let system = parts.next()?.to_lowercase();
        let arch = match parts.next()? {
            "arm64" => "aarch64".to_string(),
            "amd64" => "x86_64".to_string(),
            other => other.to_string(),
        };
        Some(Self { system, arch })
    }

    /// This machine's.
    pub fn here() -> Self {
        let system = match std::env::consts::OS {
            "macos" => "darwin",
            other => other,
        };
        Self { system: system.to_string(), arch: std::env::consts::ARCH.to_string() }
    }

    pub fn name(&self) -> String {
        format!("{}-{}", self.system, self.arch)
    }
}

/// Where the host keeps this build's copy, from its home folder: `hash` is the first twelve hex
/// digits of the copy's SHA-256.
pub fn remote_binary(version: &str, hash: &str) -> String {
    format!(".cache/lathe/remote/{version}-{hash}/lathe-remote")
}

/// The first twelve hex digits of `bytes`' SHA-256.
pub fn short_hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// The names `Host` lines give in an ssh config, leaving out patterns (`*`, `?`, `!`).
pub fn hosts_in_config(config: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    for line in config.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(char::is_whitespace) else { continue };
        if !key.eq_ignore_ascii_case("host") {
            continue;
        }
        for name in rest.split_whitespace() {
            if !name.contains(['*', '?', '!']) && !hosts.iter().any(|h| h == name) {
                hosts.push(name.to_string());
            }
        }
    }
    hosts
}

/// The hosts in the user's `~/.ssh/config`, for "Open over SSH…".
pub fn known_hosts() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME") else { return Vec::new() };
    std::fs::read_to_string(PathBuf::from(home).join(".ssh/config")).map(|c| hosts_in_config(&c)).unwrap_or_default()
}

/// `ssh` for `host`, never asking for a password, and giving up on a dead link in under a minute.
fn ssh(host: &str) -> std::process::Command {
    let mut ssh = std::process::Command::new("ssh");
    ssh.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ServerAliveInterval=15",
        "-o",
        "ServerAliveCountMax=3",
        "--",
        host,
    ]);
    ssh
}

/// Runs `script` on `host` and returns its stdout; a failure says what ssh or the host said.
fn run(host: &str, script: &str, stdin: Option<&[u8]>) -> io::Result<String> {
    let mut child = ssh(host)
        .arg(script)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| io::Error::new(e.kind(), format!("ssh did not start: {e}")))?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(bytes)?;
    }
    let out = child.wait_with_output()?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    let said = said.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("no message").trim().to_string();
    let kind = if said.contains("Permission denied") { io::ErrorKind::PermissionDenied } else { io::ErrorKind::ConnectionRefused };
    // ssh often names the host itself ("ssh: Could not resolve hostname x"); then it is not said twice.
    let said = if said.contains(host) { said } else { format!("{host}: {said}") };
    Err(io::Error::new(kind, said))
}

/// The host's platform and home folder.
pub fn probe(host: &str) -> io::Result<(Platform, String)> {
    let out = run(host, "uname -sm && printf '%s\\n' \"$HOME\"", None)?;
    let mut lines = out.lines();
    let platform = lines.next().and_then(Platform::from_uname).ok_or_else(|| io::Error::other(format!("{host}: uname said {out:?}")))?;
    let home = lines.next().unwrap_or("").to_string();
    Ok((platform, home))
}

/// The copy of lathe-remote to upload to a host of `platform`, if there is one.
pub fn local_binary(platform: &Platform) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LATHE_REMOTE_DIR") {
        let path = PathBuf::from(dir).join(platform.name()).join("lathe-remote");
        if path.is_file() {
            return Some(path);
        }
    }
    let beside = std::env::current_exe().ok()?.parent()?.join("lathe-remote");
    (*platform == Platform::here() && beside.is_file()).then_some(beside)
}

/// Makes sure the host has this version's lathe-remote, uploading it when not, and returns its path
/// from the host's home.
pub fn deploy(host: &str, platform: &Platform, say: &dyn Fn(String)) -> io::Result<String> {
    let local = local_binary(platform).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no lathe-remote {VERSION} built for {} to put on {host}; set LATHE_REMOTE_DIR", platform.name()),
        )
    })?;
    let bytes = std::fs::read(&local)?;
    let path = remote_binary(VERSION, &short_hash(&bytes));
    let has = run(host, &format!("test -x {path} && {path} --version"), None).map(|v| v.trim() == VERSION).unwrap_or(false);
    if has {
        return Ok(path);
    }
    say(format!("Putting lathe-remote on {host}…"));
    let dir = path.rsplit_once('/').map_or(".", |(d, _)| d);
    run(host, &format!("mkdir -p {dir} && cat > {path}.part && chmod +x {path}.part && mv {path}.part {path}"), Some(&bytes))?;
    let version = run(host, &format!("{path} --version"), None)?;
    if version.trim() != VERSION {
        return Err(io::Error::other(format!("{host}: the uploaded lathe-remote says {:?}", version.trim())));
    }
    Ok(path)
}

/// Dials `ssh <host> <binary> --stdio`, once for each connection.
pub fn dial(host: &str, binary: &str) -> Dial {
    let (host, binary) = (host.to_string(), binary.to_string());
    Box::new(move || {
        let mut child: Child = ssh(&host)
            .arg(format!("{binary} --stdio"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let writer = child.stdin.take().expect("stdin is piped");
        let reader = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().map(Tail::follow).unwrap_or_default();
        let child = Arc::new(Mutex::new(child));
        Ok(Connection {
            reader: Box::new(reader) as Box<dyn Read + Send>,
            writer: Box::new(writer),
            close: Box::new(move || {
                let mut child = child.lock().unwrap_or_else(|p| p.into_inner());
                let _ = child.kill();
                let _ = child.wait();
            }),
            last_words: Box::new(move || stderr.text()),
        })
    })
}

/// Opens the folder `root` on `host` as a project: probes the host, puts lathe-remote there if it
/// has not got this version, and connects. `say` hears each step, for the app to show.
pub fn connect(host: &str, root: &str, say: &dyn Fn(String)) -> io::Result<RemoteProject> {
    say(format!("Reaching {host}…"));
    let (platform, _home) = probe(host)?;
    let binary = deploy(host, &platform, say)?;
    say(format!("Opening {root} on {host}…"));
    RemoteProject::connect(host, root, dial(host, &binary), Timeouts::default())
}

/// Like [`connect`], with the host's home folder as the root, so the app can browse the host's folders before the reader
/// has chosen one. It returns the project and the home folder's path.
pub fn connect_at_home(host: &str, say: &dyn Fn(String)) -> io::Result<(RemoteProject, String)> {
    say(format!("Reaching {host}…"));
    let (platform, home) = probe(host)?;
    if home.is_empty() {
        return Err(io::Error::other(format!("{host} did not say where its home folder is")));
    }
    let binary = deploy(host, &platform, say)?;
    say(format!("Opening {host}…"));
    let project = RemoteProject::connect(host, &home, dial(host, &binary), Timeouts::default())?;
    Ok((project, home))
}

#[cfg(test)]
mod tests;
