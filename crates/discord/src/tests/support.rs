//! What the tests share: fixtures taken from the shapes discordcli prints, and the references they use.
use std::sync::{Arc, Mutex};

use atelier_capabilities::Ref;

use crate::{DiscordConfig, DiscordMessaging, Output, RunError, Runner};

pub const WHOAMI: &str = include_str!("../../tests/fixtures/whoami.json");
pub const SERVERS: &str = include_str!("../../tests/fixtures/servers.json");
pub const CHANNELS: &str = include_str!("../../tests/fixtures/channels.json");
pub const DMS: &str = include_str!("../../tests/fixtures/dms.json");
pub const READ: &str = include_str!("../../tests/fixtures/read.json");
pub const SEARCH: &str = include_str!("../../tests/fixtures/search.json");
pub const POSTED: &str = include_str!("../../tests/fixtures/posted.json");

pub use crate::fake::{GENERAL, GUILD};
/// The ids of the messages of `read.json`, oldest first. The second is answered by the third, and the third by the fifth.
pub const M1: &str = "1425768080998400000";
pub const M2: &str = "1425768332656640001";
pub const M3: &str = "1425768584314880002";
pub const M5: &str = "1425769087631360004";

/// Answers each command from a fixture and remembers what it was asked. A later answer for a command wins, so a test
/// can change one answer of `the_usual`.
#[derive(Clone, Default)]
pub struct Fixtures {
    answers: Arc<Mutex<Vec<(String, Output)>>>,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl Fixtures {
    pub fn with(self, command: &str, output: Output) -> Self {
        self.answers.lock().unwrap().push((command.into(), output));
        self
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    pub fn the_usual() -> Self {
        Self::default()
            .with("whoami", Output::ok(WHOAMI))
            .with("servers", Output::ok(SERVERS))
            .with("channels", Output::ok(CHANNELS))
            .with("dms", Output::ok(DMS))
            .with("read", Output::ok(READ))
            .with("search", Output::ok(SEARCH))
            .with("send", Output::ok(POSTED))
            .with("reply", Output::ok(POSTED))
    }
}

impl Runner for Fixtures {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        self.calls.lock().unwrap().push(args.to_vec());
        let answers = self.answers.lock().unwrap();
        Ok(answers
            .iter()
            .rev()
            .find(|(c, _)| c == &args[0])
            .map(|(_, o)| o.clone())
            .unwrap_or_else(|| Output::failed("Unknown command")))
    }
}

/// A provider for the server `Acme` that may not write.
pub fn provider(fixtures: &Fixtures) -> DiscordMessaging {
    DiscordMessaging::new(fixtures.clone(), DiscordConfig::new(GUILD))
}

/// The same provider, allowed to write.
pub fn writer(fixtures: &Fixtures) -> DiscordMessaging {
    let mut config = DiscordConfig::new(GUILD);
    config.allow_writes = true;
    DiscordMessaging::new(fixtures.clone(), config)
}

pub fn general() -> Ref {
    format!("messaging:discord:{GUILD}:{GENERAL}")
        .parse()
        .unwrap()
}

pub fn message(id: &str) -> Ref {
    format!("messaging:discord:{GUILD}:{GENERAL}:{id}")
        .parse()
        .unwrap()
}

/// The arguments of the command at `index`, as one string, to compare in a test.
pub fn call(fixtures: &Fixtures, index: usize) -> String {
    fixtures.calls()[index].join(" ")
}

/// The arguments of the last command run.
pub fn last_call(fixtures: &Fixtures) -> String {
    call(fixtures, fixtures.calls().len() - 1)
}
