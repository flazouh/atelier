//! A small Discord that answers like `discordcli --json`: the same arguments in, the same JSON out, the same words on
//! stderr when it fails. It checks what the real tool checks (`--yes` before a write, an id of the right form, a link
//! that names the right server), so a wrong command fails here as it would there. It exists so the contract suite and the
//! mapping tests never reach the real service.
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::{Output, RunError, Runner, is_snowflake};

pub const GUILD: &str = "1100000000000000001";
pub const GENERAL: &str = "1200000000000000001";
pub const VOICE: &str = "1200000000000000002";
pub const DM: &str = "1300000000000000001";
pub const ME_ID: &str = "1400000000000000001";
pub const SAM_ID: &str = "1500000000000000001";

const EPOCH_MS: u64 = 1_420_070_400_000;

#[derive(Clone)]
struct Msg {
    id: String,
    channel: String,
    author_id: String,
    author: String,
    text: String,
    reply_to: Option<String>,
}

struct State {
    messages: Vec<Msg>,
    counter: u64,
    fail: Option<String>,
    calls: Vec<Vec<String>>,
}

pub struct FakeDiscord {
    state: Mutex<State>,
}

impl FakeDiscord {
    /// A server `Acme` with `#general`, a voice channel and a category, and a dm with `sam`. All are empty.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                messages: Vec::new(),
                counter: 0,
                fail: None,
                calls: Vec::new(),
            }),
        }
    }

    /// Makes every command fail with this text on stderr, as discordcli does.
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
        s.add(channel, SAM_ID, author, text, None)
    }

    /// The text of every message in a channel, oldest first, as Discord holds it.
    pub fn texts(&self, channel: &str) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.messages
            .iter()
            .filter(|m| m.channel == channel)
            .map(|m| m.text.clone())
            .collect()
    }
}

impl State {
    fn add(
        &mut self,
        channel: &str,
        author_id: &str,
        author: &str,
        text: &str,
        reply_to: Option<String>,
    ) -> String {
        self.counter += 1;
        // A real snowflake: the time since the Discord epoch, then the counter. One second apart keeps order obvious.
        let ms = 1_760_000_000_000 + self.counter * 1000;
        let id = (((ms - EPOCH_MS) << 22) | self.counter).to_string();
        self.messages.push(Msg {
            id: id.clone(),
            channel: channel.to_string(),
            author_id: author_id.to_string(),
            author: author.to_string(),
            text: text.to_string(),
            reply_to,
        });
        id
    }

    fn known(channel: &str) -> bool {
        [GENERAL, VOICE, DM].contains(&channel)
    }

    fn guild_of(channel: &str) -> Option<&'static str> {
        (channel != DM).then_some(GUILD)
    }

    fn row(&self, msg: &Msg) -> Value {
        let guild = Self::guild_of(&msg.channel);
        json!({
            "id": msg.id,
            "channelId": msg.channel,
            "guildId": guild,
            "authorId": msg.author_id,
            "author": msg.author,
            "message": msg.text,
            "at": "2025-10-09T08:53:20.000000+00:00",
            "url": format!("https://discord.com/channels/{}/{}/{}", guild.unwrap_or("@me"), msg.channel, msg.id),
            "attachments": [],
            "replyTo": msg.reply_to,
        })
    }
}

fn channel_row(id: &str, name: &str, kind: u32) -> Value {
    let guild = State::guild_of(id);
    json!({
        "id": id, "name": name, "type": kind, "guildId": guild, "parentId": null,
        "url": format!("https://discord.com/channels/{}/{id}", guild.unwrap_or("@me")),
    })
}

struct Args {
    command: String,
    n: usize,
    offset: usize,
    flags: Vec<(String, String)>,
    yes: bool,
    positional: Vec<String>,
}

