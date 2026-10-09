//! A fake `gmailcli` that behaves like a small mailbox. It reads the same arguments and prints the same JSON as the tool
//! (`whoami`, `search`, `thread`, `labels`, `attachments`), and keeps the date text the way Gmail shows it.
use std::{collections::HashMap, sync::Mutex};

use atelier_capabilities::mail::{Incoming, local_id};
use serde_json::{Value, json};

use crate::{RunFailure, Runner};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec",
];
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// `Fri, 18 Sept 2026, 10:39`, as the page of a British English account shows it. The minute is the finest part.
pub fn display(ms: i64) -> String {
    let minutes = ms.div_euclid(60_000);
    let days = minutes.div_euclid(1440);
    let of_day = minutes.rem_euclid(1440);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{}, {day} {} {year}, {:02}:{:02}",
        DAYS[(days + 4).rem_euclid(7) as usize],
        MONTHS[month as usize - 1],
        of_day / 60,
        of_day % 60
    )
}

/// After Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

struct FakeMessage {
    from: String,
    from_name: String,
    to: Vec<String>,
    date: i64,
    body: String,
    files: Vec<String>,
}

struct FakeThread {
    id: String,
    subject: String,
    messages: Vec<FakeMessage>,
    unread: bool,
}

#[derive(Default)]
struct State {
    threads: Vec<FakeThread>,
    /// Files `attachments -download` wrote, by path, and the bytes of each attachment by file name.
    written: HashMap<String, Vec<u8>>,
    bytes: HashMap<String, Vec<u8>>,
    next: u64,
}

pub struct FakeGmail {
    email: String,
    state: Mutex<State>,
    calls: Mutex<Vec<Vec<String>>>,
    failure: Mutex<Option<RunFailure>>,
    can_read_files: bool,
}

impl FakeGmail {
    pub fn new(email: &str) -> Self {
        Self {
            email: email.into(),
            state: Mutex::default(),
            calls: Mutex::default(),
            failure: Mutex::default(),
            can_read_files: true,
        }
    }

    pub fn without_files(mut self) -> Self {
        self.can_read_files = false;
        self
    }

    /// Every call from now on fails like this.
    pub fn fail_with(&self, failure: Option<RunFailure>) {
        *self.failure.lock().unwrap() = failure;
    }

