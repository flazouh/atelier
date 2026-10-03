use serde_json::json;

use crate::{claude_code::accounts::parse_accounts, session::Account};

const MARK: &str = "@@ ";
const USUAL: &str = "default";
const WORK: &str = "work";
const TEAM: &str = "team";
const MAX_PLAN: &str = "max";
const WORK_EMAIL: &str = "work@example.com";

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
