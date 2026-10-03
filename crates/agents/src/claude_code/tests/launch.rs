mod fake_host;

use atelier_project::Command;
use serde_json::{Map, Value};

use crate::{
    claude_code::launch::command,
    session::{ApiKey, OpenRequest, PermissionMode, Provider, SessionId},
};
use fake_host::{FakeHost, Started};

const CLAUDE: &str = "claude";
const MISSING_CLAUDE: &str = "/nonexistent/claude";
const FIRST_FLAG: &str = "--print";

const DEFAULT_ACCOUNT: &str = "default";
const WORK_ACCOUNT: &str = "work";
const TEAM_ACCOUNT: &str = "team";
/// Breaks out of any quoting it is spliced into, and leaves a file behind if a shell runs it.
const HOSTILE_ACCOUNT: &str = "a b'$(touch pwned)\"`touch pwned`";
const HOSTILE_LEFTOVER: &str = "pwned";

const OPENROUTER_KEY: &str = "sk-or-v1-secret";
const OPENROUTER_URL: &str = "https://openrouter.ai/api";
const BLANKED_FOR_OPENROUTER: [&str; 4] = ["ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"];

#[test]
fn a_new_session_asks_for_stream_json_and_permission_questions_over_stdio() {
    let args = args(&OpenRequest::default());
    assert!(has_pair(&args, "--input-format", "stream-json"));
    assert!(has_pair(&args, "--output-format", "stream-json"));
    assert!(has_pair(&args, "--permission-prompt-tool", "stdio"));
    assert!(args.contains(&"--include-partial-messages".to_string()));
    assert!(!args.contains(&"--resume".to_string()) && !args.contains(&"--model".to_string()));
    assert!(!args.contains(&"--permission-mode".to_string()), "Ask is claude's own default");
}

#[test]
fn resume_model_and_mode_become_flags() {
    let request = OpenRequest {
        resume: Some(SessionId::new("abc-123")),
        model: Some("sonnet".into()),
        mode: Some(PermissionMode::Plan),
        ..OpenRequest::default()
    };
    let args = args(&request);
    assert!(has_pair(&args, "--resume", "abc-123"));
    assert!(has_pair(&args, "--model", "sonnet"));
    assert!(has_pair(&args, "--permission-mode", "plan"));
}

#[test]
fn the_program_is_the_one_given() {
    assert_eq!(command("/opt/claude", &OpenRequest::default()).program, std::path::PathBuf::from("/opt/claude"));
}

#[test]
fn the_default_account_starts_claude_as_it_is() {
    assert_eq!(
        command(CLAUDE, &on(account(DEFAULT_ACCOUNT))),
        command(CLAUDE, &OpenRequest::default()),
        "~/.claude needs no CLAUDE_CONFIG_DIR; setting it is not the same"
    );
}

#[test]
fn another_account_runs_claude_with_its_own_folder_on_the_host() {
    let host = FakeHost::new();
    host.sign_in(WORK_ACCOUNT);

    let started = host.run(&command(CLAUDE, &on(account(WORK_ACCOUNT))));

    assert_eq!(started, Ok(Started { account_folder: host.account_folder(WORK_ACCOUNT), first_arg: FIRST_FLAG.into() }));
}

#[test]
fn an_account_name_is_never_read_as_shell() {
    let host = FakeHost::new();
    host.sign_in(HOSTILE_ACCOUNT);

    let started = host.run(&command(CLAUDE, &on(account(HOSTILE_ACCOUNT))));

    assert_eq!(started, Ok(Started { account_folder: host.account_folder(HOSTILE_ACCOUNT), first_arg: FIRST_FLAG.into() }));
    assert!(!host.has(HOSTILE_LEFTOVER));
}

#[test]
fn an_account_never_signed_in_fails_with_how_to_sign_in() {
    let host = FakeHost::new();

    let error = host.run(&command(CLAUDE, &on(account(TEAM_ACCOUNT)))).unwrap_err();

    assert!(error.contains("claude auth login"), "{error}");
    assert!(error.contains(&host.account_folder(TEAM_ACCOUNT)), "{error}");
}

#[test]
fn an_account_on_a_host_without_claude_says_claude_is_missing() {
    let host = FakeHost::new();
    host.sign_in(WORK_ACCOUNT);

    let error = host.run(&command(MISSING_CLAUDE, &on(account(WORK_ACCOUNT)))).unwrap_err();

    assert_eq!(error.trim(), format!("{MISSING_CLAUDE} is not installed on this host"));
}

#[test]
fn openrouter_points_claude_at_openrouter_with_the_key() {
    let command = command(CLAUDE, &on(openrouter()));

    assert_eq!(env(&command, "ANTHROPIC_BASE_URL"), Some(OPENROUTER_URL));
    assert_eq!(env(&command, "ANTHROPIC_API_KEY"), Some(OPENROUTER_KEY));
    for name in BLANKED_FOR_OPENROUTER {
        assert_eq!(env(&command, name), Some(""), "{name}");
    }
}

#[test]
fn openrouter_settings_beat_a_settings_file_but_never_carry_the_key() {
    let command = command(CLAUDE, &on(openrouter()));
    let settings = inline_settings_env(&command);

    assert_eq!(settings["ANTHROPIC_BASE_URL"], OPENROUTER_URL);
    assert_eq!(settings["ANTHROPIC_AUTH_TOKEN"], "");
    assert!(!settings.contains_key("ANTHROPIC_API_KEY"));
    assert!(command.args.iter().all(|arg| !arg.contains(OPENROUTER_KEY)), "the command line shows in ps");
}

#[test]
fn a_key_never_prints() {
    let printed = format!("{:?}", on(openrouter()));

    assert!(!printed.contains(OPENROUTER_KEY));
}

fn on(provider: Provider) -> OpenRequest {
    OpenRequest { provider: Some(provider), ..OpenRequest::default() }
}

fn account(name: &str) -> Provider {
    Provider::Account(name.into())
}

fn openrouter() -> Provider {
    Provider::OpenRouter { key: ApiKey::new(OPENROUTER_KEY) }
}

fn args(request: &OpenRequest) -> Vec<String> {
    command(CLAUDE, request).args
}

fn has_pair(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2).any(|pair| pair[0] == flag && pair[1] == value)
}

fn env<'a>(command: &'a Command, name: &str) -> Option<&'a str> {
    command.env.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
}

/// The `env` of the JSON after `--settings`.
fn inline_settings_env(command: &Command) -> Map<String, Value> {
    let at = command.args.iter().position(|arg| arg == "--settings").expect("an inline --settings");
    let settings: Value = serde_json::from_str(&command.args[at + 1]).expect("the settings are JSON");
    settings["env"].as_object().expect("the settings have an env").clone()
}
