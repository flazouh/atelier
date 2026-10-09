use std::{os::unix::fs::PermissionsExt, path::PathBuf};

use atelier_capabilities::{
    Actor, CapError, Ref,
    messaging::{ChannelKind, MessagingProvider, NewMessage},
};
use atelier_discord::{DiscordMessaging, Output, RunError, Runner};
use atelier_settings::{DiscordSaved, GmailSaved, SlackSaved};

use super::{
    channel_specs, discord_config, discord_system, gmail_system, program_of, server_account,
    slack_config, slack_system, split_list,
};

const ACME: &str = "1100000000000000001";
const SIDE: &str = "1100000000000000002";

/// A `discordcli` that answers `servers` with two pages and fails every other command, and counts nothing.
struct Servers;

impl Runner for Servers {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        assert_eq!(
            args[0], "servers",
            "only the server list is asked: {args:?}"
        );
        let page = if args.iter().any(|a| a == "--after") {
            format!(r#"{{"rows":[{{"id":"{SIDE}","name":"Side project"}}],"hasMore":false}}"#)
        } else {
            format!(
                r#"{{"rows":[{{"id":"{ACME}","name":"Acme"}}],"hasMore":true,"after":"{ACME}"}}"#
            )
        };
        Ok(Output::ok(page))
    }
}

/// A `discordcli` that must not run.
struct Never;

impl Runner for Never {
    fn run(&self, args: &[String]) -> Result<Output, RunError> {
        panic!("discordcli ran with {args:?}");
    }
}

struct Failing(Result<Output, RunError>);

impl Runner for Failing {
    fn run(&self, _: &[String]) -> Result<Output, RunError> {
        self.0.clone()
    }
}

fn discord(server: &str) -> DiscordSaved {
    DiscordSaved {
        server: server.into(),
        ..Default::default()
    }
}

