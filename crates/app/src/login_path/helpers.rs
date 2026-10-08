use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Wraps the answer, so what a shell prints on its own (a banner, a plugin's notice) is not taken for the `PATH`.
const MARK: &str = "__atelier_path__";
/// How long the shell has to answer.
const WAIT: Duration = Duration::from_secs(3);
/// Where installers put programs, under the home folder and not: used when they exist and the shell did not name them.
const HOME_DIRS: [&str; 3] = [".local/bin", ".cargo/bin", ".bun/bin"];
const SYSTEM_DIRS: [&str; 3] = ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"];

/// The `PATH` to run with: the login shell's folders in its order, then the ones already set, then the installers'
/// folders that exist. No folder is named twice and no name is empty.
pub fn merge(login: Option<&str>, current: &str, home: &Path, is_dir: impl Fn(&Path) -> bool) -> String {
    let mut dirs: Vec<String> = Vec::new();
    let named = login.into_iter().chain([current]).flat_map(|path| path.split(':'));
    let installers = HOME_DIRS
        .iter()
        .map(|dir| home.join(dir))
        .chain(SYSTEM_DIRS.iter().map(Path::new).map(Path::to_path_buf))
        .filter(|dir| is_dir(dir))
        .map(|dir| dir.to_string_lossy().into_owned());
    for dir in named.map(str::to_string).chain(installers) {
        if !dir.is_empty() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs.join(":")
}

/// The `PATH` inside the marks of what a shell printed.
pub(super) fn marked(output: &str) -> Option<&str> {
    let (_, after) = output.split_once(MARK)?;
    let (path, _) = after.split_once(MARK)?;
    (!path.is_empty()).then_some(path)
}

/// What `shell` says its `PATH` is, if it says so within `wait`. It runs as a login shell and an interactive one, so it
/// reads the files where installers add their folders.
pub(super) fn ask(shell: &str, wait: Duration) -> Option<String> {
    let script = format!("printf '%s' \"{MARK}$PATH{MARK}\"");
    let mut child = Command::new(shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(status) if status.success() => break,
            Some(_) => return None,
            None if started.elapsed() >= wait => {
                drop(child.kill());
                drop(child.wait());
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(15)),
        }
    }
    let mut output = String::new();
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    marked(&output).map(str::to_string)
}

/// Gives this process the `PATH` the reader means. Call it first in `main`, before any thread starts.
pub fn adopt() {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let current = std::env::var("PATH").unwrap_or_default();
    let path = merge(ask(&shell, WAIT).as_deref(), &current, &home, Path::is_dir);
    // SAFETY: called at the start of `main`, before the app starts a thread.
    unsafe { std::env::set_var("PATH", path) };
}
