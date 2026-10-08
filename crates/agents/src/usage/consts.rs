/// The seconds in a minute, an hour and a day: a window is told in the biggest of them it fills.
pub(super) const MINUTE: u64 = 60;
pub(super) const HOUR: u64 = 60 * MINUTE;
pub(super) const DAY: u64 = 24 * HOUR;

/// Claude's usage endpoint, and the header that lets a Claude Code sign-in read it.
pub(super) const CLAUDE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub(super) const CLAUDE_BETA: (&str, &str) = ("anthropic-beta", "oauth-2025-04-20");
pub(super) const CLAUDE_TIMEOUT_SECS: u64 = 10;

/// Prints the sign-in Claude Code keeps: its file when that file holds the sign-in, else on a Mac its Keychain item. A file can
/// exist and hold only the MCP servers' sign-ins, so its existing is not enough. The token stays in this process.
pub(super) const CLAUDE_CREDENTIALS: &str = r#"f="$HOME/.claude/.credentials.json"
if [ -f "$f" ] && grep -q claudeAiOauth "$f"; then cat "$f"
elif command -v security >/dev/null 2>&1; then security find-generic-password -s "Claude Code-credentials" -w 2>/dev/null
fi"#;

/// Codex's app server, spoken to on its standard input. It leaves when the input ends, so the waits are how long it
/// is given to start and then to answer.
pub(super) const CODEX_SCRIPT: &str = r#"{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"atelier","version":"0"}}}'
  sleep 1
  printf '%s\n' '{"jsonrpc":"2.0","method":"initialized"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"account/rateLimits/read"}'
  sleep 3
} | "$0" app-server 2>/dev/null"#;
pub(super) const CODEX_READ_ID: i64 = 2;
pub(super) const SHELL: &str = "sh";

/// OpenRouter's key endpoint: what the key has spent, and the limit it was given if any.
pub(super) const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/key";
