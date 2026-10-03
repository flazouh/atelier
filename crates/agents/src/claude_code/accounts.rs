//! The Claude accounts on a host: `~/.claude`, and each `~/.claude-<name>` that a sign-in made, with what
//! `claude auth status` says of each. They are read through a process the project spawns, so a remote project
//! lists its host's accounts.
use atelier_project::{Command, Project};
use serde_json::Value;

use crate::{
    session::{Account, SessionError},
    subprocess,
};

/// The account `~/.claude` holds. Setting `CLAUDE_CONFIG_DIR` to `~/.claude` is not the same as leaving it unset.
pub const DEFAULT_ACCOUNT: &str = "default";

const MARK: &str = "@@ ";
const SHELL: &str = "sh";

/// For each account folder: `MARK name`, then `claude auth status --json` run against it. `$0` is `claude`.
const LIST_SCRIPT: &str = r#"for config in "$HOME/.claude" "$HOME"/.claude-*; do
  [ -d "$config" ] || continue
  if [ "$config" = "$HOME/.claude" ]; then
    printf '@@ default\n'
    "$0" auth status --json 2>/dev/null
  else
    printf '@@ %s\n' "${config##*/.claude-}"
    CLAUDE_CONFIG_DIR="$config" "$0" auth status --json 2>/dev/null
  fi
  echo
done"#;

/// Makes `~/.claude-<name>` and signs in to it. The name comes in through the environment, so the script never
/// holds it.
const SIGN_IN_SCRIPT: &str = r#"CLAUDE_CONFIG_DIR="$HOME/.claude-$ATELIER_CLAUDE_ACCOUNT"
mkdir -p "$CLAUDE_CONFIG_DIR"
export CLAUDE_CONFIG_DIR
exec "$0" auth login"#;
const ACCOUNT_ENV: &str = "ATELIER_CLAUDE_ACCOUNT";

pub fn accounts(project: &dyn Project, program: &str) -> Result<Vec<Account>, SessionError> {
    let command = Command::new(SHELL).args(["-c", LIST_SCRIPT, program]);
    Ok(parse_accounts(&subprocess::output(project, &command)?))
}

/// The command that opens the browser to sign in to `name`, making its folder first.
pub fn sign_in_command(program: &str, name: &str) -> Command {
    if name == DEFAULT_ACCOUNT {
        return Command::new(program).args(["auth", "login"]);
    }
    let mut command = Command::new(SHELL).args(["-c", SIGN_IN_SCRIPT, program]);
    command.env.push((ACCOUNT_ENV.into(), name.into()));
    command
}

/// Reads what [`LIST_SCRIPT`] printed. A status that is not JSON is an account signed out.
pub(super) fn parse_accounts(text: &str) -> Vec<Account> {
    let mut accounts = Vec::new();
    for block in text.split(MARK).filter(|block| !block.trim().is_empty()) {
        let (name, status) = block.split_once('\n').unwrap_or((block, ""));
        let name = name.trim();
        if is_account_name(name) {
            accounts.push(account(name, serde_json::from_str(status.trim()).unwrap_or(Value::Null)));
        }
    }
    accounts
}

/// Whether `name` could be an account's: letters, digits, `-` and `_`. Other folders `~/.claude-*` finds, such as
/// the locks `claude` keeps beside an account's folder, are not accounts.
pub fn is_account_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn account(name: &str, status: Value) -> Account {
    let text = |key: &str| status.get(key).and_then(Value::as_str).map(str::to_string);
    let signed_in = status.get("loggedIn").and_then(Value::as_bool).unwrap_or(false);
    Account {
        name: name.into(),
        signed_in,
        plan: if signed_in { text("subscriptionType") } else { None },
        email: if signed_in { text("email") } else { None },
    }
}