    /// How many files the tool wrote that nobody removed.
    pub fn files_left(&self) -> usize {
        self.state.lock().unwrap().written.len()
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    pub fn exit(stdout: &str) -> RunFailure {
        RunFailure::Exit {
            code: Some(1),
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    /// A message arrives. With `incoming.thread` it joins that thread, else it starts one.
    pub fn deliver(&self, incoming: &Incoming) {
        let mut state = self.state.lock().unwrap();
        let message = FakeMessage {
            from: incoming.from.address.clone(),
            from_name: incoming.from.name.clone().unwrap_or_default(),
            to: incoming.to.iter().map(|c| c.address.clone()).collect(),
            date: incoming.date,
            body: incoming.text.clone(),
            files: incoming
                .attachments
                .iter()
                .map(|(n, ..)| n.clone())
                .collect(),
        };
        for (name, _, bytes) in &incoming.attachments {
            state.bytes.insert(name.clone(), bytes.clone());
        }
        let joined = incoming
            .thread
            .as_ref()
            .and_then(|t| state.threads.iter().position(|f| f.id == local_id(t)));
        match joined {
            Some(at) => {
                state.threads[at].messages.push(message);
                state.threads[at].unread = true;
            }
            None => {
                state.next += 1;
                let id = format!("{:016x}", 0x1a0b_3ab5_23a5_0000_u64 + state.next);
                state.threads.push(FakeThread {
                    id,
                    subject: incoming.subject.clone(),
                    messages: vec![message],
                    unread: true,
                });
            }
        }
    }

    fn last(thread: &FakeThread) -> &FakeMessage {
        &thread.messages[thread.messages.len() - 1]
    }

    fn search(&self, query: &str, n: usize) -> Value {
        let state = self.state.lock().unwrap();
        let mut unread = false;
        let mut only: Option<&str> = None;
        let mut words: Vec<String> = Vec::new();
        for token in query.split_whitespace() {
            match token {
                "is:unread" => unread = true,
                "-in:trash" | "-in:spam" => {}
                t if t.starts_with("in:") => only = Some(&t[3..]),
                t if t.starts_with("label:") => only = Some("label"),
                t => words.push(t.to_lowercase()),
            }
        }
        let mut found: Vec<(usize, &FakeThread)> = state
            .threads
            .iter()
            .enumerate()
            .filter(|_| matches!(only, None | Some("inbox") | Some("anywhere")))
            .filter(|(_, t)| !unread || t.unread)
            .filter(|(_, t)| {
                let hay = format!(
                    "{} {}",
                    t.subject,
                    t.messages
                        .iter()
                        .map(|m| format!("{} {}", m.body, m.from))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
                .to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect();
        found.sort_by_key(|(at, t)| std::cmp::Reverse((Self::last(t).date, *at)));
        let rows: Vec<Value> = found
            .iter()
            .take(n)
            .map(|(_, t)| {
                let first = &t.messages[0];
                json!({
                    "threadId": t.id,
                    "from": first.from,
                    "fromName": first.from_name,
                    "date": display(Self::last(t).date),
                    "subject": t.subject,
                    "snippet": Self::last(t).body.split_whitespace().collect::<Vec<_>>().join(" "),
                    "unread": t.unread,
                    "attachments": t.messages.iter().any(|m| !m.files.is_empty()),
                })
            })
            .collect();
        json!({ "account": { "email": self.email, "unread": 0 }, "query": query, "rows": rows })
    }

    fn thread(&self, id: &str) -> Value {
        let state = self.state.lock().unwrap();
        match state.threads.iter().find(|t| t.id == id) {
            Some(t) => json!({
                "subject": t.subject,
                "messages": t.messages.iter().map(|m| json!({
                    "from": m.from, "fromName": m.from_name, "to": m.to.join(", "),
                    "date": display(m.date), "body": m.body, "attachments": m.files,
                })).collect::<Vec<_>>(),
            }),
            // What the real tool does with an id it cannot find: one message with no sender and the text of the page.
            None => json!({
                "subject": "",
                "messages": [{ "from": "", "to": "", "date": "", "body": "Inbox (3) Compose", "attachments": [] }],
            }),
        }
    }

    fn attachments(&self, id: &str, dir: Option<&str>) -> Value {
        let mut state = self.state.lock().unwrap();
        let names: Vec<String> = state
            .threads
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.messages.iter().flat_map(|m| m.files.clone()).collect())
            .unwrap_or_default();
        let mut written = Vec::new();
        if let Some(dir) = dir {
            for name in &names {
                let path = format!("{dir}/{}", name.replace(['/', '\\'], "-"));
                let bytes = state.bytes.get(name).cloned().unwrap_or_default();
                state.written.insert(path.clone(), bytes);
                written.push(path);
            }
        }
        json!({ "names": names, "count": names.len(), "written": written })
    }
}

impl Runner for FakeGmail {
    fn run(&self, args: &[String]) -> Result<String, RunFailure> {
        self.calls.lock().unwrap().push(args.to_vec());
        if let Some(failure) = self.failure.lock().unwrap().clone() {
            return Err(failure);
        }
        let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or_default();
        let after = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|at| args.get(at + 1))
                .cloned()
        };
        let out = match arg(0) {
            "whoami" => json!({ "email": self.email, "unread": 0 }),
            "search" => self.search(
                arg(1),
                after("-n").and_then(|n| n.parse().ok()).unwrap_or(20),
            ),
            "thread" => self.thread(arg(1)),
            "labels" => json!([
                { "name": "Inbox", "unread": 1 }, { "name": "Starred", "unread": 0 },
                { "name": "Sent", "unread": 0 }, { "name": "Drafts", "unread": 0 },
                { "name": "Spam", "unread": 0 }, { "name": "Trash", "unread": 0 },
                { "name": "All Mail", "unread": 0 }, { "name": "Work/Projects", "unread": 2 },
            ]),
            "attachments" => self.attachments(arg(1), after("-download").as_deref()),
            other => return Err(Self::exit(&format!("error: unknown command {other:?}"))),
        };
        Ok(serde_json::to_string_pretty(&out).unwrap())
    }

    fn reads_files(&self) -> bool {
        self.can_read_files
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, RunFailure> {
        self.state
            .lock()
            .unwrap()
            .written
            .get(path)
            .cloned()
            .ok_or_else(|| Self::exit(&format!("cat: {path}: No such file")))
    }

    fn remove_dir(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .written
            .retain(|p, _| !p.starts_with(path));
    }
}