/// A shell script that plays a command line tool.
fn script(dir: &tempfile::TempDir, name: &str, body: &str) -> String {
    let path: PathBuf = dir.path().join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn a_list_is_split_at_commas_and_lines_trimmed_and_without_repeats() {
    assert_eq!(
        split_list(" general, C01 ,\n random ,, general "),
        ["general", "C01", "random"]
    );
    assert!(split_list(" , \n ").is_empty());
}

#[test]
fn a_typed_path_is_used_as_it_is_and_nothing_typed_looks_on_the_path() {
    assert_eq!(program_of("  ", "slackcli"), "slackcli");
    assert_eq!(
        program_of(" /opt/bin/slackcli ", "slackcli"),
        "/opt/bin/slackcli"
    );
    let home = std::env::var("HOME").unwrap();
    assert_eq!(
        program_of("~/bin/slackcli", "slackcli"),
        format!("{home}/bin/slackcli")
    );
}

#[test]
fn slack_channels_are_read_by_name_or_id_and_an_id_says_what_kind_it_is() {
    let specs = channel_specs(&[
        "#general".into(),
        "C0123ABCD".into(),
        "G0123ABCD".into(),
        "D0123ABCD".into(),
    ]);
    let seen: Vec<(&str, ChannelKind)> = specs.iter().map(|s| (s.id.as_str(), s.kind)).collect();
    assert_eq!(
        seen,
        [
            ("general", ChannelKind::Public),
            ("C0123ABCD", ChannelKind::Public),
            ("G0123ABCD", ChannelKind::Private),
            ("D0123ABCD", ChannelKind::Dm),
        ]
    );
    assert!(
        specs.iter().all(|s| s.id == s.name),
        "slackcli takes either, so the entry is both"
    );
}

#[test]
fn the_workspace_name_is_the_account_and_has_no_space_or_colon() {
    let config = slack_config(&SlackSaved {
        workspace: " My team: two ".into(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(config.account, "My-team--two");
    let nameless = slack_config(&SlackSaved::default());
    assert!(matches!(nameless, Err(CapError::Invalid { .. })));
}

#[test]
fn a_discord_block_from_the_defaults_reads_only_and_cannot_send() {
    let saved = discord(ACME);
    assert!(!saved.allow_writes, "sending is off in a new block");
    let config = discord_config(&saved, ACME);
    assert!(!config.allow_writes && !config.include_dms);
    // `Never` proves nothing is run: a read-only provider refuses before it reaches the tool.
    let provider = DiscordMessaging::new(Never, config);
    let channel = Ref {
        capability: "messaging".into(),
        provider: "discord".into(),
        account: ACME.into(),
        id: "1200000000000000001".into(),
    };
    let sent = provider.send(
        &NewMessage::to(&channel, "hello"),
        &Actor::person("me", "Me"),
    );
    assert!(
        matches!(&sent, Err(CapError::Provider { code, .. }) if code == "read_only"),
        "{sent:?}"
    );
}

#[test]
fn turning_the_switches_on_reaches_the_provider() {
    let saved = DiscordSaved {
        include_dms: true,
        allow_writes: true,
        ..discord(ACME)
    };
    let config = discord_config(&saved, ACME);
    assert!(config.allow_writes && config.include_dms);
}

#[test]
fn a_server_given_by_id_is_used_without_asking_discord() {
    assert_eq!(server_account(&Never, &discord(ACME)).unwrap(), ACME);
}

#[test]
fn a_server_given_by_name_is_found_on_any_page_whatever_the_case() {
    assert_eq!(server_account(&Servers, &discord("acme")).unwrap(), ACME);
    assert_eq!(
        server_account(&Servers, &discord(" SIDE project ")).unwrap(),
        SIDE
    );
    assert!(matches!(
        server_account(&Servers, &discord("nowhere")),
        Err(CapError::NotFound { .. })
    ));
}

#[test]
fn no_server_means_the_direct_messages_only_when_they_are_asked_for() {
    let dms = DiscordSaved {
        include_dms: true,
        ..Default::default()
    };
    assert_eq!(server_account(&Never, &dms).unwrap(), "dm");
    assert!(matches!(
        server_account(&Never, &DiscordSaved::default()),
        Err(CapError::Invalid { .. })
    ));
}

#[test]
fn a_logged_out_or_missing_discordcli_gives_the_error_the_row_needs() {
    let logged_out = Failing(Ok(Output::failed("Not logged in. Run discordcli login.")));
    assert!(matches!(
        server_account(&logged_out, &discord("acme")),
        Err(CapError::NotSignedIn)
    ));
    let missing = Failing(Err(RunError::NotInstalled("discordcli".into())));
    assert!(matches!(
        server_account(&missing, &discord("acme")),
        Err(CapError::Provider { code, .. }) if code == "discordcli_missing"
    ));
}

#[test]
fn a_tool_at_the_typed_path_answers_and_one_that_is_missing_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let slackcli = script(
        &dir,
        "slackcli",
        r#"echo '{"user":"alex","userId":"U01","teamId":"T01","workspace":"acme"}'"#,
    );
    let slack = |program: String| SlackSaved {
        workspace: "acme".into(),
        program,
        ..Default::default()
    };
    let name = slack_system(&slack(slackcli))
        .unwrap()
        .whoami()
        .unwrap()
        .name;
    assert_eq!(name, "alex");

    let gone = dir
        .path()
        .join("nothing-here")
        .to_string_lossy()
        .into_owned();
    let error = slack_system(&slack(gone)).unwrap().whoami().unwrap_err();
    assert!(
        matches!(&error, CapError::Provider { code, .. } if code == "slackcli_missing"),
        "{error:?}"
    );
}

#[test]
fn a_logged_out_tool_is_not_signed_in() {
    let dir = tempfile::tempdir().unwrap();
    let program = script(
        &dir,
        "slackcli",
        "echo 'No Slack credentials found. Run slackcli login.' >&2; exit 1",
    );
    let saved = SlackSaved {
        workspace: "acme".into(),
        program,
        ..Default::default()
    };
    let error = slack_system(&saved).unwrap().whoami().unwrap_err();
    assert!(matches!(error, CapError::NotSignedIn), "{error:?}");
}

#[test]
fn a_discord_name_is_looked_up_through_the_tool_at_the_typed_path() {
    let dir = tempfile::tempdir().unwrap();
    let program = script(
        &dir,
        "discordcli",
        &format!(
            r#"case "$1" in
servers) echo '{{"rows":[{{"id":"{ACME}","name":"Acme"}}],"hasMore":false}}' ;;
whoami) echo '{{"id":"1400000000000000001","username":"alex_u","name":"Alex"}}' ;;
esac"#
        ),
    );
    let saved = DiscordSaved {
        program,
        ..discord("Acme")
    };
    let provider = discord_system(&saved).unwrap();
    assert_eq!(provider.account(), ACME);
    assert_eq!(provider.whoami().unwrap().name, "Alex");
}

#[test]
fn gmail_needs_an_address_and_runs_the_tool_at_the_typed_path() {
    assert!(matches!(
        gmail_system(&GmailSaved::default()),
        Err(CapError::Invalid { .. })
    ));
    let dir = tempfile::tempdir().unwrap();
    let program = script(
        &dir,
        "gmailcli",
        r#"echo '{"email":"me@acme.test","unread":2}'"#,
    );
    let saved = GmailSaved {
        address: " me@acme.test ".into(),
        program,
        ..Default::default()
    };
    let provider = gmail_system(&saved).unwrap();
    assert_eq!(provider.account(), "me@acme.test");
    assert_eq!(provider.whoami().unwrap().address, "me@acme.test");
}
