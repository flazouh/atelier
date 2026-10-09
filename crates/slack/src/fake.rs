//! A small Slack that answers like `slackcli --json`: the same arguments in, the same JSON out, the same messages on
//! stderr when it fails. It exists so the contract suite and the mapping tests never reach a real workspace.
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::{Output, RunError, Runner};

pub const ME: &str = "alex";
pub const ME_ID: &str = "U01";

struct Msg {
    channel: String,
    ts: String,
    thread_ts: Option<String>,
    author: String,
    text: String,
}

struct State {
    channels: Vec<(String, String)>,
    messages: Vec<Msg>,
    counter: u64,
    fail: Option<String>,
    calls: Vec<Vec<String>>,
}

pub struct FakeSlack {
    state: Mutex<State>,
}

impl FakeSlack {
    /// A workspace with `#general` (`C01`) and a dm with `sam` (`D01`).
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                channels: vec![
                    ("C01".into(), "general".into()),
                    ("D01".into(), "sam".into()),
                ],
                messages: Vec::new(),
                counter: 0,
                fail: None,
                calls: Vec::new(),
            }),
        }
    }

    /// Makes every command fail with this text on stderr, as slackcli does.
    pub fn fail_with(&self, stderr: &str) {
        self.state.lock().unwrap().fail = Some(stderr.to_string());
    }

    /// Every command run so far, arguments only.
    pub fn calls(&self) -> Vec<Vec<String>> {
        self.state.lock().unwrap().calls.clone()
    }

    /// Puts a message from somebody else in a channel, as if they had posted it.
    pub fn post_as(&self, channel: &str, author: &str, text: &str) -> String {
        let mut s = self.state.lock().unwrap();
        s.add(channel, None, author, text)
    }
}

impl State {
    fn add(
        &mut self,
        channel: &str,
        thread_ts: Option<String>,
        author: &str,
        text: &str,
    ) -> String {
        self.counter += 1;
        let ts = format!("{}.000100", 1_760_000_000 + self.counter);
        self.messages.push(Msg {
            channel: channel.to_string(),
            ts: ts.clone(),
            thread_ts,
            author: author.to_string(),
            text: text.to_string(),
        });
        ts
    }

    fn resolve(&self, reference: &str) -> Option<&(String, String)> {
        let wanted = reference.trim_start_matches('#');
        self.channels
            .iter()
            .find(|(id, name)| id == wanted || name == wanted)
    }

    fn name_of(&self, id: &str) -> String {
        self.channels
            .iter()
            .find(|(c, _)| c == id)
            .map_or_else(|| id.to_string(), |(_, n)| format!("#{n}"))
    }

    fn url(msg: &Msg) -> String {
        let (secs, micro) = msg.ts.split_once('.').unwrap();
        let base = format!(
            "https://app.slack.com/client/T01/{}/p{secs}{micro}",
            msg.channel
        );
        match &msg.thread_ts {
            Some(root) if *root != msg.ts => format!("{base}?thread_ts={root}"),
            _ => base,
        }
    }

    fn replies_of(&self, root: &Msg) -> usize {
        self.messages
            .iter()
            .filter(|m| {
                m.channel == root.channel
                    && m.thread_ts.as_deref() == Some(&root.ts)
                    && m.ts != root.ts
            })
            .count()
    }

    fn row(&self, msg: &Msg) -> Value {
        let mut row = json!({
            "author": msg.author,
            "message": msg.text,
            "at": "2025-10-09T08:53:20.000Z",
            "url": Self::url(msg),
            "files": [],
        });
        let replies = self.replies_of(msg);
        if replies > 0 && msg.thread_ts.is_none() {
            row["replies"] = json!(replies);
        }
        row
    }

    fn posted(&self, msg: &Msg) -> Value {
        json!({ "channel": self.name_of(&msg.channel), "ts": msg.ts, "url": Self::url(msg) })
    }
}

struct Args {
    command: String,
    n: usize,
    positional: Vec<String>,
    after_dashes: Vec<String>,
}

fn parse(args: &[String]) -> Args {
    let mut parsed = Args {
        command: args.first().cloned().unwrap_or_default(),
        n: 20,
        positional: Vec::new(),
        after_dashes: Vec::new(),
    };
    let mut i = 1;
    let mut dashes = false;
    while i < args.len() {
        let a = &args[i];
        if dashes {
            parsed.after_dashes.push(a.clone());
        } else if a == "--" {
            dashes = true;
        } else if a == "-n" {
            i += 1;
            parsed.n = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(20);
        } else if !a.starts_with("--") {
            parsed.positional.push(a.clone());
        }
        i += 1;
    }
    parsed
}

