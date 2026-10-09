use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, Stdio},
    sync::{Arc, Mutex},
};

use atelier_project::Tail;

use crate::client::{Connection, Dial, RemoteProject, Timeouts};
use super::structs::Platform;
use super::types::VERSION;

/// Where the host keeps this build's copy, from its home folder: `hash` is the first twelve hex
/// digits of the copy's SHA-256.
pub fn remote_binary(version: &str, hash: &str) -> String {
    format!(".cache/atelier/remote/{version}-{hash}/atelier-remote")
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
pub(super) fn ssh(host: &str) -> std::process::Command {
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
pub(super) fn run(host: &str, script: &str, stdin: Option<&[u8]>) -> io::Result<String> {
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

/// Where a copy of atelier-remote for `platform` may be, in the order to look: the folder a developer
/// names (`env_dir`), next to the app `exe` by platform, in a Mac bundle's resources, the plain copy
/// beside the app when the host is of the app's own kind (`same`), and the folder
/// `tools/build-remote.sh` fills under `home`.
pub fn candidates(exe: &std::path::Path, platform: &Platform, env_dir: Option<&std::path::Path>, home: Option<&std::path::Path>, same: bool) -> Vec<PathBuf> {
    let by_platform = |dir: PathBuf| dir.join(platform.name()).join("atelier-remote");
    let mut found = Vec::new();
    found.extend(env_dir.map(|d| by_platform(d.to_path_buf())));
    if let Some(dir) = exe.parent() {
        found.push(by_platform(dir.join("remote")));
        // A Mac app runs from Contents/MacOS, and keeps what it ships in Contents/Resources.
        if dir.file_name().is_some_and(|n| n == "MacOS") {
            found.extend(dir.parent().map(|contents| by_platform(contents.join("Resources").join("remote"))));
        }
        if same {
            found.push(dir.join("atelier-remote"));
        }
    }
    found.extend(home.map(|h| by_platform(h.join(".cache/atelier/remote-builds"))));
    found
}

/// Where a copy for `platform` may be, for this app and this environment.
fn places(platform: &Platform) -> Vec<PathBuf> {
    let Ok(exe) = std::env::current_exe() else { return Vec::new() };
    let env_dir = std::env::var_os("ATELIER_REMOTE_DIR").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    candidates(&exe, platform, env_dir.as_deref(), home.as_deref(), *platform == Platform::here())
}
/// The copy of atelier-remote to upload to a host of `platform`, if there is one.
pub fn local_binary(platform: &Platform) -> Option<PathBuf> {
    first_matching(&places(platform))
}
/// The first copy among `candidates` that is a file, and the protocol it speaks (none for a copy with no stamp): what is
/// found when none matches, which is not the same as finding nothing.
pub fn first_found(candidates: &[PathBuf]) -> Option<(PathBuf, Option<u32>)> {
    candidates.iter().find_map(|p| std::fs::read(p).ok().map(|bytes| (p.clone(), speaks(&bytes))))
}

/// The protocol a helper binary's bytes say it speaks, from its stamp; `None` for a copy with none, as
/// helpers built before the stamp are.
pub fn speaks(bytes: &[u8]) -> Option<u32> {
    const MARK: &[u8] = b"atelier-remote-protocol:";
    let at = bytes.windows(MARK.len()).position(|w| w == MARK)? + MARK.len();
    let digits: Vec<u8> = bytes[at..].iter().take_while(|b| b.is_ascii_digit()).copied().collect();
    let end = bytes.get(at + digits.len())?;
    (*end == b';').then(|| std::str::from_utf8(&digits).ok()?.parse().ok()).flatten()
}

/// The first of `candidates` that is a file and speaks this protocol: an old copy is passed over.
pub fn first_matching(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| std::fs::read(p).is_ok_and(|bytes| speaks(&bytes) == Some(crate::protocol::VERSION))).cloned()
}

/// What a helper answers to `--version`: its build and its protocol. The copy on a host is used only
/// when it answers this, else the matching one goes up in its place.
pub fn version_line() -> String {
    format!("{VERSION} protocol {}", crate::protocol::VERSION)
}

/// Whether the `--version` line of a helper names this app's protocol, whatever version text comes before it. The
/// protocol is the contract between the app and the helper: a helper built from an older checkout speaks it as well as a
/// new one, and its path already carries the hash of its bytes.
pub fn speaks_this_protocol(line: &str) -> bool {
    line.trim().rsplit_once(" protocol ").is_some_and(|(_, n)| n == crate::protocol::VERSION.to_string())
}

/// What the reader reads when no copy for the host is found: what is missing, and what to do.
pub fn missing_words(host: &str, platform: &Platform) -> String {
    format!(
        "{host} is a {} machine, and this copy of atelier has no helper built for it, so it cannot open folders there. \
         Install a atelier build that includes the {} helper, or, from a atelier checkout, build one with tools/build-remote.sh on a {} machine.",
        platform.describe(),
        platform.name(),
        platform.describe()
    )
}

/// What the reader reads when a copy for the host is found but speaks another protocol, as every copy does after the app
/// moves on to a new one: which copy, what it speaks, what is needed, and how to make one.
pub fn outdated_words(host: &str, platform: &Platform, path: &std::path::Path, found: Option<u32>) -> String {
    let speaks = found.map_or_else(|| "no protocol it can name".to_string(), |n| format!("protocol {n}"));
    format!(
        "{host} is a {} machine. The helper this app found for it, {}, speaks {speaks}, and this build needs protocol {}. \
         Build a new one with tools/build-remote.sh from a atelier checkout on a {} machine, and put it in {}.",
        platform.describe(),
        path.display(),
        crate::protocol::VERSION,
        platform.describe(),
        path.parent().map_or_else(String::new, |p| p.display().to_string()),
    )
}
/// Makes sure the host has this version's atelier-remote, uploading it when not, and returns its path
/// from the host's home.
pub fn deploy(host: &str, platform: &Platform, say: &dyn Fn(String)) -> io::Result<String> {
    let local = local_binary(platform).ok_or_else(|| {
        let words = match first_found(&places(platform)) {
            Some((path, found)) => outdated_words(host, platform, &path, found),
            None => missing_words(host, platform),
        };
        io::Error::new(io::ErrorKind::NotFound, words)
    })?;
    let bytes = std::fs::read(&local)?;
    let path = remote_binary(VERSION, &short_hash(&bytes));
    // A copy that answers another protocol is replaced by the one that matches, with nothing to see.
    let has = run(host, &format!("test -x {path} && {path} --version"), None).map(|v| speaks_this_protocol(&v)).unwrap_or(false);
    if has {
        return Ok(path);
    }
    say(format!("Putting atelier-remote on {host}…"));
    run(host, &upload_command(&path), Some(&bytes))?;
    let version = run(host, &format!("{path} --version"), None)?;
    if !speaks_this_protocol(&version) {
        return Err(io::Error::other(format!("{host}: the helper atelier put there does not start as it should")));
    }
    Ok(path)
}

/// The command that puts the bytes on its stdin at `path`, run by the host's shell. The copy is written under
/// a name of its own (`$$` is the shell's pid), so two connections to one host never write the same file, and
/// it is moved into place whole.
pub(super) fn upload_command(path: &str) -> String {
    let dir = path.rsplit_once('/').map_or(".", |(d, _)| d);
    format!("mkdir -p {dir} && cat > {path}.part.$$ && chmod +x {path}.part.$$ && mv {path}.part.$$ {path}")
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

/// Opens the folder `root` on `host` as a project: probes the host, puts atelier-remote there if it
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
