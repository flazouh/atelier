use serde_json::json;

use crate::{
    claude_code::accounts::{LIST_SCRIPT, is_account_name, parse_accounts, sign_in_command},
    session::Account,
};

const MARK: &str = "@@ ";
const USUAL: &str = "default";
const WORK: &str = "work";
const TEAM: &str = "team";
const MAX_PLAN: &str = "max";
const WORK_EMAIL: &str = "work@example.com";
/// A folder `claude` keeps beside `~/.claude-work` for its locks, which `~/.claude-*` also finds.
const LOCK_FOLDER: &str = "work.lock";

/// What the accounts script prints for one account: its name, then what `claude auth status --json` said.
fn listed(name: &str, status: &str) -> String {
    format!("{MARK}{name}\n{status}\n")
}

fn signed_in(plan: &str, email: &str) -> String {
    json!({"loggedIn": true, "authMethod": "claude.ai", "subscriptionType": plan, "email": email}).to_string()
}

fn signed_out() -> String {
    json!({"loggedIn": false, "authMethod": "none"}).to_string()
}

#[test]
fn each_account_comes_with_whether_it_is_signed_in_and_its_plan() {
    let text = [listed(USUAL, &signed_in(MAX_PLAN, WORK_EMAIL)), listed(TEAM, &signed_out())].concat();

    assert_eq!(
        parse_accounts(&text),
        vec![
            Account { name: USUAL.into(), signed_in: true, plan: Some(MAX_PLAN.into()), email: Some(WORK_EMAIL.into()) },
            Account { name: TEAM.into(), signed_in: false, plan: None, email: None },
        ]
    );
}

#[test]
fn a_status_spread_over_lines_still_reads() {
    let pretty = serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&signed_in(MAX_PLAN, WORK_EMAIL)).unwrap()).unwrap();

    assert!(parse_accounts(&listed(WORK, &pretty))[0].signed_in);
}

#[test]
fn an_account_whose_status_cannot_be_read_is_signed_out() {
    let accounts = parse_accounts(&listed(WORK, "claude: command not found"));

    assert_eq!(accounts, vec![Account { name: WORK.into(), signed_in: false, plan: None, email: None }]);
}

#[test]
fn nothing_listed_is_no_account() {
    assert!(parse_accounts("").is_empty());
}

#[test]
fn a_folder_whose_name_no_account_could_have_is_left_out() {
    let text = [listed(WORK, &signed_out()), listed(LOCK_FOLDER, &signed_out())].concat();

    let names: Vec<String> = parse_accounts(&text).into_iter().map(|a| a.name).collect();

    assert_eq!(names, vec![WORK.to_string()]);
}

#[test]
fn an_account_name_is_letters_digits_dashes_and_underscores() {
    for name in [USUAL, WORK, "team-2", "my_work"] {
        assert!(is_account_name(name), "{name}");
    }
    for name in ["", LOCK_FOLDER, "a b", "../x"] {
        assert!(!is_account_name(name), "{name}");
    }
}

/// The folders in a home, with a `claude` that says it is signed in only where `.signed-in` lies in the folder.
fn names_listed_in(home: &std::path::Path) -> Vec<String> {
    let stub = home.join("claude");
    std::fs::write(&stub, "#!/bin/sh\n[ -e \"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/.signed-in\" ] && echo '{\"loggedIn\":true}' || echo '{\"loggedIn\":false}'\n").unwrap();
    std::process::Command::new("chmod").arg("+x").arg(&stub).status().unwrap();
    let out = std::process::Command::new("sh").args(["-c", LIST_SCRIPT]).arg(&stub).env("HOME", home).output().unwrap();
    parse_accounts(&String::from_utf8_lossy(&out.stdout)).into_iter().map(|a| a.name).collect()
}

fn folder(home: &std::path::Path, name: &str, files: &[(&str, &str)]) {
    let dir = home.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
}

#[test]
fn a_folder_another_tool_keeps_is_not_an_account() {
    let home = tempfile::tempdir().unwrap();
    folder(home.path(), ".claude", &[]);
    folder(home.path(), ".claude-openrouter", &[(".credentials.json", "{}"), (".signed-in", "")]);
    folder(home.path(), ".claude-viaoauth", &[(".claude.json", r#"{"oauthAccount":{"emailAddress":"a@b.c"}}"#)]);
    folder(home.path(), ".claude-code-router", &[("config.json", "{}"), (".claude.json", "{}")]);
    folder(home.path(), ".claude-worktrees", &[(".claude.json", "{}")]);
    assert_eq!(names_listed_in(home.path()), ["default", "openrouter", "viaoauth"]);
}

#[test]
fn an_account_atelier_made_shows_before_its_sign_in_is_done() {
    let home = tempfile::tempdir().unwrap();
    folder(home.path(), ".claude-team", &[(".atelier-account", "")]);
    assert_eq!(names_listed_in(home.path()), ["team"]);
}

#[test]
fn the_sign_in_marks_the_folder_it_makes() {
    let home = tempfile::tempdir().unwrap();
    let stub = home.path().join("claude");
    std::fs::write(&stub, "#!/bin/sh\ntrue\n").unwrap();
    std::process::Command::new("chmod").arg("+x").arg(&stub).status().unwrap();
    let command = sign_in_command(stub.to_str().unwrap(), "team");
    let mut run = std::process::Command::new(&command.program);
    run.args(&command.args).env("HOME", home.path());
    run.envs(command.env.iter().cloned());
    assert!(run.status().unwrap().success());
    assert!(home.path().join(".claude-team/.atelier-account").exists());
}
