use crate::session::Backend;

use super::super::{Acp, AcpAgent};
use super::agent;

fn signing_in_with(login: &[&str]) -> Option<(String, Vec<String>)> {
    let acp = Acp::new(AcpAgent { login: login.iter().map(|word| word.to_string()).collect(), ..agent() });
    acp.sign_in("default").map(|command| (command.program.to_string_lossy().into_owned(), command.args))
}

#[test]
fn the_login_is_a_whole_command_so_it_may_be_another_program_than_the_adapter() {
    assert_eq!(signing_in_with(&["codex", "login"]), Some(("codex".into(), vec!["login".into()])));
}

#[test]
fn an_agent_with_no_login_has_no_sign_in() {
    assert_eq!(signing_in_with(&[]), None);
}
