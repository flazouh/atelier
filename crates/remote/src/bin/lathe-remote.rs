//! `lathe-remote --stdio`: serves the Project interface on stdin and stdout, for an app that started
//! it over ssh. `lathe-remote --version` prints the version the app checks before it uses a copy.

/// The protocol stamp, kept in the binary so the app reads it from the file.
#[used]
static STAMP: &[u8] = lathe_remote::protocol::STAMP;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version") => println!("{}", lathe_remote::ssh::version_line()),
        Some("--stdio") => {
            login_path();
            if let Err(error) = lathe_remote::server::serve(std::io::stdin().lock(), std::io::stdout()) {
                eprintln!("lathe-remote: {error}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("usage: lathe-remote --stdio | --version");
            std::process::exit(2);
        }
    }
}

/// `ssh host command` runs with a bare PATH, which misses the language servers the user installed
/// in `~/.cargo/bin` or `~/.local/bin`. The user's login shell knows the PATH they mean, so it is
/// asked once, as Zed does; those two folders are added when it did not name them.
fn login_path() {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let asked = std::process::Command::new(&shell).args(["-lc", "printf %s \"$PATH\""]).output();
    let mut path = match asked {
        Ok(out) if out.status.success() && !out.stdout.is_empty() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => std::env::var("PATH").unwrap_or_default(),
    };
    if let Some(home) = std::env::var_os("HOME") {
        for dir in [".cargo/bin", ".local/bin"] {
            let dir = std::path::Path::new(&home).join(dir);
            let dir = dir.to_string_lossy();
            if !path.split(':').any(|p| p == dir) && std::path::Path::new(dir.as_ref()).is_dir() {
                path = format!("{dir}:{path}");
            }
        }
    }
    // SAFETY: set once, before the server starts any thread.
    unsafe { std::env::set_var("PATH", path) };
}
