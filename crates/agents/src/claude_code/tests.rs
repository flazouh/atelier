use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::session::Event;

use super::Mapper;

mod accounts;
mod control;
mod launch;
mod mapping;
mod process;
mod store;
mod streaming;
mod targets;

/// A captured run of `claude` 2.1.284 from `tests/fixtures/claude_code`.
fn fixture(name: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "claude_code", &format!("{name}.jsonl")].iter().collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The events of a captured run, each line one millisecond after the last.
fn replay(name: &str) -> Vec<Event> {
    replay_with(&mut Mapper::new(), &fixture(name))
}

fn replay_with(mapper: &mut Mapper, text: &str) -> Vec<Event> {
    let start = Instant::now();
    text.lines()
        .enumerate()
        .flat_map(|(i, line)| mapper.line(line, start + Duration::from_millis(i as u64)))
        .collect()
}
