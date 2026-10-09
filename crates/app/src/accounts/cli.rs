//! The command line tools behind Slack, Discord and Gmail: what each saved block becomes. Every tool keeps its own login
//! (`slackcli`, `discordcli` and `gmailcli` hold the session), so nothing here reads or writes a secret. A tool is run by
//! the path the reader gave, or by its name on `PATH` when none is given.
use std::{path::Path, sync::Arc};

use atelier_capabilities::{
    CapError, CapResult,
    mail::MailProvider,
    messaging::{ChannelKind, MessagingProvider},
};
use atelier_discord::{DiscordConfig, DiscordMessaging, Runner as DiscordRunner, is_snowflake};
use atelier_gmail::{CliRunner, GmailMail};
use atelier_settings::{DiscordSaved, GmailSaved, SlackSaved};
use atelier_slack::{ChannelSpec, SlackConfig, SlackMessaging};
use serde_json::Value;

const SLACKCLI: &str = "slackcli";
const DISCORDCLI: &str = "discordcli";
const GMAILCLI: &str = "gmailcli";
/// The account of the direct messages, as the Discord provider names it.
const DM: &str = "dm";
/// How many pages of servers a name is looked up in.
const SERVER_PAGES: usize = 20;

/// The entries of a field the reader typed as a list: split at commas and line ends, trimmed, with no empty one and no
/// repeat.
pub(crate) fn split_list(text: &str) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    for part in text.split([',', '\n']) {
        let part = part.trim();
        if !part.is_empty() && !list.iter().any(|kept| kept == part) {
            list.push(part.to_string());
        }
    }
    list
}

/// The program to run: what the reader typed, or `tool` (found on `PATH`) when nothing is typed. A leading `~/` is the
/// home folder, which a shell would have expanded but a spawned program does not.
pub(crate) fn program_of(typed: &str, tool: &str) -> String {
    let typed = typed.trim();
    if typed.is_empty() {
        return tool.to_string();
    }
    match (typed.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest).to_string_lossy().into_owned(),
        _ => typed.to_string(),
    }
}

/// The workspace name as an account in references: a reference is separated by colons and read by spaces, so neither stays.
fn account_of(workspace: &str) -> String {
    workspace
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .replace(':', "-")
}

/// A Slack id (`C…` a channel, `G…` a private one or a group dm, `D…` a dm) is capitals and digits; a channel name is
/// lowercase.
fn slack_id(entry: &str) -> Option<ChannelKind> {
    let capitals = entry
        .bytes()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    match entry.chars().next() {
        Some('C') if capitals && entry.len() >= 3 => Some(ChannelKind::Public),
        Some('G') if capitals && entry.len() >= 3 => Some(ChannelKind::Private),
        Some('D') if capitals && entry.len() >= 3 => Some(ChannelKind::Dm),
        _ => None,
    }
}

/// The channels to show: `slackcli` reads a channel by its name or its id, so the entry is both. A `#` before a name is
/// the reader's habit and is left out.
fn channel_specs(entries: &[String]) -> Vec<ChannelSpec> {
    entries
        .iter()
        .map(|entry| entry.trim().trim_start_matches('#').trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let kind = slack_id(entry).unwrap_or(ChannelKind::Public);
            ChannelSpec::new(entry, entry, kind)
        })
        .collect()
}

/// What one Slack account serves. A workspace name is needed: it is the account in every reference.
pub(crate) fn slack_config(saved: &SlackSaved) -> CapResult<SlackConfig> {
    let account = account_of(&saved.workspace);
    if account.is_empty() {
        return Err(CapError::invalid("workspace"));
    }
    Ok(SlackConfig::new(&account, channel_specs(&saved.channels)))
}

/// What one Discord account serves, once the server is known (`dm` for the direct messages only). Sending is on only when
/// the reader turned it on: the login is a user token (see `docs/capabilities/discord-notes.md`).
pub(crate) fn discord_config(saved: &DiscordSaved, account: &str) -> DiscordConfig {
    DiscordConfig {
        include_dms: saved.include_dms,
        allow_writes: saved.allow_writes,
        ..DiscordConfig::new(account)
    }
}