impl Runner for FakeSlack {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        let mut s = self.state.lock().unwrap();
        s.calls.push(args.to_vec());
        if let Some(stderr) = &s.fail {
            return Ok(Output::failed(stderr.clone()));
        }
        let a = parse(args);
        let first = a.positional.first().cloned().unwrap_or_default();
        let text = a.after_dashes.join(" ");
        let done = |v: Value| Ok(Output::ok(v.to_string()));
        match a.command.as_str() {
            "whoami" => done(json!({
                "user": ME, "userId": ME_ID, "teamId": "T01", "workspace": "acme",
                "host": "acme.slack.com", "credential": "session",
            })),
            "read" => {
                let Some((id, name)) = s.resolve(&first).cloned() else {
                    return Ok(no_channel(&first));
                };
                let mut top: Vec<&Msg> = s
                    .messages
                    .iter()
                    .filter(|m| {
                        m.channel == id
                            && (m.thread_ts.is_none() || m.thread_ts.as_deref() == Some(&m.ts))
                    })
                    .collect();
                let cut = top.len().saturating_sub(a.n);
                let rows: Vec<Value> = top.drain(cut..).map(|m| s.row(m)).collect();
                done(json!({ "channel": format!("#{name}"), "rows": rows }))
            }
            "thread" => {
                let Some((channel, ts)) = first.split_once(':') else {
                    return Ok(Output::failed(format!(
                        "Not a Slack thread reference: {first}."
                    )));
                };
                let Some(found) = s
                    .messages
                    .iter()
                    .find(|m| m.channel == channel && m.ts == ts)
                else {
                    return Ok(Output::failed(
                        "Slack refused conversations.replies: thread_not_found",
                    ));
                };
                let root_ts = found.thread_ts.clone().unwrap_or_else(|| found.ts.clone());
                let root = s
                    .messages
                    .iter()
                    .find(|m| m.channel == channel && m.ts == root_ts)
                    .unwrap();
                let replies: Vec<&Msg> = s
                    .messages
                    .iter()
                    .filter(|m| {
                        m.channel == channel
                            && m.thread_ts.as_deref() == Some(&root_ts)
                            && m.ts != root_ts
                    })
                    .collect();
                let cut = replies.len().saturating_sub(a.n);
                let mut rows = vec![s.row(root)];
                rows.extend(replies[cut..].iter().map(|m| s.row(m)));
                done(json!({
                    "channel": s.name_of(channel), "total": replies.len() - cut,
                    "hasMore": cut > 0, "rows": rows,
                }))
            }
            "search" => {
                let mut words = Vec::new();
                let (mut in_channel, mut from) = (None, None);
                for token in text.split_whitespace() {
                    if let Some(c) = token.strip_prefix("in:") {
                        in_channel = Some(c.trim_start_matches('#').to_string());
                    } else if let Some(f) = token.strip_prefix("from:") {
                        from = Some(f.trim_start_matches('@').to_string());
                    } else {
                        words.push(token.to_lowercase());
                    }
                }
                let hits: Vec<&Msg> = s
                    .messages
                    .iter()
                    .rev()
                    .filter(|m| {
                        in_channel
                            .as_ref()
                            .is_none_or(|c| s.name_of(&m.channel) == format!("#{c}"))
                    })
                    .filter(|m| from.as_ref().is_none_or(|f| &m.author == f))
                    .filter(|m| words.iter().all(|w| m.text.to_lowercase().contains(w)))
                    .collect();
                let rows: Vec<Value> = hits
                    .iter()
                    .take(a.n)
                    .map(|m| {
                        json!({
                            "channel": s.name_of(&m.channel), "author": m.author, "message": m.text,
                            "at": "2025-10-09T08:53:20.000Z", "url": State::url(m), "files": [],
                        })
                    })
                    .collect();
                done(json!({ "total": hits.len(), "rows": rows }))
            }
            "send" => {
                let Some((id, _)) = s.resolve(&first).cloned() else {
                    return Ok(no_channel(&first));
                };
                s.add(&id, None, ME, &text);
                done(s.posted(s.messages.last().unwrap()))
            }
            "reply" => {
                let Some((channel, ts)) = first.split_once(':') else {
                    return Ok(Output::failed(format!(
                        "Not a Slack thread reference: {first}."
                    )));
                };
                let Some(found) = s
                    .messages
                    .iter()
                    .find(|m| m.channel == channel && m.ts == ts)
                else {
                    return Ok(Output::failed(
                        "Slack refused chat.postMessage: thread_not_found",
                    ));
                };
                if found.thread_ts.as_deref().is_some_and(|root| root != ts) {
                    return Ok(Output::failed(
                        "Slack refused chat.postMessage: invalid_thread_ts",
                    ));
                }
                let (channel, ts) = (channel.to_string(), ts.to_string());
                s.add(&channel, Some(ts), ME, &text);
                done(s.posted(s.messages.last().unwrap()))
            }
            "edit" => {
                let Some((channel, ts)) = first.split_once(':') else {
                    return Ok(Output::failed(format!(
                        "Not a Slack thread reference: {first}."
                    )));
                };
                let (channel, ts) = (channel.to_string(), ts.to_string());
                let Some(at) = s
                    .messages
                    .iter()
                    .position(|m| m.channel == channel && m.ts == ts)
                else {
                    return Ok(Output::failed(
                        "Slack refused chat.update: message_not_found",
                    ));
                };
                s.messages[at].text = text;
                done(s.posted(&s.messages[at]))
            }
            "users" => {
                let wanted = text.to_lowercase();
                let all = [
                    (ME_ID, ME, Some("Alex Example"), false),
                    ("U02", "sam", Some("Sam Example"), false),
                    ("B01", "deploybot", None, true),
                ];
                let rows: Vec<Value> = all
                    .iter()
                    .filter(|(_, h, r, _)| {
                        wanted.is_empty() || h.contains(&wanted) || r.is_some_and(|r| r.to_lowercase().contains(&wanted))
                    })
                    .map(|(id, h, r, bot)| {
                        let mut row = json!({ "id": id, "mention": format!("<@{id}>"), "handle": h, "bot": bot });
                        if let Some(r) = r {
                            row["realName"] = json!(r);
                        }
                        row
                    })
                    .collect();
                done(json!({ "rows": rows }))
            }
            other => Ok(Output::failed(format!("Unknown command: {other}"))),
        }
    }
}

fn no_channel(name: &str) -> Output {
    Output::failed(format!(
        "No channel matches \"{name}\". If it is new, run: slackcli refresh"
    ))
}
