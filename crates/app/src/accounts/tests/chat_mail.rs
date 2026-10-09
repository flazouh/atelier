//! Slack, Discord and Gmail through `build`: a good account makes a provider and names itself, each failure gives its
//! plain row, and one bad account hides no other.
use std::sync::Arc;

use atelier_capabilities::{
    CapError, CapResult,
    mail::{MailProvider, MemoryMail},
    messaging::{MemoryMessaging, MessagingProvider},
};
use atelier_settings::{AccountsSaved, DiscordSaved, GmailSaved, SlackSaved};

use crate::accounts::{AccountServices, Kind, Row, build, plain_words};

fn slack(saved: &SlackSaved) -> CapResult<Arc<dyn MessagingProvider>> {
    match saved.workspace.as_str() {
        "acme" => Ok(Arc::new(MemoryMessaging::new("acme").with_me("Ada"))),
        "loggedout" => Err(CapError::NotSignedIn),
        "offline" => Err(CapError::Offline),
        "missing" => Err(CapError::Provider {
            code: "slackcli_missing".into(),
            message: "`slackcli` is not installed".into(),
        }),
        _ => Err(CapError::invalid("workspace")),
    }
}

fn discord(saved: &DiscordSaved) -> CapResult<Arc<dyn MessagingProvider>> {
    match saved.server.as_str() {
        "Acme" => Ok(Arc::new(
            MemoryMessaging::new("1100000000000000001").with_me("Alex"),
        )),
        "loggedout" => Err(CapError::NotSignedIn),
        "gone" => Err(CapError::not_found("the server")),
        "" => Err(CapError::invalid("server")),
        _ => Err(CapError::Offline),
    }
}

fn gmail(saved: &GmailSaved) -> CapResult<Arc<dyn MailProvider>> {
    match saved.address.as_str() {
        "me@acme.test" => Ok(Arc::new(MemoryMail::new("me@acme.test"))),
        "other@acme.test" => Err(CapError::Provider {
            code: "account_mismatch".into(),
            message: "the browser is signed in to me@acme.test, not other@acme.test".into(),
        }),
        "far@acme.test" => Err(CapError::Provider {
            code: "not_installed".into(),
            message: "gmailcli: command not found".into(),
        }),
        _ => Err(CapError::NotSignedIn),
    }
}

fn services() -> AccountServices {
    AccountServices {
        slack,
        discord,
        gmail,
        ..AccountServices::isolated()
    }
}

fn saved_slack(workspace: &str) -> Option<SlackSaved> {
    Some(SlackSaved {
        workspace: workspace.into(),
        ..Default::default()
    })
}

fn saved_discord(server: &str) -> Option<DiscordSaved> {
    Some(DiscordSaved {
        server: server.into(),
        ..Default::default()
    })
}

fn saved_gmail(address: &str) -> Option<GmailSaved> {
    Some(GmailSaved {
        address: address.into(),
        ..Default::default()
    })
}

#[test]
fn a_good_slack_account_builds_a_provider_and_the_row_names_the_person() {
    let saved = AccountsSaved {
        slack: saved_slack("acme"),
        ..Default::default()
    };
    let built = build(&saved, &services());
    let accounts: Vec<_> = built
        .messaging
        .iter()
        .map(|p| p.account().to_string())
        .collect();
    assert_eq!(accounts, ["acme"]);
    assert_eq!(built.rows.slack, Row::Connected("Ada".into()));
    assert!(
        built.providers.is_empty() && built.mail.is_empty(),
        "nothing else is built"
    );
}

#[test]
fn a_good_discord_account_builds_a_provider_and_the_row_names_the_person() {
    let saved = AccountsSaved {
        discord: saved_discord("Acme"),
        ..Default::default()
    };
    let built = build(&saved, &services());
    assert_eq!(built.messaging[0].account(), "1100000000000000001");
    assert_eq!(built.rows.discord, Row::Connected("Alex".into()));
}

#[test]
fn a_good_gmail_account_builds_a_provider_and_the_row_names_the_address() {
    let saved = AccountsSaved {
        gmail: saved_gmail("me@acme.test"),
        ..Default::default()
    };
    let built = build(&saved, &services());
    assert_eq!(built.mail[0].account(), "me@acme.test");
    assert_eq!(built.rows.gmail, Row::Connected("me@acme.test".into()));
}

