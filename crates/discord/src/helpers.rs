use atelier_capabilities::{CapError, Ref, messaging::ChannelKind};

/// The account of the direct messages of the person.
pub const DM: &str = "dm";

/// Discord counts time from this moment (2015-01-01) in the top bits of every id.
const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

/// Whether `text` can be a Discord id: 17 to 20 digits, no leading zero, and it fits in 64 bits. The command line tool
/// refuses any other id, so a provider that checks first never runs a command that cannot work.
pub fn is_snowflake(text: &str) -> bool {
    (17..=20).contains(&text.len())
        && !text.starts_with('0')
        && text.bytes().all(|b| b.is_ascii_digit())
        && text.parse::<u64>().is_ok()
}

/// Milliseconds since the epoch at which Discord made this id. A message has no other exact time in the output.
pub fn snowflake_ms(id: &str) -> Option<i64> {
    let value: u64 = id.parse().ok()?;
    Some((value >> 22) as i64 + DISCORD_EPOCH_MS)
}

/// The link to a message, in the form `discordcli` takes. A direct message has the server `@me`.
pub fn link_of(channel: &Ref, message: &str) -> String {
    let server = if channel.account == DM {
        "@me"
    } else {
        &channel.account
    };
    format!(
        "https://discord.com/channels/{server}/{}/{message}",
        channel.id
    )
}

/// The kind of a channel, from Discord's type number. `None` for a channel that holds no messages a person reads here: a
/// category, a voice channel, a stage and a forum.
pub fn kind_of(channel_type: u32) -> Option<ChannelKind> {
    match channel_type {
        0 | 5 | 10 | 11 => Some(ChannelKind::Public),
        12 => Some(ChannelKind::Private),
        1 => Some(ChannelKind::Dm),
        3 => Some(ChannelKind::GroupDm),
        _ => None,
    }
}

/// Discord counts a message in UTF-16 units, so a limit of 2000 means 2000 of these.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The text of an error: `discordcli --json` prints `{"error":"..."}` on stderr, and without `--json` it prints
/// `Error: ...`.
fn message_of(stderr: &str) -> String {
    let text = stderr.trim();
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| text.strip_prefix("Error: ").unwrap_or(text).to_string())
}

/// What a failed `discordcli` command means. The messages are the ones in `src/cli.mjs`, `src/client.mjs` and
/// `src/session.mjs` of discordcli. The tool never prints the body Discord sent, only its own words.
pub fn error_of(stderr: &str) -> CapError {
    let text = message_of(stderr);
    let lower = text.to_lowercase();
    if lower.contains("run discordcli login")
        || lower.contains("not logged in")
        || lower.contains("discord_token is empty")
    {
        CapError::NotSignedIn
    } else if lower.contains("rate limit") {
        // The tool does not say how long. Its own retry waits at most 5 seconds, so ask for more than that.
        CapError::RateLimited {
            retry_after_ms: 10_000,
        }
    } else if lower.contains("http 404") {
        CapError::not_found("the channel or message")
    } else if lower.contains("request failed") {
        CapError::Offline
    } else if lower.contains("denied access") {
        CapError::Provider {
            code: "forbidden".into(),
            message: text,
        }
    } else if lower.contains("still indexing") {
        CapError::Provider {
            code: "search_pending".into(),
            message: text,
        }
    } else if lower.contains("snowflake") {
        CapError::invalid("id")
    } else {
        CapError::Provider {
            code: "discordcli".into(),
            message: text,
        }
    }
}
