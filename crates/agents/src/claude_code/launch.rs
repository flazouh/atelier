//! The command line that starts `claude` as a session atelier can drive.
use atelier_project::Command;

use super::control::mode_name;
use crate::session::{ApiKey, OpenRequest, PermissionMode, Provider, SessionId};

/// The command line for one piece of text: `claude --print` with the prompt on stdin, the answer as
/// plain text on stdout, in Plan mode (it changes nothing) and with no session saved, so it never shows
/// among the project's sessions.
pub(super) fn draft_command(program: &str, model: Option<&str>) -> Command {
    let mut args: Vec<String> = ["--print", "--no-session-persistence", "--output-format", "text", "--permission-mode", "plan"]
        .into_iter()
        .map(String::from)
        .collect();
    if let Some(model) = model {
        args.extend(["--model".into(), model.to_string()]);
    }
    Command::new(program).args(args)
}

pub(super) fn command(program: &str, request: &OpenRequest) -> Command {
    let mut args = vec![
        "--print",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        // Permission questions come to atelier as `control_request` lines, not to a terminal. `host`
        // alone denies them; the tool flag (hidden from `--help`) makes `claude` ask over stdio.
        "--permission-prompts",
        "host",
        "--permission-prompt-tool",
        "stdio",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    if let Some(session) = &request.resume {
        args.extend(["--resume".into(), session.as_str().into()]);
    }
    if let Some(model) = &request.model {
        args.extend(["--model".into(), model.clone()]);
    }
    // `Ask` is what `claude` does with no flag.
    if let Some(mode) = request.mode.filter(|mode| *mode != PermissionMode::Ask) {
        args.extend(["--permission-mode".into(), mode_name(mode).into()]);
    }
    match &request.provider {
        Some(Provider::OpenRouter { key }) => on_openrouter(program, key, args),
        Some(Provider::Account(name)) if name != DEFAULT_ACCOUNT => on_account(program, name, args),
        Some(Provider::Account(_)) => Command::new(program).args(args),
        None => match &request.resume {
            Some(session) => on_the_account_holding(program, session, args),
            None => Command::new(program).args(args),
        },
    }
}

/// The account `~/.claude` holds. Setting `CLAUDE_CONFIG_DIR` to `~/.claude` is not the same as leaving it unset.
const DEFAULT_ACCOUNT: &str = "default";

const SHELL: &str = "sh";
/// Carries the account's name into the script, so the script never holds it.
const ACCOUNT_ENV: &str = "ATELIER_CLAUDE_ACCOUNT";
/// Carries the id of the session to resume into the script, so the script never holds it.
const SESSION_ENV: &str = "ATELIER_CLAUDE_SESSION";

/// Stops with the words of [`crate::session::SessionError::Missing`] when the host has no `claude`.
const CHECK_CLAUDE: &str = r#"if ! command -v "$0" >/dev/null 2>&1; then
  printf '%s is not installed on this host\n' "$0" >&2
  exit 127
fi
"#;

/// Points `claude` at `~/.claude-<name>` on the host, whose home only a shell there knows, or stops with how to sign in.
const USE_ACCOUNT: &str = r#"CLAUDE_CONFIG_DIR="$HOME/.claude-$ATELIER_CLAUDE_ACCOUNT"
if [ ! -d "$CLAUDE_CONFIG_DIR" ]; then
  printf 'The Claude account "%s" is not signed in. Sign in with: CLAUDE_CONFIG_DIR="%s" claude auth login\n' "$ATELIER_CLAUDE_ACCOUNT" "$CLAUDE_CONFIG_DIR" >&2
  exit 1
fi
export CLAUDE_CONFIG_DIR
"#;

/// Points `claude` at the other account whose folder holds the session, when `~/.claude` does not.
const FIND_ACCOUNT: &str = r#"if ! ls "$HOME"/.claude/projects/*/"$ATELIER_CLAUDE_SESSION".jsonl >/dev/null 2>&1; then
  for saved in "$HOME"/.claude-*/projects/*/"$ATELIER_CLAUDE_SESSION".jsonl; do
    if [ -f "$saved" ]; then
      CLAUDE_CONFIG_DIR="${saved%/projects/*}"
      export CLAUDE_CONFIG_DIR
      break
    fi
  done
fi
"#;

const RUN_CLAUDE: &str = r#"exec "$0" "$@""#;

fn on_account(program: &str, name: &str, args: Vec<String>) -> Command {
    through_shell(program, &[CHECK_CLAUDE, USE_ACCOUNT, RUN_CLAUDE], args, (ACCOUNT_ENV, name))
}

fn on_the_account_holding(program: &str, session: &SessionId, args: Vec<String>) -> Command {
    through_shell(program, &[CHECK_CLAUDE, FIND_ACCOUNT, RUN_CLAUDE], args, (SESSION_ENV, session.as_str()))
}

/// `program` with `args`, started by a shell running `script`, with `input` in its environment.
fn through_shell(program: &str, script: &[&str], args: Vec<String>, (name, value): (&str, &str)) -> Command {
    let mut command = Command::new(SHELL).args(["-c", script.concat().as_str(), program]).args(args);
    command.env.push((name.into(), value.into()));
    command
}

const OPENROUTER_URL: &str = "https://openrouter.ai/api";
const BASE_URL_ENV: &str = "ANTHROPIC_BASE_URL";
const API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
const SETTINGS_FLAG: &str = "--settings";

/// Every other way `claude` could find a model or a credential, blanked so OpenRouter is the only one left.
const COMPETING_ENV: [&str; 16] = [
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_AWS_BASE_URL",
    "ANTHROPIC_BEDROCK_BASE_URL",
    "ANTHROPIC_BEDROCK_MANTLE_BASE_URL",
    "ANTHROPIC_FOUNDRY_BASE_URL",
    "ANTHROPIC_GOOGLE_CLOUD_BASE_URL",
    "ANTHROPIC_UNIX_SOCKET",
    "ANTHROPIC_VERTEX_BASE_URL",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    "CLAUDE_CODE_USE_ANTHROPIC_GOOGLE_CLOUD",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDE_CODE_USE_GATEWAY",
    "CLAUDE_CODE_USE_MANTLE",
    "CLAUDE_CODE_USE_VERTEX",
];

/// A settings file's `env` beats the process's, so the same values also go in an inline `--settings`, which
/// beats the files. The key stays out of it: the command line shows in `ps`.
fn on_openrouter(program: &str, key: &ApiKey, mut args: Vec<String>) -> Command {
    let openrouter_only = openrouter_env();
    args.extend([SETTINGS_FLAG.into(), settings_json(&openrouter_only)]);

    let mut command = Command::new(program).args(args);
    command.env = openrouter_only;
    command.env.push((API_KEY_ENV.into(), key.expose().into()));
    command
}

/// OpenRouter's address, and every competing variable blank.
fn openrouter_env() -> Vec<(String, String)> {
    let blanks = COMPETING_ENV.iter().map(|name| (name.to_string(), String::new()));
    let address = std::iter::once((BASE_URL_ENV.to_string(), OPENROUTER_URL.to_string()));
    blanks.chain(address).collect()
}

/// A settings document that sets `env`.
fn settings_json(env: &[(String, String)]) -> String {
    let env: serde_json::Map<String, serde_json::Value> = env.iter().map(|(name, value)| (name.clone(), value.clone().into())).collect();
    serde_json::json!({ "env": env }).to_string()
}