#[test]
fn each_failure_leaves_the_provider_out_and_gives_its_plain_row() {
    let row_of = |saved: AccountsSaved| {
        let built = build(&saved, &services());
        assert!(
            built.messaging.is_empty() && built.mail.is_empty(),
            "no provider for a failure"
        );
        built.rows
    };
    let slack_row = |name: &str| {
        row_of(AccountsSaved {
            slack: saved_slack(name),
            ..Default::default()
        })
        .slack
    };
    assert_eq!(slack_row("loggedout"), Row::NotSignedIn);
    assert_eq!(slack_row("offline"), Row::Offline);
    let Row::Failed(words) = slack_row("missing") else {
        panic!("a missing tool is a problem with words")
    };
    assert!(
        words.contains("slackcli is not installed")
            && words.contains("github.com/flazouh/slackcli"),
        "{words}"
    );
    let Row::Failed(words) = slack_row("") else {
        panic!("no workspace name is a problem with words")
    };
    assert!(words.contains("workspace"), "{words}");

    let discord_row = |server: &str| {
        row_of(AccountsSaved {
            discord: saved_discord(server),
            ..Default::default()
        })
        .discord
    };
    assert_eq!(discord_row("loggedout"), Row::NotSignedIn);
    assert_eq!(discord_row("elsewhere"), Row::Offline);
    assert_eq!(
        discord_row("gone"),
        Row::Failed("Discord does not show a server with this name to your login.".into())
    );
    assert_eq!(
        discord_row(""),
        Row::Failed("Name a server, or turn on Include direct messages.".into())
    );

    let gmail_row = |address: &str| {
        row_of(AccountsSaved {
            gmail: saved_gmail(address),
            ..Default::default()
        })
        .gmail
    };
    assert_eq!(gmail_row("nobody@acme.test"), Row::NotSignedIn);
    assert_eq!(
        gmail_row("other@acme.test"),
        Row::Failed("the browser is signed in to me@acme.test, not other@acme.test.".into())
    );
    let Row::Failed(words) = gmail_row("far@acme.test") else {
        panic!("a missing tool is a problem with words")
    };
    assert!(words.contains("gmailcli is not installed"), "{words}");
}

#[test]
fn one_bad_account_does_not_hide_the_others() {
    let saved = AccountsSaved {
        slack: saved_slack("loggedout"),
        discord: saved_discord("Acme"),
        gmail: saved_gmail("me@acme.test"),
        ..Default::default()
    };
    let built = build(&saved, &services());
    assert_eq!(built.rows.slack, Row::NotSignedIn);
    assert_eq!(built.messaging.len(), 1, "Discord is still there");
    assert_eq!(built.mail.len(), 1, "so is Gmail");
    assert!(matches!(built.rows.discord, Row::Connected(_)));
    assert!(matches!(built.rows.gmail, Row::Connected(_)));
}

#[test]
fn nothing_connected_builds_nothing_and_runs_no_tool() {
    fn slack_never(_: &SlackSaved) -> CapResult<Arc<dyn MessagingProvider>> {
        panic!("slackcli was used for a reader who connected nothing")
    }
    fn discord_never(_: &DiscordSaved) -> CapResult<Arc<dyn MessagingProvider>> {
        panic!("discordcli was used for a reader who connected nothing")
    }
    fn gmail_never(_: &GmailSaved) -> CapResult<Arc<dyn MailProvider>> {
        panic!("gmailcli was used for a reader who connected nothing")
    }
    let services = AccountServices {
        slack: slack_never,
        discord: discord_never,
        gmail: gmail_never,
        ..AccountServices::isolated()
    };
    let built = build(&AccountsSaved::default(), &services);
    assert!(built.messaging.is_empty() && built.mail.is_empty() && built.providers.is_empty());
    assert_eq!(built.rows, Default::default());
}

#[test]
fn the_words_for_a_missing_tool_say_where_to_get_it() {
    let missing = |code: &str| CapError::Provider {
        code: code.into(),
        message: String::new(),
    };
    assert!(
        plain_words(Kind::Slack, &missing("slackcli_missing"))
            .contains("github.com/flazouh/slackcli")
    );
    assert!(
        plain_words(Kind::Discord, &missing("discordcli_missing"))
            .contains("github.com/flazouh/discordcli")
    );
    assert!(plain_words(Kind::Slack, &CapError::NotSignedIn).contains("slackcli login"));
    assert!(plain_words(Kind::Discord, &CapError::NotSignedIn).contains("discordcli login"));
    assert!(plain_words(Kind::Gmail, &CapError::NotSignedIn).contains("Sign in to Gmail"));
}
