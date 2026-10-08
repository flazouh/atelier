//! A host to run a launch command on: a home folder of its own, and a fake `claude` that reports what it
//! was started with instead of starting a session.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process,
};

use atelier_project::Command;
use tempfile::TempDir;

use super::CLAUDE;

/// Prints the account folder `claude` was given, then its first argument, one to a line.
const FAKE_CLAUDE_SCRIPT: &str = "#!/bin/sh\nprintf '%s\\n%s' \"$CLAUDE_CONFIG_DIR\" \"$1\"\n";
const FAKE_CLAUDE_FILE: &str = "fake-claude";
const EXECUTABLE: u32 = 0o755;
const USUAL_FOLDER: &str = ".claude";
const SOME_PROJECT: &str = "-home-user-code-atelier";

/// What the fake `claude` saw.
#[derive(Debug, PartialEq, Eq)]
pub struct Started {
    pub account_folder: String,
    pub first_arg: String,
}

pub struct FakeHost {
    home: TempDir,
}

impl FakeHost {
    pub fn new() -> Self {
        let host = Self { home: tempfile::tempdir().expect("a temporary home") };
        let fake = host.fake_claude();
        fs::write(&fake, FAKE_CLAUDE_SCRIPT).expect("the fake claude is written");
        fs::set_permissions(&fake, fs::Permissions::from_mode(EXECUTABLE)).expect("the fake claude can run");
        host
    }

    /// The folder `claude auth login` makes for `account`.
    pub fn account_folder(&self, account: &str) -> String {
        format!("{}/.claude-{account}", self.home.path().display())
    }

    pub fn sign_in(&self, account: &str) {
        fs::create_dir(self.account_folder(account)).expect("the account folder is made");
    }

    /// Saves an empty session file under `account`, or under `~/.claude` when `None`.
    pub fn save_session(&self, account: Option<&str>, id: &str) {
        let config = match account {
            Some(account) => self.account_folder(account),
            None => format!("{}/{USUAL_FOLDER}", self.home.path().display()),
        };
        let project = format!("{config}/projects/{SOME_PROJECT}");
        fs::create_dir_all(&project).expect("the project folder is made");
        fs::write(format!("{project}/{id}.jsonl"), "").expect("the session is saved");
    }

    /// Whether `account`, or `~/.claude` when `None`, holds the session `id`.
    pub fn holds_session(&self, account: Option<&str>, id: &str) -> bool {
        let config = match account {
            Some(account) => self.account_folder(account),
            None => format!("{}/{USUAL_FOLDER}", self.home.path().display()),
        };
        Path::new(&format!("{config}/projects/{SOME_PROJECT}/{id}.jsonl")).exists()
    }

    pub fn has(&self, file: &str) -> bool {
        self.home.path().join(file).exists()
    }

    /// Runs `command` here. `Err` holds what it wrote to stderr when it failed.
    pub fn run(&self, command: &Command) -> Result<Started, String> {
        let command = self.with_fake_claude(command.clone());
        let output = process::Command::new(&command.program)
            .args(&command.args)
            .envs(command.env)
            .env("HOME", self.home.path())
            .env_remove("CLAUDE_CONFIG_DIR")
            .current_dir(self.home.path())
            .output()
            .expect("the command runs");
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into());
        }
        let printed = String::from_utf8_lossy(&output.stdout);
        let (account_folder, first_arg) = printed.split_once('\n').expect("the fake claude printed two lines");
        Ok(Started { account_folder: account_folder.into(), first_arg: first_arg.into() })
    }

    /// `command` with the fake in place of `claude`, whether it starts `claude` itself or a shell that runs it.
    /// Any other program is left as it is.
    fn with_fake_claude(&self, mut command: Command) -> Command {
        let fake = self.fake_claude();
        match command.args.iter().position(|arg| arg == CLAUDE) {
            Some(at) => command.args[at] = fake.to_string_lossy().into(),
            None if command.program == Path::new(CLAUDE) => command.program = fake,
            None => {}
        }
        command
    }

    fn fake_claude(&self) -> PathBuf {
        self.home.path().join(FAKE_CLAUDE_FILE)
    }
}
