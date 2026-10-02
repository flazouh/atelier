use std::{
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
    };

use sha2::{Digest, Sha256};

use super::types::Unavailable;

/// This machine's Rust target triple, if atelier downloads servers for it.
pub fn platform() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => Some("aarch64-apple-darwin"),
        ("x86_64", "macos") => Some("x86_64-apple-darwin"),
        ("aarch64", "linux") => Some("aarch64-unknown-linux-gnu"),
        ("x86_64", "linux") => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

/// Where atelier keeps its servers: `ATELIER_SERVERS_DIR`, else the platform's app data folder.
pub(super) fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ATELIER_SERVERS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let data = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"))
    };
    data.join("atelier/servers")
}

/// Moves a finished staging folder into place. When another atelier got there first, its copy is as
/// good as this one, so this one is dropped.
pub(super) fn settle(staging: &Path, folder: &Path) -> Result<(), Unavailable> {
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
pub(super) fn fetch_checked(url: &str, sha256: &str, to: &Path) -> Result<(), Unavailable> {
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
pub(super) fn gunzip(archive: &Path, to: &Path) -> Result<(), Unavailable> {
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
pub(super) fn untar(archive: &Path, into: &Path) -> Result<(), Unavailable> {
    run(Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(into).arg("--strip-components=1"), "tar")
}

pub(super) fn remove(file: &Path) -> Result<(), Unavailable> {
    fs::remove_file(file).map_err(|e| failed("cannot remove", file, e))
}

/// Runs a tool to the end; a failure carries what it printed.
pub(super) fn run(command: &mut Command, what: &str) -> Result<(), Unavailable> {
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

pub(super) fn failed(what: &str, path: &Path, error: io::Error) -> Unavailable {
    Unavailable::Failed(format!("{what} {}: {error}", path.display()))
}
