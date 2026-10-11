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

const SAVED_SESSION: &str = "5e55-1011";
const FORK_FLAG: &str = "--fork-session";

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
fn a_resume_runs_on_the_account_that_holds_the_session() {
    let host = FakeHost::new();
    host.save_session(Some(WORK_ACCOUNT), SAVED_SESSION);

    let started = host.run(&command(CLAUDE, &resuming(SAVED_SESSION)));

    assert_eq!(started, Ok(Started { account_folder: host.account_folder(WORK_ACCOUNT), first_arg: FIRST_FLAG.into() }));
}

#[test]
fn a_resume_of_a_session_on_the_usual_account_leaves_claude_as_it_is() {
    let host = FakeHost::new();
    host.save_session(None, SAVED_SESSION);

    let started = host.run(&command(CLAUDE, &resuming(SAVED_SESSION)));

    assert_eq!(started, Ok(Started { account_folder: String::new(), first_arg: FIRST_FLAG.into() }));
}

#[test]
fn a_fork_resumes_into_a_new_session_and_leaves_the_old_one() {
    let forking = OpenRequest { fork: true, ..resuming(SAVED_SESSION) };

    assert!(args(&forking).contains(&FORK_FLAG.to_string()));
    assert!(!args(&resuming(SAVED_SESSION)).contains(&FORK_FLAG.to_string()));
    assert!(!args(&OpenRequest { fork: true, ..OpenRequest::default() }).contains(&FORK_FLAG.to_string()), "nothing to fork");
}

#[test]
fn a_resume_on_another_account_brings_the_session_there_first() {
    let host = FakeHost::new();
    host.save_session(Some(WORK_ACCOUNT), SAVED_SESSION);
    host.sign_in(TEAM_ACCOUNT);

    let started = host.run(&command(CLAUDE, &OpenRequest { provider: Some(account(TEAM_ACCOUNT)), ..resuming(SAVED_SESSION) }));

    assert_eq!(started, Ok(Started { account_folder: host.account_folder(TEAM_ACCOUNT), first_arg: FIRST_FLAG.into() }));
    assert!(host.holds_session(Some(TEAM_ACCOUNT), SAVED_SESSION));
    assert!(host.holds_session(Some(WORK_ACCOUNT), SAVED_SESSION), "the old account keeps it");
}

#[test]
fn a_resume_on_the_usual_account_brings_the_session_into_its_folder() {
    let host = FakeHost::new();
    host.save_session(Some(WORK_ACCOUNT), SAVED_SESSION);

    let started = host.run(&command(CLAUDE, &OpenRequest { provider: Some(account(DEFAULT_ACCOUNT)), ..resuming(SAVED_SESSION) }));

    assert_eq!(started, Ok(Started { account_folder: String::new(), first_arg: FIRST_FLAG.into() }));
    assert!(host.holds_session(None, SAVED_SESSION));
}

