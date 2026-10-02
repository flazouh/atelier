use std::{
    io::{self, Read},
    sync::{Arc, Mutex, Weak, },
    thread,
    time::Duration,
};

use atelier_project::Link;

use crate::protocol::{Call, Reply, read_frame};
use super::structs::{Connection, Shared};

/// The waits between dials after a drop: a quick retry first, then up to 30 s apart.
fn backoff(attempt: u32) -> Duration {
    Duration::from_millis((500u64 << attempt.min(6)).min(30_000))
}

/// A wait in words: "30 s", or "200 ms" under a second.
pub(super) fn span(wait: Duration) -> String {
    match wait.as_secs() {
        0 => format!("{} ms", wait.as_millis()),
        s => format!("{s} s"),
    }
}

pub(super) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub(super) fn not_connected(host: &str, why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, format!("not connected to {host}: {why}"))
}

/// Reads frames until the connection ends, then starts dialing again.
fn read_loop(shared: Weak<Shared>, mut reader: Box<dyn Read + Send>, last_words: Box<dyn Fn() -> String + Send>) {
    let ended = loop {
        match read_frame(&mut reader) {
            Ok(Some(frame)) => match shared.upgrade() {
                Some(shared) => shared.handle(frame),
                None => return,
            },
            Ok(None) => break "the connection closed".to_string(),
            Err(error) => break error.to_string(),
        }
    };
    let Some(shared) = shared.upgrade() else { return };
    let words = last_words();
    let why = match words.trim() {
        "" => ended,
        words => format!("{ended}: {}", words.lines().last().unwrap_or(words)),
    };
    shared.dropped(why);
    let weak = Arc::downgrade(&shared);
    drop(shared);
    thread::spawn(move || redial(weak));
}

pub(super) fn attach(shared: &Arc<Shared>, connection: Connection) {
    *lock(&shared.writer) = Some(connection.writer);
    *lock(&shared.closer) = Some(connection.close);
    let weak = Arc::downgrade(shared);
    let (reader, last_words) = (connection.reader, connection.last_words);
    thread::spawn(move || read_loop(weak, reader, last_words));
}

/// Dials until a connection says hello, for as long as the project lives.
fn redial(shared: Weak<Shared>) {
    let mut attempt = 0;
    loop {
        thread::sleep(backoff(attempt));
        attempt += 1;
        let Some(shared) = shared.upgrade() else { return };
        let Ok(connection) = (shared.dial)() else { continue };
        attach(&shared, connection);
        // Calls wait on the hello, so the link is marked up for it alone.
        let was = lock(&shared.down).take();
        match shared.hello() {
            Ok(_) => {
                if !lock(&shared.watchers).is_empty() {
                    let _ = shared.request(Call::Watch);
                }
                shared.report(Link::Up);
                return;
            }
            Err(_) => *lock(&shared.down) = was,
        }
    }
}

pub(super) fn unexpected(reply: Reply) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("an unexpected answer: {reply:?}"))
}