impl Args {
    fn flag(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Reads arguments as discordcli does: `--` ends the options, and what follows is text.
fn parse(args: &[String]) -> Args {
    let mut parsed = Args {
        command: args.first().cloned().unwrap_or_default(),
        n: 20,
        offset: 0,
        flags: Vec::new(),
        yes: false,
        positional: Vec::new(),
    };
    let mut i = 1;
    let mut dashes = false;
    while i < args.len() {
        let a = &args[i];
        if dashes {
            parsed.positional.push(a.clone());
        } else if a == "--" {
            dashes = true;
        } else if a == "--yes" {
            parsed.yes = true;
        } else if a == "--json" || a == "--full" {
        } else if a == "-n" || a == "--limit" {
            i += 1;
            parsed.n = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(20);
        } else if a == "--offset" {
            i += 1;
            parsed.offset = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(0);
        } else if a.starts_with("--") {
            i += 1;
            parsed
                .flags
                .push((a.clone(), args.get(i).cloned().unwrap_or_default()));
        } else {
            parsed.positional.push(a.clone());
        }
        i += 1;
    }
    parsed
}

fn error(message: &str) -> Result<Output, RunError> {
    Ok(Output::failed(json!({ "error": message }).to_string()))
}

/// A channel id, or a link to a channel or a message: the channel, the message and the server the link names.
struct Target {
    channel: String,
    message: Option<String>,
    server: Option<String>,
}

fn target_of(text: &str) -> Result<Target, String> {
    if is_snowflake(text) {
        return Ok(Target {
            channel: text.to_string(),
            message: None,
            server: None,
        });
    }
    let parts: Vec<&str> = text
        .strip_prefix("https://discord.com/channels/")
        .ok_or("Invalid Discord channel link.")?
        .split('/')
        .collect();
    match parts.as_slice() {
        [server, channel, rest @ ..] if is_snowflake(channel) && rest.len() <= 1 => Ok(Target {
            channel: channel.to_string(),
            message: rest.first().map(|m| m.to_string()),
            server: Some(server.to_string()),
        }),
        _ => Err("Invalid Discord channel link.".to_string()),
    }
}

/// Finds the channel a target names, and checks the link against it as the tool does.
fn resolve(target: &Target) -> Result<(), String> {
    if !State::known(&target.channel) {
        return Err("Discord returned HTTP 404.".to_string());
    }
    if let Some(server) = &target.server
        && server != State::guild_of(&target.channel).unwrap_or("@me")
    {
        return Err(
            "This channel belongs to a different server. Check the link or --server.".into(),
        );
    }
    Ok(())
}

fn id_value(id: &str) -> u64 {
    id.parse().unwrap_or(0)
}

/// A test keeps a handle to look at the fake after the provider took the runner.
impl Runner for std::sync::Arc<FakeDiscord> {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        (**self).run(args)
    }
}

impl Runner for FakeDiscord {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        let mut s = self.state.lock().unwrap();
        s.calls.push(args.to_vec());
        if let Some(stderr) = &s.fail {
            return Ok(Output::failed(stderr.clone()));
        }
        let a = parse(args);
        let done = |v: Value| Ok(Output::ok(v.to_string()));
        match a.command.as_str() {
            "whoami" => done(json!({
                "id": ME_ID, "username": "alex_u", "name": "alex", "credentialSource": "session",
            })),
            "servers" => done(json!({
                "rows": [{ "id": GUILD, "name": "Acme", "owner": false }],
                "hasMore": false, "after": GUILD,
            })),
            "channels" => {
                if a.positional.first().map(String::as_str) != Some(GUILD) {
                    return error("Server name not found. Run discordcli servers and use an ID.");
                }
                let all = [
                    channel_row("1200000000000000010", "Text Channels", 4),
                    channel_row(GENERAL, "general", 0),
                    channel_row(VOICE, "Lounge", 2),
                ];
                let rows: Vec<Value> = all.iter().skip(a.offset).take(a.n).cloned().collect();
                let more = a.offset + a.n < all.len();
                done(json!({
                    "rows": rows, "serverId": GUILD, "total": all.len(), "hasMore": more,
                    "nextOffset": more.then_some(a.offset + a.n),
                }))
            }
            "dms" => done(json!({
                "rows": [channel_row(DM, "sam", 1)], "total": 1, "hasMore": false, "nextOffset": null,
            })),
            "read" => {
                let target = match a.positional.first().map(|t| target_of(t)) {
                    Some(Ok(t)) => t,
                    Some(Err(e)) => return error(&e),
                    None => return error("read expects 1 argument."),
                };
                if let Err(e) = resolve(&target) {
                    return error(&e);
                }
                let mut rows: Vec<&Msg> = s
                    .messages
                    .iter()
                    .filter(|m| m.channel == target.channel)
                    .collect();
                let page: Vec<&Msg> = if let Some(around) = &target.message {
                    match rows.iter().position(|m| &m.id == around) {
                        Some(at) => {
                            let from = at.saturating_sub(a.n / 2);
                            rows.drain(..from);
                            rows.into_iter().take(a.n).collect()
                        }
                        None => Vec::new(),
                    }
                } else if let Some(after) = a.flag("--after") {
                    rows.retain(|m| id_value(&m.id) > id_value(after));
                    rows.into_iter().take(a.n).collect()
                } else {
                    if let Some(before) = a.flag("--before") {
                        rows.retain(|m| id_value(&m.id) < id_value(before));
                    }
                    let skip = rows.len().saturating_sub(a.n);
                    rows.into_iter().skip(skip).collect()
                };
                let has_more = page.len() >= a.n;
                let rows: Vec<Value> = page.iter().map(|m| s.row(m)).collect();
                done(json!({
                    "rows": rows, "channelId": target.channel, "guildId": State::guild_of(&target.channel),
                    "hasMore": has_more,
                    "before": rows.first().map(|r| r["id"].clone()),
                    "after": rows.last().map(|r| r["id"].clone()),
                }))
            }
            "search" => {
                let word = a
                    .positional
                    .first()
                    .cloned()
                    .unwrap_or_default()
                    .to_lowercase();
                if a.flag("--server") != Some(GUILD) {
                    return error("search requires --server <id-or-name>.");
                }
                let mut found: Vec<&Msg> = s
                    .messages
                    .iter()
                    .filter(|m| m.channel != DM && m.text.to_lowercase().contains(&word))
                    .filter(|m| a.flag("--channel").is_none_or(|c| c == m.channel))
                    .filter(|m| a.flag("--from").is_none_or(|f| f == m.author_id))
                    .collect();
                found.reverse();
                let total = found.len();
                let rows: Vec<Value> = found
                    .iter()
                    .skip(a.offset)
                    .take(a.n)
                    .map(|m| s.row(m))
                    .collect();
                let next = a.offset + rows.len();
                let more = next < total;
                done(json!({
                    "rows": rows, "total": total, "hasMore": more, "nextOffset": more.then_some(next),
                }))
            }
            "send" | "reply" => {
                if !a.yes {
                    return error(
                        "Sending requires --yes. Review the exact target and text before adding it.",
                    );
                }
                let (Some(to), Some(text)) = (a.positional.first(), a.positional.get(1)) else {
                    return error("Use send <target> <text>.");
                };
                if text.trim().is_empty() || text.encode_utf16().count() > 2000 {
                    return error("Message text must contain 1..2000 characters.");
                }
                let target = match target_of(to) {
                    Ok(t) => t,
                    Err(e) => return error(&e),
                };
                if (a.command == "send") == target.message.is_some() {
                    return error("Use discordcli reply to respond to a message link.");
                }
                if let Err(e) = resolve(&target) {
                    return error(&e);
                }
                let reply_to = target.message.clone();
                if let Some(original) = &reply_to
                    && !s.messages.iter().any(|m| &m.id == original)
                {
                    return error("Discord returned HTTP 404.");
                }
                let id = s.add(&target.channel, ME_ID, "alex", text, reply_to);
                let guild = State::guild_of(&target.channel).unwrap_or("@me");
                done(json!({
                    "id": id, "channelId": target.channel,
                    "url": format!("https://discord.com/channels/{guild}/{}/{id}", target.channel),
                }))
            }
            other => error(&format!("Unknown command: {other}. Run discordcli --help.")),
        }
    }
}
