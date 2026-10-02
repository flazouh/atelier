use std::{io, thread};

use super::types::TETHER;

/// Ends the child `pid`, the leader of its own process group, and its group when this app ends, however it
/// ends. Closing a child's stdin is not enough: some agents keep running after it, and a crash or a kill
/// gives the app no time to stop them. The watchdog runs detached; the `sh` that starts it ends at once, and
/// a thread of its own waits for it, so a spawn does not wait and leaves no zombie.
#[cfg(unix)]
pub fn tether(pid: u32) -> io::Result<()> {
    tether_with("sh", pid)
}

#[cfg(unix)]
pub(super) fn tether_with(shell: &str, pid: u32) -> io::Result<()> {
    let mut starter = std::process::Command::new(shell)
        .args(["-c", TETHER, "atelier-tether", &std::process::id().to_string(), &pid.to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    match thread::Builder::new().name("atelier-tether".into()).spawn(move || starter.wait()) {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
pub fn tether(_pid: u32) -> io::Result<()> {
    Ok(())
}