/// The id of the server the reader named: an id as it is, a name looked up in the servers the login belongs to. An empty
/// name is the direct messages, which the reader must have asked for.
pub(crate) fn server_account(
    runner: &dyn DiscordRunner,
    saved: &DiscordSaved,
) -> CapResult<String> {
    let server = saved.server.trim();
    if server.is_empty() {
        return if saved.include_dms {
            Ok(DM.to_string())
        } else {
            Err(CapError::invalid("server"))
        };
    }
    if is_snowflake(server) {
        return Ok(server.to_string());
    }
    let mut after: Option<String> = None;
    for _ in 0..SERVER_PAGES {
        let mut args: Vec<String> = ["servers", "--json", "-n", "100"]
            .map(String::from)
            .to_vec();
        if let Some(cursor) = &after {
            args.extend(["--after".to_string(), cursor.clone()]);
        }
        let out = runner.run(&args).map_err(|e| match e {
            atelier_discord::RunError::NotInstalled(program) => CapError::Provider {
                code: "discordcli_missing".into(),
                message: format!("`{program}` is not installed"),
            },
            atelier_discord::RunError::Io(message) => CapError::Provider {
                code: "io".into(),
                message,
            },
        })?;
        if out.code != 0 {
            return Err(atelier_discord::error_of(&out.stderr));
        }
        let view: Value = serde_json::from_str(&out.stdout).map_err(|e| CapError::Provider {
            code: "bad_output".into(),
            message: format!("discordcli printed something unexpected: {e}"),
        })?;
        let found = view["rows"].as_array().into_iter().flatten().find(|row| {
            row["name"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(server))
        });
        if let Some(id) = found.and_then(|row| row["id"].as_str()) {
            return Ok(id.to_string());
        }
        match (view["hasMore"].as_bool(), view["after"].as_str()) {
            (Some(true), Some(next)) => after = Some(next.to_string()),
            _ => break,
        }
    }
    Err(CapError::not_found("the server"))
}

/// Slack over the reader's `slackcli`. It starts nothing: the first call comes when the caller asks who is signed in.
pub(super) fn slack_system(saved: &SlackSaved) -> CapResult<Arc<dyn MessagingProvider>> {
    let runner = atelier_slack::CommandRunner::new(program_of(&saved.program, SLACKCLI));
    Ok(Arc::new(SlackMessaging::new(runner, slack_config(saved)?)))
}

/// Discord over the reader's `discordcli`. A server given by name is looked up here, so this runs the tool once.
pub(super) fn discord_system(saved: &DiscordSaved) -> CapResult<Arc<dyn MessagingProvider>> {
    let program = program_of(&saved.program, DISCORDCLI);
    let account = server_account(&atelier_discord::CommandRunner::new(&program), saved)?;
    Ok(Arc::new(DiscordMessaging::new(
        atelier_discord::CommandRunner::new(program),
        discord_config(saved, &account),
    )))
}

/// Gmail over the reader's `gmailcli`, on this machine or over SSH on the host that holds the browser login. The address
/// is needed: it is the account in every reference, and the tool says which one the browser is signed in to.
pub(super) fn gmail_system(saved: &GmailSaved) -> CapResult<Arc<dyn MailProvider>> {
    let address = saved.address.trim();
    if address.is_empty() {
        return Err(CapError::invalid("address"));
    }
    let host = saved.host.trim();
    let runner = if host.is_empty() {
        CliRunner::local(program_of(&saved.program, GMAILCLI))
    } else {
        // The remote shell expands a `~`, not this machine.
        let typed = saved.program.trim();
        CliRunner::over_ssh(host, if typed.is_empty() { GMAILCLI } else { typed })
    };
    Ok(Arc::new(GmailMail::new(address, Arc::new(runner))))
}

#[cfg(test)]
mod tests;