#[test]
fn a_session_id_is_never_read_as_shell() {
    let host = FakeHost::new();
    host.sign_in(TEAM_ACCOUNT);

    let started = host.run(&command(CLAUDE, &OpenRequest { provider: Some(account(TEAM_ACCOUNT)), ..resuming(HOSTILE_ACCOUNT) }));

    assert!(started.is_ok(), "{started:?}");
    assert!(!host.has(HOSTILE_LEFTOVER));
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

#[test]
fn an_mcp_config_goes_to_claude_as_a_file_and_no_config_adds_no_flag() {
    let with = OpenRequest { mcp_config: Some("/run/atelier/mcp-1.json".into()), ..OpenRequest::default() };
    assert!(has_pair(&args(&with), "--mcp-config", "/run/atelier/mcp-1.json"));
    assert!(!args(&OpenRequest::default()).iter().any(|arg| arg == "--mcp-config"));
}
#[test]
fn the_permission_flow_is_the_same_with_an_mcp_config() {
    let with = OpenRequest { mcp_config: Some("/run/atelier/mcp-1.json".into()), ..OpenRequest::default() };
    assert!(has_pair(&args(&with), "--permission-prompt-tool", "stdio"));
}
/// A persona is text added to `claude`'s own system prompt at launch, at a new session and at a resume alike: the prompt is
/// not kept with the session, so a resume must say it again. It is one argument, whatever it holds.
#[test]
fn a_persona_goes_to_claude_as_an_appended_system_prompt_and_none_adds_no_flag() {
    let persona = "You are Dot.\nRole: Debugger. It's \"thorough\".";
    let with = OpenRequest { append_system_prompt: Some(persona.into()), ..OpenRequest::default() };
    assert!(has_pair(&args(&with), "--append-system-prompt", persona));
    let resumed = OpenRequest { append_system_prompt: Some(persona.into()), ..resuming(SAVED_SESSION) };
    assert!(has_pair(&args(&resumed), "--append-system-prompt", persona));
    assert!(!args(&OpenRequest::default()).iter().any(|arg| arg == "--append-system-prompt"));
    let blank = OpenRequest { append_system_prompt: Some("  ".into()), ..OpenRequest::default() };
    assert!(!args(&blank).iter().any(|arg| arg == "--append-system-prompt"), "a blank persona is none");
}

fn on(provider: Provider) -> OpenRequest {
    OpenRequest { provider: Some(provider), ..OpenRequest::default() }
}

fn resuming(session: &str) -> OpenRequest {
    OpenRequest { resume: Some(SessionId::new(session)), ..OpenRequest::default() }
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

/// An account's session gets a tool call's input as it is written, unless the environment says otherwise; an OpenRouter session
/// never asks.
#[test]
fn an_account_session_streams_tool_input_unless_the_environment_says() {
    use crate::claude_code::launch::streams_tool_input;
    let account = Provider::Account(DEFAULT_ACCOUNT.into());
    let router = Provider::OpenRouter { key: ApiKey::new("sk-or-test") };
    assert!(streams_tool_input(Some(&account), false));
    assert!(streams_tool_input(None, false));
    assert!(!streams_tool_input(Some(&account), true), "the reader's own setting stands");
    assert!(!streams_tool_input(Some(&router), false));
}

/// A new Claude Code session starts in Auto, which its own menu offers, and not in "ask first"; no other backend names a default.
#[test]
fn a_new_claude_code_session_starts_in_auto() {
    use crate::{claude_code::ClaudeCode, session::Backend};
    let caps = ClaudeCode::new().capabilities();
    assert_eq!(caps.default_mode, Some(PermissionMode::Auto));
    assert!(caps.permission_modes.contains(&PermissionMode::Auto), "the default is one the menu offers");
}

/// The models endpoint's answer becomes the picker's list: the id to start Claude Code with, and the name without "Claude ".
#[test]
fn the_models_endpoints_answer_names_the_real_versions() {
    let answer = r#"{"data":[{"id":"claude-opus-5-5","display_name":"Claude Opus 5.5","created_at":"2026-09-21T00:00:00Z"},{"id":"claude-haiku-4-5-20251001","display_name":"Claude Haiku 4.5"},{"id":"odd"}],"has_more":false}"#;
    let list = crate::claude_code::models::parse(answer).unwrap();
    let pairs: Vec<(&str, &str)> = list.iter().map(|m| (m.id.as_str(), m.label.as_str())).collect();
    assert_eq!(pairs, [("claude-opus-5-5", "Opus 5.5"), ("claude-haiku-4-5-20251001", "Haiku 4.5"), ("odd", "odd")], "opus, then haiku, then the others");
    assert!(crate::claude_code::models::parse(r#"{"data":[]}"#).is_err(), "no models is a reason, not an empty picker");
    assert!(crate::claude_code::models::parse("not json").is_err());
}

/// The first of the list is the agent's usual model: the newest Opus, not the newest model of any family.
#[test]
fn the_list_starts_on_the_newest_opus() {
    let answer = r#"{"data":[{"id":"claude-haiku-5-5","display_name":"Claude Haiku 5.5"},{"id":"claude-sonnet-5-5","display_name":"Claude Sonnet 5.5"},{"id":"claude-opus-5-5","display_name":"Claude Opus 5.5"},{"id":"claude-fable-5-1","display_name":"Claude Fable 5.1"},{"id":"claude-opus-4-8","display_name":"Claude Opus 4.8"}]}"#;
    let ids: Vec<String> = crate::claude_code::models::parse(answer).unwrap().into_iter().map(|m| m.id).collect();
    assert_eq!(ids, ["claude-opus-5-5", "claude-opus-4-8", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"]);
}
