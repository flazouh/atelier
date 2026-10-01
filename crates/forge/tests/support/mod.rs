//! Shared by the replay, present and perf tests: the recorded answers, and small ways to bend them.
//! Each test binary uses some of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use atelier_forge::{
    PullRef, RepoRef,
    github::{GitHub, testing::Fixtures},
};
use serde_json::Value;

pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/github")
}

/// The answers recorded from GitHub for oven-sh/bun#44169.
pub fn recorded() -> Fixtures {
    Fixtures::from_dir(&dir())
}

pub fn read(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join(name)).unwrap()).unwrap()
}

pub fn bun() -> RepoRef {
    RepoRef::new("github.com", "oven-sh", "bun")
}

pub fn pull_ref() -> PullRef {
    PullRef { repo: bun(), number: 44169 }
}

pub fn github(fixtures: &Fixtures) -> GitHub {
    GitHub::with_transport(fixtures.clone())
}

/// The recorded `Pull` answer with `change` applied to the pull request and the repository around it.
pub fn patched_pull(change: impl FnOnce(&mut Value)) -> Fixtures {
    let mut answer = read("Pull.json");
    change(&mut answer["data"]["repository"]);
    Fixtures::new().ok("Pull", answer.to_string())
}
